#!/bin/sh
# Install a checksum-verified GitHub release. Inspect this file before running it.
set -eu
main() {
  version=latest
  bin_dir=${CARGO_HOME:-"$HOME/.cargo"}/bin
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --version|--bin-dir)
        [ "$#" -ge 2 ] || { echo "Missing value for $1" >&2; exit 1; }
        case "$1" in --version) version=$2 ;; --bin-dir) bin_dir=$2 ;; esac
        shift 2 ;;
      *) echo "Usage: sh install.sh [--version v0.2.2] [--bin-dir DIRECTORY]" >&2; exit 1 ;;
    esac
  done
  case "$version" in latest|v[0-9]*) ;; *) echo 'Version must be latest or a v-prefixed release tag.' >&2; exit 1 ;; esac
  case "$version" in *[!a-zA-Z0-9._-]*) echo 'Invalid version.' >&2; exit 1 ;; esac
  for tool in curl tar mktemp; do
    command -v "$tool" >/dev/null 2>&1 || { echo "Missing required tool: $tool" >&2; exit 1; }
  done
  case "$(uname -s)/$(uname -m)" in
    Darwin/arm64|Darwin/aarch64) target=aarch64-apple-darwin ;;
    Darwin/x86_64) target=x86_64-apple-darwin ;;
    Linux/x86_64) target=x86_64-unknown-linux-musl ;;
    Linux/aarch64|Linux/arm64) target=aarch64-unknown-linux-musl ;;
    *) echo 'Unsupported platform. Build from source with cargo install --git https://github.com/alexlwn123/cargo-leaderboard --locked' >&2; exit 1 ;;
  esac
  if command -v sha256sum >/dev/null 2>&1; then hasher=sha256sum
  elif command -v shasum >/dev/null 2>&1; then hasher=shasum
  else echo 'Install sha256sum or shasum to verify downloads.' >&2; exit 1
  fi
  archive="cargo-leaderboard-$target.tar.gz"
  base=https://github.com/alexlwn123/cargo-leaderboard/releases
  if [ "$version" = latest ]; then base="$base/latest/download"; else base="$base/download/$version"; fi
  work=$(mktemp -d)
  staged=
  trap 'result=$?; rm -rf "$work"; if [ -n "$staged" ]; then rm -f "$staged"; fi; exit "$result"' EXIT
  trap 'exit 1' HUP INT TERM
  echo "Downloading $version for ${target}..."
  curl --proto '=https' --tlsv1.2 --fail --show-error --silent --location --retry 3 "$base/$archive" -o "$work/$archive"
  curl --proto '=https' --tlsv1.2 --fail --show-error --silent --location --retry 3 "$base/SHA256SUMS" -o "$work/SHA256SUMS"
  expected=$(awk -v file="$archive" '$2 == file { print $1 }' "$work/SHA256SUMS")
  [ "${#expected}" -eq 64 ] || { echo 'Missing or invalid release checksum.' >&2; exit 1; }
  case "$expected" in *[!0-9a-fA-F]*) echo 'Invalid release checksum.' >&2; exit 1 ;; esac
  if [ "$hasher" = sha256sum ]; then actual=$(sha256sum "$work/$archive" | awk '{print $1}');
  else actual=$(shasum -a 256 "$work/$archive" | awk '{print $1}'); fi
  [ "$actual" = "$expected" ] || { echo 'Checksum mismatch. Nothing was installed.' >&2; exit 1; }
  tar -xzf "$work/$archive" -C "$work" cargo-leaderboard
  chmod +x "$work/cargo-leaderboard"
  "$work/cargo-leaderboard" --version
  mkdir -p "$bin_dir"
  staged=$(mktemp "$bin_dir/.cargo-leaderboard.XXXXXX")
  cp "$work/cargo-leaderboard" "$staged"
  chmod 755 "$staged"
  mv -f "$staged" "$bin_dir/cargo-leaderboard"
  staged=
  echo "Installed to $bin_dir/cargo-leaderboard"
  case ":${PATH:-}:" in
    *":$bin_dir:"*) ;;
    *) echo "Add this directory to PATH, then reopen your terminal: $bin_dir" ;;
  esac
  if ! command -v cargo >/dev/null 2>&1; then
    echo 'Rust is required to measure builds. Install it from https://rustup.rs and reopen your terminal.'
  fi
  echo 'Next: cargo leaderboard setup'
  echo 'Update later by running this installer again. Your saved nickname is preserved.'
}
main "$@"
