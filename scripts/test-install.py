"""Exercise the real Unix installer using an isolated release mirror and binary."""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile

binary = Path(sys.argv[1]).resolve()
installer = Path('public/install.sh').resolve()
with tempfile.TemporaryDirectory(prefix='leaderboard install ') as directory:
    root = Path(directory)
    mirror, mocks, dest = root / 'mirror', root / 'mocks', root / 'bin with spaces'
    mirror.mkdir()
    mocks.mkdir()
    # Redirect curl to local release files, preserving actual archive/hash/install behavior.
    curl = mocks / 'curl'
    curl.write_text('''#!/bin/sh
while [ "$#" -gt 0 ]; do
  case "$1" in https://*) url=$1 ;; -o) shift; output=$1 ;; esac
  shift
done
cp "$INSTALL_TEST_MIRROR/${url##*/}" "$output"
''')
    curl.chmod(0o755)
    platform = ('apple-darwin' if sys.platform == 'darwin' else 'unknown-linux-musl')
    arch = 'aarch64' if os.uname().machine in ('arm64', 'aarch64') else 'x86_64'
    archive = mirror / f'cargo-leaderboard-{arch}-{platform}.tar.gz'
    with tarfile.open(archive, 'w:gz') as bundle:
        bundle.add(binary, arcname='cargo-leaderboard')
    checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
    sums = mirror / 'SHA256SUMS'
    sums.write_text(f'{checksum}  {archive.name}\n')
    env = {**os.environ, 'PATH': str(mocks) + os.pathsep + os.environ['PATH'],
           'INSTALL_TEST_MIRROR': str(mirror), 'CARGO_LEADERBOARD_CONFIG_DIR': str(root / 'config')}
    for key in ('CARGO_LEADERBOARD_API_URL', 'CARGO_LEADERBOARD_NICKNAME', 'CARGO_LEADERBOARD_TOKEN'):
        env.pop(key, None)
    command = ['sh', str(installer), '--version', 'v0.2.0', '--bin-dir', str(dest)]
    subprocess.run(command, env=env, check=True)
    installed = dest / 'cargo-leaderboard'
    subprocess.run([str(installed), 'setup', '--nickname', 'installer-test'], env=env, check=True)
    config = (root / 'config/config.json').read_bytes()
    subprocess.run(command, env=env, check=True)  # Upgrade preserves setup.
    assert (root / 'config/config.json').read_bytes() == config
    original = installed.read_bytes()
    sums.write_text(f'{"0" * 64}  {archive.name}\n')
    assert subprocess.run(command, env=env).returncode != 0
    assert installed.read_bytes() == original
    sums.unlink()
    assert subprocess.run(command, env=env).returncode != 0
    assert installed.read_bytes() == original
    assert not list(dest.glob('.cargo-leaderboard.*'))
print('Installer: fresh install, upgrade, saved setup, corrupt/missing checksum, paths with spaces passed.')
