const $ = selector => document.querySelector(selector);
let session;
let refresh;
const requestedCode = new URL(location.href).searchParams.get('code');
let code = requestedCode && /^[a-f0-9]{10}$/i.test(requestedCode) ? requestedCode.toUpperCase() : null;
let approved = false;
if (code) {
  $('#device-code').value = code.slice(0, 5) + '-' + code.slice(5);
  $('#sign-in').href = '/auth/login?user_code=' + encodeURIComponent(code);
}
function render() {
  const user = session?.user;
  $('#signed-out').hidden = !code || !!user;
  $('#signed-in').hidden = !user;
  $('#approve-form').hidden = !code || !user;
  $('#account-title').textContent = approved ? 'Connection approved.' : code ? 'Connect this CLI.' : 'Your GitHub account.';
  $('#account-intro').textContent = approved ? 'Return to your agent or terminal to continue with your first build.' : code ?
    'Sign in with GitHub to continue. You’ll review the connection before approving it.' :
    'GitHub sign-in starts from your CLI.';
  $('#account-status').textContent = user ? `Signed in as @${user.github_login}` : !session ? 'Checking your login…' : 'You’re not signed in.';
  if (!code && user && !approved) {
    $('#account-intro').textContent = 'Manage your connected CLIs and account access.';
  }
  if (code && user) $('#account-intro').textContent = 'Allow this CLI to submit measurements under your GitHub account.';
  if (session?.local) {
    $('#account-intro').textContent = 'GitHub sign-in is not required for a local leaderboard.';
    $('#account-status').textContent = 'Using a local leaderboard.';
  }
}
render();
async function load() {
  clearTimeout(refresh);
  try {
    const res = await fetch('/auth/session', { cache: 'no-store' });
    if (!res.ok) throw new Error('Could not check your login. Refresh to try again.');
    session = await res.json();
    if (session.local) {
      code = null;
      render();
      return;
    }
    render();
    if (session.user) {
      $('#device-count').textContent = `${session.devices} active CLI ${session.devices === 1 ? 'login' : 'logins'}` +
        (session.pending_devices ? ` · ${session.pending_devices} waiting to connect` : '');
      $('#revoke').disabled = session.devices === 0 && !session.pending_devices;
      if (session.pending_devices) refresh = setTimeout(load, 5000);
    }
  } catch (error) {
    $('#account-card').hidden = false;
    $('#signed-out').hidden = true;
    $('#account-status').textContent = error.message;
  }
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
    code = null;
    approved = true;
    $('#action-status').textContent = 'Approved. Return to your agent to continue, or run cargo leaderboard benchmark once your terminal confirms login.';
    history.replaceState(null, '', '/account.html');
    await load();
  } catch (error) { $('#action-status').textContent = error.message; }
  finally { $('#approve').disabled = false; }
});
$('#sign-out').addEventListener('click', async () => {
  try { await action('logout'); approved = false; await load(); }
  catch (error) { $('#action-status').textContent = error.message; }
});
$('#revoke').addEventListener('click', async () => {
  $('#revoke').disabled = true;
  try {
    await action('revoke-all');
    approved = false;
    $('#action-status').textContent = 'All CLI access revoked. Run cargo leaderboard login to connect again.';
    await load();
  } catch (error) { $('#action-status').textContent = error.message; $('#revoke').disabled = false; }
});
load();
