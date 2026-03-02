#!/usr/bin/env bash
#
# Install arc from GitHub Releases.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/morten-olsen/arc/main/install.sh | bash
#
set -euo pipefail

REPO="morten-olsen/arc"
INSTALL_DIR="$HOME/.arc/bin"

# --- Detect platform ---

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin) ;;
  Linux)  ;;
  *) echo "Unsupported OS: $OS" >&2; exit 1 ;;
esac

case "$ARCH" in
  arm64|aarch64) ARCH="aarch64" ;;
  x86_64)        ARCH="x86_64"  ;;
  *) echo "Unsupported architecture: $ARCH" >&2; exit 1 ;;
esac

case "$OS" in
  Darwin) TARGET="${ARCH}-apple-darwin" ;;
  Linux)  TARGET="${ARCH}-unknown-linux-gnu" ;;
esac

echo "Detected platform: ${TARGET}"

# --- Fetch latest release tag ---

TAG="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed 's/.*: "\(.*\)".*/\1/')"

if [ -z "$TAG" ]; then
  echo "Failed to fetch latest release." >&2
  exit 1
fi

echo "Latest release: ${TAG}"

# --- Download and verify ---

TARBALL="arc-${TARGET}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${TAG}/${TARBALL}"
SHA_URL="${URL}.sha256"

TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

echo "Downloading ${TARBALL}..."
curl -fsSL -o "${TMPDIR}/${TARBALL}" "$URL"
curl -fsSL -o "${TMPDIR}/${TARBALL}.sha256" "$SHA_URL"

echo "Verifying checksum..."
cd "$TMPDIR"
if command -v sha256sum >/dev/null 2>&1; then
  sha256sum -c "${TARBALL}.sha256"
else
  shasum -a 256 -c "${TARBALL}.sha256"
fi

# --- Install ---

mkdir -p "$INSTALL_DIR"
tar xzf "${TARBALL}" -C "$INSTALL_DIR"
chmod +x "${INSTALL_DIR}/arc"

echo ""
echo "Installed arc to ${INSTALL_DIR}/arc"

# --- PATH instructions ---

case ":${PATH}:" in
  *":${INSTALL_DIR}:"*)
    echo "arc is already on your PATH."
    ;;
  *)
    SHELL_NAME="$(basename "$SHELL")"
    case "$SHELL_NAME" in
      zsh)  PROFILE="$HOME/.zshrc" ;;
      bash) PROFILE="$HOME/.bashrc" ;;
      *)    PROFILE="" ;;
    esac

    LINE="export PATH=\"${INSTALL_DIR}:\$PATH\""

    if [ -n "$PROFILE" ] && [ -w "$PROFILE" ]; then
      echo "$LINE" >> "$PROFILE"
      echo "Added ${INSTALL_DIR} to PATH in ${PROFILE}"
      echo "Run 'source ${PROFILE}' or open a new terminal to use arc."
    else
      echo "Add the following to your shell profile:"
      echo ""
      echo "  ${LINE}"
    fi
    ;;
esac
