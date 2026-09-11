import { createHash, randomBytes } from 'node:crypto';
import { database } from '../web/database.mjs';
import { HttpError } from '../web/contract.mjs';
import { send, handleError, readEvent, clientHash } from '../web/http.mjs';
import { hash, mint, valid, origin, cookie, setCookie, bearer, identity, requireCli, csrf, requireCsrf, equal, limit, cleanup, userCode, githubIdentity } from '../web/auth.mjs';

const routes = { login: 'GET', callback: 'GET', session: 'GET', me: 'GET', logout: 'POST',
  'device-start': 'POST', 'device-poll': 'POST', 'device-approve': 'POST', 'revoke-all': 'POST', 'cli-logout': 'POST' };
const redirect = (res, path) => { res.statusCode = 302; res.setHeader('Location', path); res.end(); };

export function createAuthHandler({ getSql = database, fetcher = fetch } = {}) {
  return async function handler(req, res) {
    res.setHeader('Cache-Control', 'no-store');
    res.setHeader('Referrer-Policy', 'no-referrer');
    const url = new URL(req.url, origin());
    const action = url.searchParams.get('action') || url.pathname.split('/').pop();
    if (!Object.hasOwn(routes, action)) return send(res, 404, { error: 'Unknown authentication endpoint.' });
    if (req.method !== routes[action]) {
      res.setHeader('Allow', routes[action]);
      return send(res, 405, { error: `Use ${routes[action]}.` });
    }
    try {
      if (action === 'session' && !valid(cookie(req), 'session')) return send(res, 200, { user: null });
      if (action === 'me' || action === 'cli-logout') {
        if (!valid(bearer(req), 'cli')) throw new HttpError(401, 'Run cargo leaderboard login.');
        const sql = getSql();
        const user = await requireCli(req, sql);
        if (action === 'cli-logout') await sql`DELETE FROM auth_tokens WHERE token_hash = ${hash(bearer(req))}`;
        return send(res, 200, { user });
      }
      if (action === 'login' || action === 'device-start') {
        if (!process.env.GITHUB_CLIENT_ID || !process.env.GITHUB_CLIENT_SECRET)
          throw new HttpError(503, 'GitHub sign-in is not configured yet.');
        const sql = getSql();
        await limit(`login-ip:${clientHash(req)}`, 30, sql);
        await cleanup(sql);
        if (action === 'login') {
          const state = mint('state');
          const verifier = randomBytes(32).toString('base64url');
          const code = url.searchParams.has('user_code') ? userCode(url.searchParams.get('user_code')) : null;
          await sql`INSERT INTO oauth_states (state_hash, verifier, user_code) VALUES (${hash(state)}, ${verifier}, ${code})`;
          setCookie(res, state, 'state', 600);
          const params = new URLSearchParams({ client_id: process.env.GITHUB_CLIENT_ID, state,
            redirect_uri: `${origin()}/auth/callback`, scope: '', code_challenge_method: 'S256',
            code_challenge: createHash('sha256').update(verifier).digest('base64url') });
          return redirect(res, `https://github.com/login/oauth/authorize?${params}`);
        }
        await readEvent(req);
        const device = mint('device');
        const code = randomBytes(5).toString('hex').toUpperCase();
        await sql`INSERT INTO device_logins (device_hash, user_code) VALUES (${hash(device)}, ${code})`;
        return send(res, 201, { device_code: device, user_code: `${code.slice(0, 5)}-${code.slice(5)}`,
          verification_uri: `${origin()}/account.html`, verification_uri_complete: `${origin()}/account.html?code=${code}`, expires_in: 600, interval: 5 });
      }
      if (action === 'callback') {
        const state = url.searchParams.get('state');
        if (!valid(state, 'state') || !equal(state, cookie(req, 'state')))
          throw new HttpError(400, 'Sign-in state did not match. Start sign-in again in this browser.');
        const sql = getSql();
        const [pending] = await sql`DELETE FROM oauth_states WHERE state_hash = ${hash(state)} AND expires_at > NOW() RETURNING verifier, user_code`;
        if (!pending) throw new HttpError(400, 'Sign-in expired or was already used. Please sign in again.');
        const code = url.searchParams.get('code');
        if (!code || code.length > 256) throw new HttpError(400, 'GitHub sign-in was declined. Please sign in again.');
        const user = await githubIdentity(code, pending.verifier, fetcher);
        await limit(`web-logins:${user.github_id}`, 30, sql);
        const session = mint('session');
        await sql.transaction([
          sql`INSERT INTO github_users (github_id, github_login) VALUES (${user.github_id}, ${user.github_login})
            ON CONFLICT (github_id) DO UPDATE SET github_login = EXCLUDED.github_login, updated_at = NOW()`,
          sql`INSERT INTO auth_tokens (token_hash, github_id, kind, expires_at) VALUES (${hash(session)}, ${user.github_id}, 'session', NOW() + INTERVAL '30 days')`,
        ]);
        setCookie(res, session);
        return redirect(res, '/account.html' + (pending.user_code ? `?code=${pending.user_code}` : ''));
      }
      if (action === 'device-poll') {
        const body = await readEvent(req);
        if (!valid(body?.device_code, 'device')) throw new HttpError(400, 'Invalid device code. Run cargo leaderboard login again.');
        const sql = getSql();
        const deviceHash = hash(body.device_code);
        // UPDATE takes a row lock: concurrent polls cannot bypass the five-second interval.
        const [device] = await sql`UPDATE device_logins SET next_poll_at = NOW() + INTERVAL '5 seconds'
          WHERE device_hash = ${deviceHash} AND expires_at > NOW() AND next_poll_at <= NOW() RETURNING github_id`;
        if (!device) {
          const [exists] = await sql`SELECT 1 FROM device_logins WHERE device_hash = ${deviceHash} AND expires_at > NOW()`;
          return send(res, exists ? 429 : 400, { error: exists ? 'slow_down' : 'expired_token' });
        }
        if (!device.github_id) return send(res, 202, { status: 'authorization_pending' });
        const token = mint('cli');
        // Consuming the grant and issuing its one token are one atomic operation.
        const [, accounts] = await sql.transaction([
          sql`SELECT pg_advisory_xact_lock(${device.github_id}::bigint)`,
          sql`
          WITH consumed AS (DELETE FROM device_logins WHERE device_hash = ${deviceHash} AND github_id IS NOT NULL AND expires_at > NOW() RETURNING github_id),
          issued AS (INSERT INTO auth_tokens (token_hash, github_id, kind, expires_at)
            SELECT ${hash(token)}, github_id, 'cli', NOW() + INTERVAL '90 days' FROM consumed RETURNING github_id, expires_at)
          SELECT u.github_id::text, u.github_login, issued.expires_at FROM issued JOIN github_users u USING (github_id)`,
        ]);
        const [account] = accounts;
        if (!account) throw new HttpError(400, 'Login was already completed. Start again.');
        return send(res, 200, { token, ...account });
      }
      const session = cookie(req);
      if (!valid(session, 'session')) throw new HttpError(401, 'Sign in with GitHub first.');
      const sql = getSql();
      const user = await identity(session, 'session', sql);
      if (action === 'session') {
        const [counts] = await sql`SELECT
          (SELECT count(*)::int FROM auth_tokens WHERE github_id = ${user.github_id} AND kind = 'cli' AND expires_at > NOW()) AS devices,
          (SELECT count(*)::int FROM device_logins WHERE github_id = ${user.github_id} AND expires_at > NOW()) AS pending_devices`;
        return send(res, 200, { user, csrf: csrf(session), devices: counts.devices, pending_devices: counts.pending_devices });
      }
      requireCsrf(req, session);
      await limit(`account-actions:${user.github_id}`, 60, sql);
      if (action === 'logout') {
        await sql`DELETE FROM auth_tokens WHERE token_hash = ${hash(session)}`;
        setCookie(res, '', 'session', 0);
      } else if (action === 'revoke-all') {
        // Also cancel approved grants, so a pending poll cannot resurrect access after revocation.
        await sql.transaction([
          sql`SELECT pg_advisory_xact_lock(${user.github_id}::bigint)`,
          sql`DELETE FROM auth_tokens WHERE github_id = ${user.github_id} AND kind = 'cli'`,
          sql`DELETE FROM device_logins WHERE github_id = ${user.github_id}`,
        ]);
      } else if (action === 'device-approve') {
        const code = userCode((await readEvent(req))?.user_code);
        await limit(`device-approvals:${user.github_id}`, 15, sql);
        const [approved] = await sql`UPDATE device_logins SET github_id = ${user.github_id}
          WHERE user_code = ${code} AND github_id IS NULL AND expires_at > NOW() RETURNING user_code`;
        if (!approved) throw new HttpError(400, 'This code is expired, invalid, or already approved. Run cargo leaderboard login again.');
      }
      send(res, 200, { ok: true });
    } catch (error) {
      if (error.status === 429) res.setHeader('Retry-After', action === 'device-poll' ? '5' : '3600');
      if (action === 'session' && error.status === 401) {
        setCookie(res, '', 'session', 0);
        return send(res, 200, { user: null });
      }
      handleError(res, error);
    }
  };
}
export default createAuthHandler();
