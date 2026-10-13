#!/usr/bin/env bash
# Install stack-stat from a checksum-verified GitHub Release.
# STACK_STAT_VERSION pins a stable vX.Y.Z tag; INSTALL_DIR defaults to ~/.local/bin.
set -euo pipefail

REPO=cam1se/stack-stat
BINARY=stack-stat
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
case "$INSTALL_DIR" in
  /*) ;;
  ~*) INSTALL_DIR="${HOME}${INSTALL_DIR#\~}" ;;
  *) INSTALL_DIR="$PWD/$INSTALL_DIR" ;;
esac

case "$(uname -s)/$(uname -m)" in
  Darwin/arm64) TARGET=aarch64-apple-darwin; SUM='shasum -a 256' ;;
  Darwin/x86_64) TARGET=x86_64-apple-darwin; SUM='shasum -a 256' ;;
  Linux/aarch64) TARGET=aarch64-unknown-linux-gnu; SUM=sha256sum ;;
  Linux/x86_64) TARGET=x86_64-unknown-linux-gnu; SUM=sha256sum ;;
  *) echo 'error: unsupported platform; use scripts/install.ps1 on Windows' >&2; exit 1 ;;
esac

TAG="${STACK_STAT_VERSION:-}"
if [[ -z "$TAG" ]]; then
  TAG=$(gh release list --repo "$REPO" --limit 1000 --json tagName,isDraft --jq '[.[] | select(.isDraft == false) | .tagName | select(test("^v[0-9]+\\.[0-9]+\\.[0-9]+$"))] | sort_by(ltrimstr("v") | split(".") | map(tonumber)) | last // empty')
fi
[[ "$TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "error: no stable release found (set STACK_STAT_VERSION=vX.Y.Z)" >&2; exit 1; }
VERSION="${TAG#v}"
ARCHIVE="${BINARY}-${VERSION}-${TARGET}.tar.gz"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
gh release download "$TAG" --repo "$REPO" --pattern "$ARCHIVE" --pattern checksums.sha256 --dir "$WORK" --clobber
(
  cd "$WORK"
  grep -F " $ARCHIVE" checksums.sha256 | $SUM --check
  tar xzf "$ARCHIVE"
)
mkdir -p "$INSTALL_DIR"
install -m 755 "$WORK/${BINARY}-${VERSION}-${TARGET}/${BINARY}" "$INSTALL_DIR/${BINARY}"
echo "Installed $INSTALL_DIR/$BINARY ($VERSION)"
"$INSTALL_DIR/$BINARY" --version
