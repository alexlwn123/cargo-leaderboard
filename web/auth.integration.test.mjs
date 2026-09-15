import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import { randomUUID } from 'node:crypto';
import { postgres } from './local-database.mjs';
import { createAuthHandler } from '../api/auth.mjs';
import { createEventsHandler } from '../api/build-events.mjs';
import { mint, hash, identity, limit, requireCli, csrf, cookieName } from './auth.mjs';
import { submit, leaderboard } from './database.mjs';

// Requires an isolated disposable database; never load .env.local for this suite.
test('GitHub login → device approval → verified submission → revocation, with real PostgreSQL', { skip: !process.env.TEST_DATABASE_URL }, async () => {
  process.env.AUTH_SECRET = 'integration-only-secret-at-least-32-characters';
  process.env.APP_ORIGIN = 'https://cargo.lwn.lol';
  process.env.GITHUB_CLIENT_ID = 'test-client'; process.env.GITHUB_CLIENT_SECRET = 'test-secret';
  const sql = postgres(process.env.TEST_DATABASE_URL);
  try {
    for (let pass = 0; pass < 2; pass++) {
      for (const name of (await readdir('migrations')).sort()) {
        const schema = await readFile('migrations/' + name, 'utf8');
        await sql.transaction(schema.split(';').map(s => s.trim()).filter(Boolean).map(s => sql.query(s)));
      }
    }
    await sql.query('TRUNCATE auth_tokens, oauth_states, device_logins, build_events, github_users, submission_limits CASCADE');
    let providerCalls = 0;
    const auth = createAuthHandler({ getSql: () => sql, fetcher: async url => {
      providerCalls++;
      return url.includes('access_token') ? Response.json({ access_token: 'provider-secret' }) : Response.json({ id: 101, login: 'verified-alex', type: 'User' });
    } });
    const invoke = async (handler, action, { method = 'GET', body, headers = {} } = {}) => {
      const res = { headers: {}, setHeader(k, v) { this.headers[k] = v; }, end(value) { this.body = value ? JSON.parse(value) : null; } };
      await handler({ method, url: action, headers: { 'content-type': 'application/json', ...headers }, body, socket: { remoteAddress: '127.0.0.1' } }, res);
      return res;
    };
    const start = await invoke(auth, '/auth/device-start', { method: 'POST', body: {} });
    assert.equal(start.statusCode, 201);
    assert.equal(new URL(start.body.verification_uri_complete).origin, process.env.APP_ORIGIN);
    const { device_code, user_code } = start.body;
    const poll = () => invoke(auth, '/auth/device-poll', { method: 'POST', body: { device_code } });
    assert.equal((await poll()).statusCode, 202);
    assert.equal((await poll()).statusCode, 429);
    const login = await invoke(auth, '/auth/login?user_code=' + user_code);
    const oauth = new URL(login.headers.Location);
    assert.equal(oauth.origin, 'https://github.com');
    assert.equal(oauth.searchParams.get('redirect_uri'), 'https://cargo.lwn.lol/auth/callback');
    assert.equal(oauth.searchParams.get('scope'), '');
    assert.equal(oauth.searchParams.get('code_challenge_method'), 'S256');
    const callback = '/auth/callback?code=code&state=' + oauth.searchParams.get('state');
    assert.equal((await invoke(auth, callback)).statusCode, 400); // Missing browser binding.
    assert.equal(providerCalls, 0);
    const callbackHeaders = { cookie: login.headers['Set-Cookie'].split(';')[0] };
    const complete = await invoke(auth, callback, { headers: callbackHeaders });
    assert.equal(complete.statusCode, 302);
    assert.match(complete.headers.Location, /account.html\?code=/);
    assert.equal(providerCalls, 2);
    assert.equal((await invoke(auth, callback, { headers: callbackHeaders })).statusCode, 400); // Replay.
    const sessionCookie = complete.headers['Set-Cookie'].split(';')[0];
    const session = await invoke(auth, '/auth/session', { headers: { cookie: sessionCookie } });
    assert.equal(session.body.user.github_login, 'verified-alex');
    const browserHeaders = { cookie: sessionCookie, origin: process.env.APP_ORIGIN, 'x-csrf-token': session.body.csrf };
    assert.equal((await invoke(auth, '/auth/device-approve', { method: 'POST', body: { user_code }, headers: { ...browserHeaders, origin: 'https://cargo-leaderboard.vercel.app' } })).statusCode, 403);
    assert.equal((await invoke(auth, '/auth/device-approve', { method: 'POST', body: { user_code }, headers: { cookie: sessionCookie } })).statusCode, 403);
    assert.equal((await invoke(auth, '/auth/device-approve', { method: 'POST', body: { user_code }, headers: browserHeaders })).statusCode, 200);
    assert.equal((await invoke(auth, '/auth/device-approve', { method: 'POST', body: { user_code }, headers: browserHeaders })).statusCode, 400);
    const pendingSession = await invoke(auth, '/auth/session', { headers: { cookie: sessionCookie } });
    assert.equal(pendingSession.body.pending_devices, 1);
    assert.equal(pendingSession.body.devices, 0);
    await sql`UPDATE device_logins SET next_poll_at = NOW() - INTERVAL '1 second'`;
    const pollResults = await Promise.all([poll(), poll()]);
    const issued = pollResults.find(r => r.statusCode === 200);
    assert.ok(issued);
    assert.equal(pollResults.filter(r => r.statusCode === 200).length, 1);
    assert.equal((await poll()).statusCode, 400);
    const connectedSession = await invoke(auth, '/auth/session', { headers: { cookie: sessionCookie } });
    assert.equal(connectedSession.body.pending_devices, 0);
    assert.equal(connectedSession.body.devices, 1);
    const token = issued.body.token;
    const cliHeaders = { authorization: `Bearer ${token}` };
    assert.equal((await invoke(auth, '/auth/me', { headers: cliHeaders })).body.user.github_id, '101');
    const events = createEventsHandler({ authenticate: req => requireCli(req, sql), consume: (key, max) => limit(key, max, sql), save: (event, user) => submit(event, user, sql) });
    const event = { event_id: randomUUID(), nickname: 'impersonated-person', repo_slug: 'test/project', command: 'build', success: true,
      started_at: new Date(Date.now() - 1000).toISOString(), finished_at: new Date().toISOString(), duration_ms: 1000,
      bytes: 42, bytes_before: 0, bytes_after: 42, file_count: 1, profile: 'debug', platform: 'test', cargo_version: 'test', client_version: '0.3.0' };
    const submitted = await invoke(events, '/v1/build-events', { method: 'POST', body: event, headers: cliHeaders });
    assert.equal(submitted.statusCode, 201);
    const [stored] = await sql`SELECT nickname, github_id::text FROM build_events WHERE event_id = ${event.event_id}`;
    assert.deepEqual(stored, { nickname: 'verified-alex', github_id: '101' });
    assert.equal((await invoke(events, '/v1/build-events', { method: 'POST', body: event, headers: cliHeaders })).statusCode, 200);
    // Legacy scores are retained but excluded from the verified board.
    await sql`UPDATE build_events SET github_id = NULL WHERE event_id = ${event.event_id}`;
    assert.equal((await leaderboard('largest_build', 100, sql)).entries.length, 0);
    await sql`UPDATE build_events SET github_id = 101 WHERE event_id = ${event.event_id}`;
    await sql`UPDATE github_users SET github_login = 'renamed-account' WHERE github_id = 101`;
    const board = await leaderboard('largest_build', 100, sql);
    assert.equal(board.entries[0].github_login, 'renamed-account');
    assert.equal(Object.hasOwn(board.entries[0], 'nickname'), false);
    // Fresh scores coexist with historical folder scores for the same account/project.
    const fresh = { ...event, event_id: randomUUID(), bytes: 20, bytes_after: 20,
      benchmark: { rustc_version: 'rustc 1.95.0', revision: 'a'.repeat(40), dirty: false, cargo_args: ['--bins'] } };
    assert.equal((await invoke(events, '/v1/build-events', { method: 'POST', body: fresh, headers: cliHeaders })).statusCode, 201);
    assert.equal((await leaderboard('largest_build', 100, sql)).entries[0].bytes, 42);
    const freshBoard = await leaderboard('largest_fresh_build', 100, sql);
    assert.equal(freshBoard.entries.length, 1);
    assert.equal(freshBoard.entries[0].bytes, 20);
    assert.deepEqual(freshBoard.entries[0].benchmark, fresh.benchmark);
    assert.equal((await leaderboard('longest_single_build', 100, sql)).entries.length, 1);
    assert.equal((await leaderboard('largest_clean', 100, sql)).entries.length, 0);
    // Multiple tokens share one stable account quota, and simultaneous requests cannot exceed it.
    const token2 = mint('cli');
    await sql`INSERT INTO auth_tokens (token_hash, github_id, kind, expires_at) VALUES (${hash(token2)}, 101, 'cli', NOW() + INTERVAL '1 day')`;
    assert.equal((await identity(token2, 'cli', sql)).github_id, '101');
    await sql`DELETE FROM submission_limits WHERE client_hash = 'submit-user:101'`;
    const attempts = await Promise.allSettled(Array.from({ length: 35 }, () => limit('submit-user:101', 30, sql)));
    assert.equal(attempts.filter(r => r.status === 'fulfilled').length, 30);
    assert.equal((await invoke(events, '/v1/build-events', { method: 'POST', body: event, headers: { authorization: `Bearer ${token2}` } })).statusCode, 429);
    // Expired tokens and device codes fail; web sessions never authorize CLI submissions.
    await sql`UPDATE auth_tokens SET expires_at = NOW() - INTERVAL '1 second' WHERE token_hash = ${hash(token2)}`;
    await assert.rejects(identity(token2, 'cli', sql), /expired/);
    const browserToken = sessionCookie.slice(sessionCookie.indexOf('=') + 1);
    await assert.rejects(requireCli({ headers: { authorization: `Bearer ${browserToken}` } }, sql), /GitHub login/);
    const expiring = await invoke(auth, '/auth/device-start', { method: 'POST', body: {} });
    await sql`UPDATE device_logins SET expires_at = NOW() - INTERVAL '1 second'`;
    assert.equal((await invoke(auth, '/auth/device-poll', { method: 'POST', body: { device_code: expiring.body.device_code } })).statusCode, 400);
    assert.equal((await invoke(auth, '/auth/revoke-all', { method: 'POST', headers: browserHeaders })).statusCode, 200);
    assert.equal((await invoke(auth, '/auth/me', { headers: cliHeaders })).statusCode, 401);
    assert.equal((await invoke(auth, '/auth/logout', { method: 'POST', headers: browserHeaders })).statusCode, 200);
    assert.deepEqual((await invoke(auth, '/auth/session', { headers: { cookie: sessionCookie } })).body, { user: null });
    assert.equal(cookieName(), '__Host-clb_session');
  } finally { await sql.end(); }
});
