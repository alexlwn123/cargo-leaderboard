import test from 'node:test';
import assert from 'node:assert/strict';
import { mint, valid, hash, csrf, requireCsrf, requireCli, githubIdentity, origin } from './auth.mjs';
import { createAuthHandler } from '../api/auth.mjs';
import { createEventsHandler } from '../api/build-events.mjs';
process.env.AUTH_SECRET = 'test-only-secret-that-is-at-least-32-characters';
process.env.APP_ORIGIN = 'http://127.0.0.1:3000';
const response = () => ({ headers: {}, setHeader(k, v) { this.headers[k] = v; }, end(body) { this.body = body ? JSON.parse(body) : null; } });
test('OAuth defaults to the canonical domain and respects configured deployments', () => {
  const configured = process.env.APP_ORIGIN;
  try {
    delete process.env.APP_ORIGIN;
    assert.equal(origin(), 'https://cargo.lwn.lol');
    process.env.APP_ORIGIN = 'https://preview.example';
    assert.equal(origin(), 'https://preview.example');
  } finally {
    if (configured === undefined) delete process.env.APP_ORIGIN;
    else process.env.APP_ORIGIN = configured;
  }
});
test('signed credentials are scoped, unguessable and reject tampering before database access', async () => {
  const token = mint('cli');
  assert.equal(valid(token, 'cli'), true);
  assert.equal(valid(token, 'session'), false);
  assert.equal(valid(token.slice(0, -5) + 'aaaaa', 'cli'), false);
  assert.equal(valid(undefined, 'cli'), false);
  assert.notEqual(hash(token), token);
  await assert.rejects(requireCli({ headers: {} }), /login/);
  const handler = createAuthHandler({ getSql() { throw new Error('Unexpected database access'); } });
  const res = response();
  await handler({ method: 'GET', url: '/auth/session', headers: {} }, res);
  assert.deepEqual(res.body, { user: null });
});
test('browser mutations require both same-origin and session-bound CSRF proof', () => {
  const session = mint('session');
  const headers = { origin: process.env.APP_ORIGIN, 'x-csrf-token': csrf(session) };
  requireCsrf({ headers }, session);
  for (const patch of [{ origin: 'https://evil.example' }, { 'x-csrf-token': csrf(mint('session')) }, { 'x-csrf-token': undefined }])
    assert.throws(() => requireCsrf({ headers: { ...headers, ...patch } }, session), /Refresh/);
});
test('anonymous submissions cannot reach body parsing, quotas or storage', async () => {
  const handler = createEventsHandler({ consume() { throw new Error('Unexpected quota access'); }, save() { throw new Error('Unexpected save'); } });
  const res = response();
  await handler({ method: 'POST', headers: {}, get body() { throw new Error('Unexpected body access'); } }, res);
  assert.equal(res.statusCode, 401);
  assert.match(res.body.error, /GitHub login/);
});
test('GitHub account lookup validates response and discards provider token', async () => {
  process.env.GITHUB_CLIENT_ID = 'test'; process.env.GITHUB_CLIENT_SECRET = 'test';
  let calls = 0;
  const user = await githubIdentity('code', 'verifier', async (url, options) => {
    calls++;
    if (url.includes('/login/oauth/access_token')) {
      assert.equal(JSON.parse(options.body).code_verifier, 'verifier');
      assert.equal(JSON.parse(options.body).redirect_uri, process.env.APP_ORIGIN + '/auth/callback');
      return Response.json({ access_token: 'provider-token-never-returned' });
    }
    assert.equal(options.headers.Authorization, 'Bearer provider-token-never-returned');
    return Response.json({ id: 123, login: 'verified-user', type: 'User' });
  });
  assert.deepEqual(user, { github_id: '123', github_login: 'verified-user' });
  assert.equal(calls, 2);
});

test('a bare sign-in link returns to onboarding without contacting GitHub or the database', async () => {
  const handler = createAuthHandler({ getSql() { throw new Error('Unexpected database access'); }, fetcher() { throw new Error('Unexpected GitHub access'); } });
  const res = response();
  await handler({ method: 'GET', url: '/auth/login', headers: {} }, res);
  assert.equal(res.statusCode, 302);
  assert.equal(res.headers.Location, '/account.html');
});
