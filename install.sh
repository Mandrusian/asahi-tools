#!/bin/sh
# asahi-tools installer
# usage: curl -fsSL <url>/install.sh | sh
#        curl -fsSL <url>/install.sh | sh -s -- --sudoers
#        sh install.sh --source              # build from git instead
#        sh install.sh --uninstall

set -eu

REPO="Mandrusian/asahi-tools"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/share/asahi-tools}"
BIN_DIR="${BIN_DIR:-$HOME/.local/bin}"
VERSION_ARG="latest"
DO_SUDOERS=0
DO_UNINSTALL=0
FROM_SOURCE=0
TOOLS="asahi-tui kbd-backlight battery-limiter net-restarter sys-monitor sysmon-daemon"

while [ $# -gt 0 ]; do
  case "$1" in
    --sudoers)   DO_SUDOERS=1 ;;
    --uninstall) DO_UNINSTALL=1 ;;
    --source)    FROM_SOURCE=1 ;;
    --version)   shift; VERSION_ARG="${1:?--version needs a value}" ;;
    --bin-dir)   shift; BIN_DIR="${1:?--bin-dir needs a value}" ;;
    --install-dir) shift; INSTALL_DIR="${1:?--install-dir needs a value}" ;;
    -h|--help)
      sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) echo "unknown option: $1" >&2; exit 1 ;;
  esac
  shift
done

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || { echo "ERROR: $1 not found" >&2; exit 1; }
}

kill_daemon() {
  pkill -f "$INSTALL_DIR/bin/sysmon-daemon" 2>/dev/null || true
  sleep 0.2
}

# ---------- uninstall ----------
if [ "$DO_UNINSTALL" = "1" ]; then
  kill_daemon
  for b in $TOOLS; do rm -f "$BIN_DIR/$b"; done
  rm -rf "$INSTALL_DIR"
  sudo rm -f /etc/sudoers.d/asahi-tools 2>/dev/null || true
  echo "✓ uninstalled asahi-tools"
  echo "  (you may want to remove the PATH line from your shell rc)"
  exit 0
fi

# ---------- arch ----------
arch=$(uname -m)
case "$arch" in
  aarch64|arm64) TARGET="aarch64-unknown-linux-gnu" ;;
  *)
    echo "unsupported architecture: $arch" >&2
    echo "asahi-tools is for Apple Silicon (aarch64) on Asahi Linux." >&2
    exit 1
    ;;
esac

# ---------- version ----------
if [ "$VERSION_ARG" = "latest" ]; then
  need_cmd curl
  api="https://api.github.com/repos/$REPO/releases/latest"
  VERSION_TAG=$(curl -fsSL "$api" 2>/dev/null | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -1)
  if [ -z "${VERSION_TAG:-}" ]; then
    echo "no published release found; falling back to --source"
    FROM_SOURCE=1
  fi
else
  case "$VERSION_ARG" in
    v*) VERSION_TAG="$VERSION_ARG" ;;
    *)  VERSION_TAG="v$VERSION_ARG" ;;
  esac
fi

# ---------- fetch ----------
tmp=$(mktemp -d 2>/dev/null || mktemp -d -t asahi-tools)
trap 'rm -rf "$tmp"' EXIT

extract_dir=""
if [ "$FROM_SOURCE" = "0" ]; then
  need_cmd curl
  need_cmd tar
  asset="asahi-tools-${VERSION_TAG}-${TARGET}.tar.gz"
  url="https://github.com/$REPO/releases/download/${VERSION_TAG}/${asset}"
  echo "→ downloading $url"
  if curl -fsSL "$url" -o "$tmp/$asset"; then
    if curl -fsSL "$url.sha256" -o "$tmp/$asset.sha256" 2>/dev/null; then
      ( cd "$tmp" && sha256sum -c "$asset.sha256" >/dev/null 2>&1 ) \
        && echo "  ✓ checksum ok" \
        || { echo "  ✗ checksum failed" >&2; exit 1; }
    fi
    tar -xzf "$tmp/$asset" -C "$tmp"
    extract_dir="$tmp/asahi-tools-${VERSION_TAG}-${TARGET}"
  else
    echo "  binary release not available; falling back to source build"
    FROM_SOURCE=1
  fi
fi

if [ "$FROM_SOURCE" = "1" ]; then
  need_cmd git
  need_cmd cargo
  echo "→ cloning $REPO"
  git clone --depth 1 "https://github.com/$REPO" "$tmp/src" >/dev/null 2>&1
  echo "→ building (a few minutes)"
  ( cd "$tmp/src" && cargo build --release --workspace --quiet )
  extract_dir="$tmp/extracted"
  mkdir -p "$extract_dir/bin"
  for b in $TOOLS; do cp "$tmp/src/target/release/$b" "$extract_dir/bin/$b"; done
fi

# ---------- install ----------
mkdir -p "$INSTALL_DIR" "$BIN_DIR"

kill_daemon

if [ -d "$INSTALL_DIR/bin" ]; then
  rm -rf "$INSTALL_DIR/bin.prev"
  mv "$INSTALL_DIR/bin" "$INSTALL_DIR/bin.prev"
fi

if ! mv "$extract_dir/bin" "$INSTALL_DIR/bin"; then
  [ -d "$INSTALL_DIR/bin.prev" ] && mv "$INSTALL_DIR/bin.prev" "$INSTALL_DIR/bin"
  echo "ERROR: install failed; previous version restored" >&2
  exit 1
fi
rm -rf "$INSTALL_DIR/bin.prev"
chmod +x "$INSTALL_DIR"/bin/*

for b in $TOOLS; do ln -sf "$INSTALL_DIR/bin/$b" "$BIN_DIR/$b"; done
echo "✓ installed $VERSION_TAG to $INSTALL_DIR"
echo "  symlinks in $BIN_DIR"

# ---------- PATH ----------
ensure_path() {
  shell=$(basename "${SHELL:-/bin/bash}")
  case "$shell" in
    fish)
      cfg="$HOME/.config/fish/config.fish"
      mkdir -p "$(dirname "$cfg")"
      grep -Fq "$BIN_DIR" "$cfg" 2>/dev/null && return
      printf '\n# asahi-tools\nfish_add_path %s\n' "$BIN_DIR" >> "$cfg"
      echo "  → added $BIN_DIR to $cfg"
      ;;
    zsh)
      cfg="${ZDOTDIR:-$HOME}/.zshrc"
      grep -Fq "$BIN_DIR" "$cfg" 2>/dev/null && return
      printf '\n# asahi-tools\nexport PATH="%s:$PATH"\n' "$BIN_DIR" >> "$cfg"
      echo "  → added $BIN_DIR to $cfg"
      ;;
    bash|sh)
      cfg="$HOME/.bashrc"
      grep -Fq "$BIN_DIR" "$cfg" 2>/dev/null && return
      printf '\n# asahi-tools\nexport PATH="%s:$PATH"\n' "$BIN_DIR" >> "$cfg"
      echo "  → added $BIN_DIR to $cfg"
      ;;
    *)
      echo "  ⚠ unknown shell ($shell); add manually: export PATH=\"$BIN_DIR:\$PATH\""
      ;;
  esac
}
ensure_path

# ---------- sudoers (optional) ----------
if [ "$DO_SUDOERS" = "1" ]; then
  need_cmd sudo
  user=$(id -un)
  tmp_sudo=$(mktemp)
  {
    echo "$user ALL=(ALL) NOPASSWD: /usr/sbin/modprobe"
    for b in kbd-backlight battery-limiter net-restarter sys-monitor sysmon-daemon; do
      echo "$user ALL=(ALL) NOPASSWD: $INSTALL_DIR/bin/$b"
    done
  } > "$tmp_sudo"
  if sudo install -m 440 -o root -g root "$tmp_sudo" /etc/sudoers.d/asahi-tools \
     && sudo visudo -c >/dev/null 2>&1; then
    echo "✓ sudoers rules installed at /etc/sudoers.d/asahi-tools"
  else
    echo "⚠ sudoers install failed; the TUI will prompt for a password on launch" >&2
  fi
  rm -f "$tmp_sudo"
fi

echo
echo "done. open a new shell (or run: hash -r) and try:"
echo "  asahi-tui"
