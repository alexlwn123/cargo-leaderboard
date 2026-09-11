const $ = selector => document.querySelector(selector);
let session;
const code = new URL(location.href).searchParams.get('code');
if (code && /^[a-f0-9]{10}$/i.test(code)) {
  $('#device-code').value = code.slice(0, 5).toUpperCase() + '-' + code.slice(5).toUpperCase();
  $('#sign-in').href = '/auth/login?user_code=' + encodeURIComponent(code);
}
async function load() {
  try {
    const res = await fetch('/auth/session', { cache: 'no-store' });
    if (!res.ok) throw new Error('Could not check your login. Refresh to try again.');
    session = await res.json();
    if (session.local) {
      $('#account-status').textContent = 'This local board uses nickname setup. Run cargo leaderboard setup --nickname YOUR_NAME --api-url ' + location.origin;
      return;
    }
    $('#signed-out').hidden = !!session.user;
    $('#signed-in').hidden = !session.user;
    $('#account-status').textContent = session.user ? `Signed in as @${session.user.github_login}` : 'Connect your GitHub account to get started.';
    if (session.user) {
      $('#device-count').textContent = `${session.devices} active CLI ${session.devices === 1 ? 'login' : 'logins'}`;
      $('#revoke').disabled = session.devices === 0;
    }
  } catch (error) { $('#account-status').textContent = error.message; }
}
async function action(path, body = {}) {
  const response = await fetch('/auth/' + path, { method: 'POST',
    headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': session.csrf }, body: JSON.stringify(body) });
  const result = await response.json();
  if (!response.ok) throw new Error(result.error || 'Could not complete this action.');
}
$('#approve-form').addEventListener('submit', async event => {
  event.preventDefault();
  $('#approve').disabled = true;
  try {
    await action('device-approve', { user_code: $('#device-code').value });
    $('#approve-form').hidden = true;
    $('#action-status').textContent = 'Approved. Return to your terminal to finish connecting, then run cargo leaderboard build.';
    history.replaceState(null, '', '/account.html');
    await load();
  } catch (error) { $('#action-status').textContent = error.message; }
  finally { $('#approve').disabled = false; }
});
$('#sign-out').addEventListener('click', async () => {
  try { await action('logout'); await load(); }
  catch (error) { $('#action-status').textContent = error.message; }
});
$('#revoke').addEventListener('click', async () => {
  $('#revoke').disabled = true;
  try {
    await action('revoke-all');
    $('#action-status').textContent = 'All CLI access revoked. Run cargo leaderboard login to connect again.';
    await load();
  } catch (error) { $('#action-status').textContent = error.message; $('#revoke').disabled = false; }
});
load();
