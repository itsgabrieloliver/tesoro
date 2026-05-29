#!/usr/bin/env sh
set -eu

REPO="itsgabrieloliver/tesoro"
BIN_NAME="teso"
VERSION="${TESORO_VERSION:-latest}"

red()    { printf '\033[31m%s\033[0m\n' "$*"; }
dim()    { printf '\033[2m%s\033[0m\n' "$*"; }
bold()   { printf '\033[1m%s\033[0m\n' "$*"; }

die() { red "error: $*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

uname_s=$(uname -s)
uname_m=$(uname -m)

case "$uname_s" in
    Darwin)
        case "$uname_m" in
            arm64|aarch64) target="aarch64-apple-darwin" ;;
            *) die "tesoro currently ships aarch64-apple-darwin only on macOS. You're on '$uname_m'. Build from source: cargo install --git https://github.com/$REPO" ;;
        esac ;;
    Linux)
        case "$uname_m" in
            x86_64|amd64) target="x86_64-unknown-linux-gnu" ;;
            aarch64|arm64) target="aarch64-unknown-linux-gnu" ;;
            *) die "unsupported linux arch: $uname_m" ;;
        esac ;;
    *) die "unsupported OS: $uname_s. Build from source: cargo install --git https://github.com/$REPO" ;;
esac

if [ "$VERSION" = "latest" ]; then
    base_url="https://github.com/$REPO/releases/latest/download"
else
    base_url="https://github.com/$REPO/releases/download/$VERSION"
fi

tarball="tesoro-$target.tar.gz"
url="$base_url/$tarball"
sums_url="$base_url/$tarball.sha256"

if have curl; then
    dl() { curl -fSL --proto '=https' --tlsv1.2 -o "$2" "$1"; }
elif have wget; then
    dl() { wget --https-only --quiet -O "$2" "$1"; }
else
    die "need curl or wget on PATH"
fi

if have shasum; then
    verify() { ( cd "$(dirname "$1")" && shasum -a 256 -c "$(basename "$1")" >/dev/null ); }
elif have sha256sum; then
    verify() { ( cd "$(dirname "$1")" && sha256sum -c "$(basename "$1")" >/dev/null ); }
else
    verify() { dim "  (skipping sha256 verify, no shasum/sha256sum found)"; }
fi

tmpdir=$(mktemp -d 2>/dev/null || mktemp -d -t tesoro)
trap 'rm -rf "$tmpdir"' EXIT INT TERM

bold "Installing tesoro ($target, $VERSION)"
dim  "  $url"

dl "$url" "$tmpdir/$tarball"
if dl "$sums_url" "$tmpdir/$tarball.sha256" 2>/dev/null; then
    verify "$tmpdir/$tarball.sha256"
fi

tar -xzf "$tmpdir/$tarball" -C "$tmpdir"
[ -f "$tmpdir/$BIN_NAME" ] || die "extracted archive does not contain '$BIN_NAME'"
chmod +x "$tmpdir/$BIN_NAME"

if [ "$uname_s" = Darwin ]; then
    xattr -d com.apple.quarantine "$tmpdir/$BIN_NAME" 2>/dev/null || true
fi

if [ -n "${TESORO_INSTALL_DIR:-}" ]; then
    install_dir="$TESORO_INSTALL_DIR"
elif [ -w /usr/local/bin ]; then
    install_dir="/usr/local/bin"
elif [ -d "$HOME/.local/bin" ] || mkdir -p "$HOME/.local/bin" 2>/dev/null; then
    install_dir="$HOME/.local/bin"
else
    die "no writable install directory found. Set TESORO_INSTALL_DIR=/path/to/bin and re-run."
fi

mv "$tmpdir/$BIN_NAME" "$install_dir/$BIN_NAME"
bold "Installed $install_dir/$BIN_NAME"

case ":$PATH:" in
    *":$install_dir:"*) ;;
    *) dim "  $install_dir is not on \$PATH. Add it to your shell rc, or run with the full path." ;;
esac

dim "  run: $BIN_NAME ~/path/to/your/vault"
