import { createHash, createHmac, randomBytes, timingSafeEqual } from 'node:crypto';
import { database } from './database.mjs';
import { HttpError } from './contract.mjs';

export const hash = value => createHash('sha256').update(value).digest('hex');
export function secret() {
  if (!process.env.AUTH_SECRET || process.env.AUTH_SECRET.length < 32)
    throw new Error('AUTH_SECRET must contain at least 32 characters');
  return process.env.AUTH_SECRET;
}
export function origin() {
  const url = new URL(process.env.APP_ORIGIN || 'https://cargo.lwn.lol');
  if (url.protocol !== 'https:' && !(['localhost', '127.0.0.1'].includes(url.hostname) && !process.env.VERCEL))
    throw new Error('APP_ORIGIN must use HTTPS');
  return url.origin;
}
const mac = value => createHmac('sha256', secret()).update(value).digest('base64url');
export function equal(a, b) {
  return typeof a === 'string' && typeof b === 'string' && a.length === b.length &&
    Buffer.byteLength(a) === Buffer.byteLength(b) && timingSafeEqual(Buffer.from(a), Buffer.from(b));
}
export function mint(kind) {
  const payload = `clb_${kind}_${randomBytes(32).toString('base64url')}`;
  return `${payload}.${mac(payload)}`;
}
export function valid(token, kind) {
  if (typeof token !== 'string' || !new RegExp(`^clb_${kind}_[A-Za-z0-9_-]{43}\\.[A-Za-z0-9_-]{43}$`).test(token)) return false;
  const [payload, signature] = token.split('.');
  return equal(signature, mac(payload));
}
export function cookieName(kind = 'session') {
  return `${origin().startsWith('https:') ? '__Host-' : ''}clb_${kind}`;
}
export function cookie(req, kind = 'session') {
  const name = cookieName(kind) + '=';
  return (req.headers.cookie || '').split(';').map(v => v.trim()).find(v => v.startsWith(name))?.slice(name.length);
}
export function setCookie(res, token, kind = 'session', age = 30 * 86400) {
  res.setHeader('Set-Cookie', `${cookieName(kind)}=${token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=${age}${origin().startsWith('https:') ? '; Secure' : ''}`);
}
export function bearer(req) { return (req.headers.authorization || '').replace(/^Bearer /, ''); }
export async function identity(token, kind, sql = database()) {
  if (!valid(token, kind)) throw new HttpError(401, 'Sign in with cargo leaderboard login. Your token is missing or invalid.');
  const [user] = await sql`
    SELECT u.github_id::text, u.github_login FROM auth_tokens t JOIN github_users u USING (github_id)
    WHERE t.token_hash = ${hash(token)} AND t.kind = ${kind} AND t.expires_at > NOW()`;
  if (!user) throw new HttpError(401, 'Your login has expired or been revoked. Run cargo leaderboard login.');
  return user;
}
export async function requireCli(req, sql) {
  // Reject random/anonymous traffic before opening a database connection.
  const token = bearer(req);
  if (!valid(token, 'cli')) throw new HttpError(401, 'GitHub login required. Install the latest CLI and run cargo leaderboard login.');
  return identity(token, 'cli', sql);
}
export function csrf(token) { return mac(`csrf:${token}`); }
export function requireCsrf(req, token) {
  if (req.headers.origin !== origin() || !equal(req.headers['x-csrf-token'], csrf(token)))
    throw new HttpError(403, 'Refresh this page and try again.');
}
export async function limit(key, max, sql = database()) {
  const rows = await sql`
    INSERT INTO submission_limits (client_hash, window_start, count) VALUES (${key}, date_trunc('hour', NOW()), 1)
    ON CONFLICT (client_hash) DO UPDATE SET
      count = CASE WHEN submission_limits.window_start < date_trunc('hour', NOW()) THEN 1 ELSE submission_limits.count + 1 END,
      window_start = date_trunc('hour', NOW())
    WHERE submission_limits.window_start < date_trunc('hour', NOW()) OR submission_limits.count < ${max}
    RETURNING count`;
  if (!rows.length) throw new HttpError(429, 'Too many requests. Try again next hour.');
}
export async function cleanup(sql = database()) {
  await sql.transaction([
    sql`DELETE FROM oauth_states WHERE expires_at < NOW()`,
    sql`DELETE FROM device_logins WHERE expires_at < NOW()`,
    sql`DELETE FROM auth_tokens WHERE expires_at < NOW()`,
    sql`DELETE FROM submission_limits WHERE window_start < NOW() - INTERVAL '2 days'`,
  ]);
}
export function userCode(value) {
  const code = typeof value === 'string' ? value.toUpperCase().replace(/-/g, '') : '';
  if (!/^[A-F0-9]{10}$/.test(code)) throw new HttpError(400, 'Enter the code shown by cargo leaderboard login.');
  return code;
}
export async function githubIdentity(code, verifier, fetcher = fetch) {
  const clientId = process.env.GITHUB_CLIENT_ID;
  const clientSecret = process.env.GITHUB_CLIENT_SECRET;
  if (!clientId || !clientSecret) throw new HttpError(503, 'GitHub sign-in is not configured yet.');
  const response = await fetcher('https://github.com/login/oauth/access_token', {
    method: 'POST', headers: { Accept: 'application/json', 'Content-Type': 'application/json' },
    body: JSON.stringify({ client_id: clientId, client_secret: clientSecret, code,
      redirect_uri: `${origin()}/auth/callback`, code_verifier: verifier }), signal: AbortSignal.timeout(10000),
  });
  const data = await response.json();
  if (!response.ok || !data.access_token) throw new HttpError(400, 'GitHub sign-in expired or was declined. Please sign in again.');
  const account = await fetcher('https://api.github.com/user', {
    headers: { Authorization: `Bearer ${data.access_token}`, Accept: 'application/vnd.github+json', 'User-Agent': 'cargo-leaderboard', 'X-GitHub-Api-Version': '2022-11-28' },
    signal: AbortSignal.timeout(10000),
  });
  const user = await account.json();
  if (!account.ok || !Number.isSafeInteger(user.id) || user.id < 1 || !/^[a-zA-Z0-9-]{1,39}$/.test(user.login) || user.type !== 'User')
    throw new HttpError(401, 'Could not verify your GitHub account.');
  // The GitHub access token is used only for this identity lookup, never stored or sent to the CLI.
  return { github_id: String(user.id), github_login: user.login };
}
