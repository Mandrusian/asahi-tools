#!/bin/bash
# asahi-tools installer
#
# Interactive TUI:
#   bash install.sh
#
# Non-interactive:
#   bash install.sh --yes --with tui,gui,daemons,scripts,desktop,services
#   bash install.sh --dry-run [--yes] [...]
#
# Flags:
#   --yes                skip confirmations, use defaults
#   --with LIST          comma-separated components (non-interactive)
#   --source             build from current directory or git clone
#   --version vX.Y.Z     (reserved for release download)
#   --install-dir DIR    default: ~/.local/share/asahi-tools
#   --bin-dir DIR        default: ~/.local/bin
#   --dry-run            print actions, do nothing
#   --uninstall          remove everything
#   --no-color           disable ANSI colors
#   -h|--help

set -euo pipefail

VERSION="0.2.0"
REPO="Mandrusian/asahi-tools"

INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/share/asahi-tools}"
BIN_DIR="${BIN_DIR:-$HOME/.local/bin}"
WITH=""
DO_SUDOERS_EXPLICIT=0
FROM_SOURCE=0
UNINSTALL=0
ASSUME_YES=0
DRY_RUN=0
UPGRADE=0
NO_COLOR=0
VERSION_ARG="latest"

# ── arg parsing ────────────────────────────────────────────
while [ $# -gt 0 ]; do
    case "$1" in
        --yes)         ASSUME_YES=1 ;;
        --with)        shift; WITH="${1:?}" ;;
        --source)      FROM_SOURCE=1 ;;
        --version)     shift; VERSION_ARG="${1:?}" ;;
        --install-dir) shift; INSTALL_DIR="${1:?}" ;;
        --bin-dir)     shift; BIN_DIR="${1:?}" ;;
        --dry-run)     DRY_RUN=1 ;;
        --upgrade)     UPGRADE=1 ;;
        --uninstall)   UNINSTALL=1 ;;
        --no-color)    NO_COLOR=1 ;;
        -h|--help)     sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "unknown: $1 (use --help)" >&2; exit 1 ;;
    esac
    shift
done

# ── colors ─────────────────────────────────────────────────
if [ "$NO_COLOR" = "1" ] || [ ! -t 1 ] || [ "${TERM:-dumb}" = "dumb" ]; then
    C_RESET=""; C_DIM=""; C_BOLD=""; C_RED=""; C_GREEN=""; C_YELLOW=""; C_BLUE=""; C_CYAN=""; C_REVERSE=""
else
    C_RESET=$'\e[0m'
    C_DIM=$'\e[2m'
    C_BOLD=$'\e[1m'
    C_RED=$'\e[31m'
    C_GREEN=$'\e[32m'
    C_YELLOW=$'\e[33m'
    C_BLUE=$'\e[34m'
    C_CYAN=$'\e[36m'
    C_REVERSE=$'\e[7m'
fi

# ── logging ────────────────────────────────────────────────
log()      { printf '%s\n' "$*"; }
log_dim()  { printf '%s%s%s\n' "$C_DIM" "$*" "$C_RESET"; }
log_ok()   { printf '%s✓%s %s\n' "$C_GREEN" "$C_RESET" "$*"; }
log_warn() { printf '%s⚠%s %s\n' "$C_YELLOW" "$C_RESET" "$*" >&2; }
log_err()  { printf '%s✗%s %s\n' "$C_RED" "$C_RESET" "$*" >&2; }
die()      { log_err "$*"; exit 1; }

# ── dry-run primitives ─────────────────────────────────────
# All state-changing actions go through these so --dry-run covers everything.

do_cmd() {
    # run a command, respecting dry-run
    if [ "$DRY_RUN" = "1" ]; then
        printf '  %s[dry-run]%s %s\n' "$C_DIM" "$C_RESET" "$*"
    else
        "$@"
    fi
}

do_sudo() {
    if [ "$DRY_RUN" = "1" ]; then
        printf '  %s[dry-run]%s sudo %s\n' "$C_DIM" "$C_RESET" "$*"
    else
        sudo "$@"
    fi
}

do_write() {
    # do_write PATH CONTENT
    local path="$1" content="$2"
    if [ "$DRY_RUN" = "1" ]; then
        printf '  %s[dry-run]%s write %s (%d bytes)\n' "$C_DIM" "$C_RESET" "$path" "${#content}"
    else
        mkdir -p "$(dirname "$path")"
        printf '%s\n' "$content" > "$path"
    fi
}

do_mkdir() {
    if [ "$DRY_RUN" = "1" ]; then
        printf '  %s[dry-run]%s mkdir -p %s\n' "$C_DIM" "$C_RESET" "$*"
    else
        mkdir -p "$@"
    fi
}

do_copy() {
    # do_copy SRC DST
    if [ "$DRY_RUN" = "1" ]; then
        printf '  %s[dry-run]%s copy %s → %s\n' "$C_DIM" "$C_RESET" "$1" "$2"
    else
        mkdir -p "$(dirname "$2")"
        cp -f "$1" "$2"
    fi
}

do_symlink() {
    if [ "$DRY_RUN" = "1" ]; then
        printf '  %s[dry-run]%s ln -sf %s %s\n' "$C_DIM" "$C_RESET" "$1" "$2"
    else
        mkdir -p "$(dirname "$2")"
        ln -sf "$1" "$2"
    fi
}

# ── TUI framework ──────────────────────────────────────────
TUI_ACTIVE=0

tui_init() {
    [ "$TUI_ACTIVE" = "1" ] && return
    [ -t 0 ] || return   # not a tty
    TUI_ACTIVE=1
    stty -echo -icanon 2>/dev/null || true
    printf '%s' $'\e[?25l'   # hide cursor
    trap 'tui_cleanup' EXIT INT TERM
}

tui_cleanup() {
    if [ "$TUI_ACTIVE" != "1" ]; then
        return 0
    fi
    TUI_ACTIVE=0
    printf '%s' $'\e[?25h'   # show cursor
    stty echo icanon 2>/dev/null || true
    tput rmcup 2>/dev/null || true
    return 0
}

tui_clear() { printf '%s' $'\e[2J\e[H'; }
tui_goto()  { printf '\e[%d;%dH' "$1" "$2"; }

# read_key: returns one of:
#   up, down, left, right, enter, space, q, y, n, esc, or a literal char
read_key() {
    local k rest
    IFS= read -rsn1 k || { echo "esc"; return; }
    if [[ "$k" == $'\x1b' ]]; then
        IFS= read -rsn2 -t 0.05 rest 2>/dev/null || true
        case "$k$rest" in
            $'\x1b[A') echo up; return ;;
            $'\x1b[B') echo down; return ;;
            $'\x1b[C') echo right; return ;;
            $'\x1b[D') echo left; return ;;
            $'\x1b[Z') echo backtab; return ;;
            $'\x1b')   echo esc; return ;;
            *)         echo "esc"; return ;;
        esac
    fi
    if [ -z "$k" ]; then echo enter; return; fi
    case "$k" in
        ' ') echo space ;;
        *)   echo "$k" ;;
    esac
}

draw_header() {
    local title="$1"
    local w=64
    tui_clear
    printf '%s╭─ %s%s%s %s╮%s\n' \
        "$C_CYAN" "$C_BOLD" "$title" "$C_RESET$C_CYAN" \
        "$(printf '─%.0s' $(seq 1 $((w - ${#title} - 3))))" "$C_RESET"
}

draw_footer() {
    local help="$1"
    printf '%s%s%s\n' "$C_DIM" "$help" "$C_RESET"
}

# single_select: vertical menu, arrow keys, enter to confirm
# usage: single_select "title" "var_name" item1 item2 ...
single_select() {
    local title="$1"
    local __result_var="$2"
    shift 2
    local items=("$@")
    local n=${#items[@]}
    local sel=0

    tui_init
    while true; do
        draw_header "$title"
        printf '\n'
        local i
        for i in "${!items[@]}"; do
            if [ "$i" = "$sel" ]; then
                printf '  %s▶ %s%s\n' "$C_CYAN" "${items[$i]}" "$C_RESET"
            else
                printf '    %s\n' "${items[$i]}"
            fi
        done
        printf '\n'
        draw_footer "↑/↓ navigate · enter select · q cancel"
        local k
        k=$(read_key)
        case "$k" in
            up)     sel=$(( (sel - 1 + n) % n )) ;;
            down)   sel=$(( (sel + 1) % n )) ;;
            enter)  printf -v "$__result_var" '%s' "$sel"; return 0 ;;
            q|esc)  return 1 ;;
        esac
    done
}

# multi_select: checkboxes, space to toggle, enter to confirm
# usage: multi_select "title" "var_name" default1 default2 ... -- label1 label2 ...
multi_select() {
    local title="$1"
    local __result_var="$2"
    shift 2
    # parse defaults ... -- labels ...
    local defaults=() labels=()
    local in_labels=0
    for arg in "$@"; do
        if [ "$arg" = "--" ]; then in_labels=1; continue; fi
        if [ "$in_labels" = "1" ]; then labels+=("$arg"); else defaults+=("$arg"); fi
    done
    local n=${#labels[@]}
    local checked=("${defaults[@]}")
    local sel=0

    tui_init
    while true; do
        draw_header "$title"
        printf '\n'
        local i
        for i in "${!labels[@]}"; do
            local mark=" "
            [ "${checked[$i]}" = "1" ] && mark="✓"
            local mark_color="$C_DIM"
            [ "${checked[$i]}" = "1" ] && mark_color="$C_GREEN"
            local cursor="  "
            [ "$i" = "$sel" ] && cursor="${C_CYAN}▶ ${C_RESET}"
            printf '%s%s[%s%s%s] %s\n' \
                "$cursor" "" "$mark_color" "$mark" "$C_RESET" "${labels[$i]}"
        done
        printf '\n'
        draw_footer "↑/↓ navigate · space toggle · enter next · q cancel"
        local k
        k=$(read_key)
        case "$k" in
            up)     sel=$(( (sel - 1 + n) % n )) ;;
            down)   sel=$(( (sel + 1) % n )) ;;
            space)  if [ "${checked[$sel]}" = "1" ]; then checked[$sel]=0; else checked[$sel]=1; fi ;;
            enter)
                local _packed_result=""
                for i in "${!checked[@]}"; do
                    _packed_result="${_packed_result}${checked[$i]}"
                done
                printf -v "$__result_var" '%s' "$_packed_result"
                return 0
                ;;
            q|esc)  return 1 ;;
        esac
    done
}

# prompt_yn: returns 0 for yes, 1 for no
prompt_yn() {
    local question="$1"
    local default="${2:-y}"
    tui_init
    while true; do
        tui_clear
        printf '\n  %s\n\n' "$question"
        if [ "$default" = "y" ]; then
            printf '  %s(Y)es%s / (n)o   ' "$C_CYAN" "$C_RESET"
        else
            printf '  (y)es / %s(N)o%s   ' "$C_CYAN" "$C_RESET"
        fi
        local k
        k=$(read_key)
        case "$k" in
            y|Y) return 0 ;;
            n|N) return 1 ;;
            enter) [ "$default" = "y" ] && return 0 || return 1 ;;
            q|esc) return 1 ;;
        esac
    done
}

# ── component definitions ──────────────────────────────────
# Order must match the labels below.
COMP_IDS=(tui gui daemons scripts desktop services sudoers)
COMP_LABELS=(
    "TUI suite        asahi-tui, kbd-backlight, net-restarter, sys-monitor"
    "GTK GUI          asahi-gtk (macOS-style), asahi-power-draw, asahi-sleep-log"
    "Daemons          asahi-charge-daemon, sysmon-daemon"
    "Helper scripts   lid, display, diag, wifi-log, fn-remap, snapshot, sensors"
    "Desktop entry    KDE/Plasma launcher icon"
    "Systemd services enable charge-daemon + sysmon-daemon on boot"
    "Sudoers rules    /etc/sudoers.d/asahi-tools (NOPASSWD for daemons)"
)
COMP_DEFAULTS=(1 1 1 1 1 1 0)

# Map each component to the binaries it owns
comp_bins() {
    case "$1" in
        tui)      echo "asahi-tui net-restarter sys-monitor asahi-battery-health" ;;
        gui)      echo "asahi-gtk asahi-power-draw asahi-sleep-log asahi-notify-hub asahi-kernel-manager" ;;
        daemons)  echo "asahi-charge-daemon sysmon-daemon asahi-priv-daemon asahi-force-click" ;;
        scripts)  echo "" ;;
        desktop)  echo "" ;;
        services) echo "" ;;
        sudoers)  echo "" ;;
    esac
}

# ── actions ────────────────────────────────────────────────
action_build() {
    # $1 = space-separated list of bins to build
    local want="$1"
    log ""
    log "${C_BOLD}Building workspace${C_RESET}"

    if [ "$FROM_SOURCE" = "1" ]; then
        # Build from current dir or clone
        local build_dir="$PWD"
        if [ ! -f "$build_dir/Cargo.toml" ]; then
            local tmp; tmp=$(mktemp -d)
            log_dim "  cloning $REPO → $tmp"
            do_cmd git clone --depth 1 "https://github.com/$REPO" "$tmp/src"
            build_dir="$tmp/src"
        fi
        do_cmd cargo build --release --workspace --manifest-path "$build_dir/Cargo.toml"
        SOURCE_DIR="$build_dir"
    else
        die "release download not implemented yet (use --source)"
    fi
}

action_install_bins() {
    local want="$1"
    log ""
    log "${C_BOLD}Installing binaries to $INSTALL_DIR/bin${C_RESET}"
    do_mkdir "$INSTALL_DIR/bin"
    do_mkdir "$BIN_DIR"
    for b in $want; do
        if [ -f "$SOURCE_DIR/target/release/$b" ]; then
            do_copy "$SOURCE_DIR/target/release/$b" "$INSTALL_DIR/bin/$b"
            do_symlink "$INSTALL_DIR/bin/$b" "$BIN_DIR/$b"
        else
            [ "$DRY_RUN" = "1" ] || log_warn "  missing: $b (not built)"
        fi
    done
}

action_install_scripts() {
    log ""
    log "${C_BOLD}Installing helper scripts${C_RESET}"
    do_mkdir "$INSTALL_DIR/scripts"
    for s in "$SOURCE_DIR"/scripts/*; do
        [ -f "$s" ] || continue
        local name
        name=$(basename "$s")
        [ "$name" = "asahi-kernel-manager" ] && continue  # that's a binary, not a script
        do_copy "$s" "$INSTALL_DIR/scripts/$name"
        do_symlink "$INSTALL_DIR/scripts/$name" "$BIN_DIR/$name"
    done
    [ "$DRY_RUN" = "1" ] || chmod +x "$INSTALL_DIR/scripts/"* 2>/dev/null || true
}

action_install_desktop() {
    log ""
    log "${C_BOLD}Installing desktop entries + icon${C_RESET}"

    local icons="$HOME/.local/share/icons/hicolor/scalable/apps"
    local apps="$HOME/.local/share/applications"
    do_mkdir "$icons"
    do_mkdir "$apps"

    # Icon
    if [ -f "$SOURCE_DIR/assets/asahi-tools.svg" ]; then
        do_copy "$SOURCE_DIR/assets/asahi-tools.svg" "$icons/asahi-tools.svg"
    fi

    # GTK entry
    local gtk_entry="[Desktop Entry]
Type=Application
Name=Asahi Settings
GenericName=System Settings
Comment=Control battery, thermal, power, display, trackpad and services
Exec=$BIN_DIR/asahi-gtk
Icon=asahi-tools
Terminal=false
Categories=System;Settings;HardwareSettings;Monitor;
Keywords=asahi;battery;thermal;fan;trackpad;display;power;
StartupNotify=true
StartupWMClass=asahi-gtk"
    do_write "$apps/asahi-gtk.desktop" "$gtk_entry"

    # TUI entry
    local tui_entry="[Desktop Entry]
Type=Application
Name=Asahi Tools
GenericName=Asahi Launcher
Comment=Launch asahi-tools utilities from a terminal dashboard
Exec=$BIN_DIR/asahi-tui
Icon=asahi-tools
Terminal=true
Categories=System;Monitor;Utility;
Keywords=asahi;tui;dashboard;launcher;"
    do_write "$apps/asahi-tui.desktop" "$tui_entry"

    # Kernel manager entry
    local km_entry="[Desktop Entry]
Type=Application
Name=Asahi Kernel Manager
Comment=Safely manage kernel updates and m1n1 bootloader
Exec=$BIN_DIR/asahi-kernel-manager
Icon=system-software-update
Terminal=true
Categories=System;"
    do_write "$apps/asahi-kernel-manager.desktop" "$km_entry"

    if [ "$DRY_RUN" != "1" ]; then
        update-desktop-database "$apps" 2>/dev/null || true
        gtk-update-icon-cache "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
    fi
}


action_install_services() {
    log ""
    log "${C_BOLD}Installing system services (SELinux-safe: /usr/local/bin)${C_RESET}"

    # SELinux blocks init_t from executing user_home_t binaries. System
    # daemons MUST live under /usr/local/bin (labeled bin_t) to work.
    for d in asahi-charge-daemon sysmon-daemon asahi-priv-daemon asahi-force-click; do
        if [ -f "$INSTALL_DIR/bin/$d" ] || [ "$DRY_RUN" = "1" ]; then
            do_sudo install -m 755 "$INSTALL_DIR/bin/$d" "/usr/local/bin/$d"
        fi
    done

    write_system_unit() {
        local name="$1" desc="$2" bin="$3"
        local svc="[Unit]
Description=$desc
After=multi-user.target

[Service]
Type=simple
ExecStart=/usr/local/bin/$bin
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target"
        if [ "$DRY_RUN" = "1" ]; then
            printf '  %s[dry-run]%s write /etc/systemd/system/%s.service
' "$C_DIM" "$C_RESET" "$name"
        else
            echo "$svc" | sudo tee "/etc/systemd/system/$name.service" >/dev/null
        fi
    }

    write_system_unit "asahi-charge-daemon" "Asahi battery charge controller" "asahi-charge-daemon"
    write_system_unit "asahi-sysmon-daemon" "Asahi fan control daemon" "sysmon-daemon"
    write_system_unit "asahi-priv-daemon" "Asahi privileged operations daemon" "asahi-priv-daemon"
    write_system_unit "asahi-force-click" "Asahi trackpad pressure right click" "asahi-force-click"

    do_sudo systemctl daemon-reload
    do_sudo systemctl enable --now asahi-charge-daemon
    do_sudo systemctl enable --now asahi-sysmon-daemon
    do_sudo systemctl enable --now asahi-priv-daemon
    # asahi-force-click is opt-in — installed but not enabled by default
    # Users enable it via Settings → Services or `systemctl enable asahi-force-click`

    # User services
    local user_svc_dir="$HOME/.config/systemd/user"
    do_mkdir "$user_svc_dir"

    local notify="[Unit]
Description=Asahi notification hub
After=graphical-session.target

[Service]
Type=simple
ExecStart=$INSTALL_DIR/bin/asahi-notify-hub
Restart=on-failure
RestartSec=10

[Install]
WantedBy=default.target"
    do_write "$user_svc_dir/asahi-notify-hub.service" "$notify"

    local lid="[Unit]
Description=Asahi lid close handler
After=graphical-session.target

[Service]
Type=simple
ExecStart=$INSTALL_DIR/scripts/asahi-lid run
Restart=always
RestartSec=5

[Install]
WantedBy=default.target"
    do_write "$user_svc_dir/asahi-lid.service" "$lid"

    if [ "$DRY_RUN" != "1" ]; then
        systemctl --user daemon-reload 2>/dev/null || true
        systemctl --user enable --now asahi-notify-hub 2>/dev/null || true
        systemctl --user enable asahi-lid 2>/dev/null || true
    fi
}


action_install_sudoers() {
    log ""
    log "${C_BOLD}Installing sudoers rules${C_RESET}"
    local user=$(id -un)
    local rules="$user ALL=(ALL) NOPASSWD: /usr/sbin/modprobe
$user ALL=(ALL) NOPASSWD: $INSTALL_DIR/bin/asahi-charge-daemon
$user ALL=(ALL) NOPASSWD: $INSTALL_DIR/bin/sysmon-daemon"

    if [ "$DRY_RUN" = "1" ]; then
        printf '  %s[dry-run]%s write /etc/sudoers.d/asahi-tools\n' "$C_DIM" "$C_RESET"
        printf '  %s[dry-run]%s chmod 440 /etc/sudoers.d/asahi-tools\n' "$C_DIM" "$C_RESET"
        printf '  %s[dry-run]%s visudo -c\n' "$C_DIM" "$C_RESET"
    else
        echo "$rules" | sudo tee /etc/sudoers.d/asahi-tools >/dev/null
        sudo chmod 440 /etc/sudoers.d/asahi-tools
        sudo visudo -c >/dev/null || { log_err "sudoers validation failed"; sudo rm -f /etc/sudoers.d/asahi-tools; return 1; }
    fi
}

action_ensure_path() {
    log ""
    log "${C_BOLD}Checking PATH${C_RESET}"
    case ":$PATH:" in
        *":$BIN_DIR:"*) log_ok "  $BIN_DIR already in PATH" ;;
        *)
            local shell_cfg
            case "$(basename "${SHELL:-bash}")" in
                fish) shell_cfg="$HOME/.config/fish/config.fish" ;;
                zsh)  shell_cfg="${ZDOTDIR:-$HOME}/.zshrc" ;;
                *)    shell_cfg="$HOME/.bashrc" ;;
            esac
            if grep -Fq "$BIN_DIR" "$shell_cfg" 2>/dev/null; then
                log_dim "  $BIN_DIR already referenced in $shell_cfg"
            else
                do_write "$shell_cfg" "
# asahi-tools
case \":\$PATH:\" in *\":$BIN_DIR:\"*) ;; *) export PATH=\"$BIN_DIR:\$PATH\" ;; esac"
                log_ok "  added $BIN_DIR to $shell_cfg"
            fi
            ;;
    esac
}

action_uninstall() {
    log ""
    log "${C_BOLD}Uninstalling asahi-tools${C_RESET}"

    do_sudo systemctl disable --now asahi-charge-daemon 2>/dev/null || true
    do_sudo systemctl disable --now asahi-sysmon-daemon 2>/dev/null || true
    systemctl --user disable --now asahi-notify-hub 2>/dev/null || true
    systemctl --user disable --now asahi-lid 2>/dev/null || true

    do_sudo rm -f /etc/systemd/system/asahi-charge-daemon.service
    do_sudo rm -f /etc/systemd/system/asahi-sysmon-daemon.service
    do_sudo rm -f /etc/sudoers.d/asahi-tools

    for f in asahi-tui asahi-gtk asahi-power-draw asahi-sleep-log \
             asahi-notify-hub asahi-kernel-manager asahi-battery-health \
             asahi-priv-daemon asahi-force-click asahi-trackpad-viz \
             net-restarter sys-monitor sysmon-daemon asahi-charge-daemon \
             asahi-lid asahi-display asahi-diag \
             asahi-wifi-log asahi-kbd-fn-remap asahi-snapshot asahi-sensor-explorer; do
        do_cmd rm -f "$BIN_DIR/$f"
    done

    do_cmd rm -rf "$INSTALL_DIR"

    log_ok "uninstalled"
}

# ── interactive flow ───────────────────────────────────────
interactive_flow() {
    # Welcome
    tui_init
    draw_header "asahi-tools installer v$VERSION"
    cat <<EOF

  ${C_BOLD}Welcome${C_RESET}

  This installer will set up the asahi-tools suite on your Apple Silicon
  Mac running Asahi Linux.

  Detected:
    ${C_CYAN}arch${C_RESET}        $(uname -m)
    ${C_CYAN}distro${C_RESET}      $(grep PRETTY_NAME /etc/os-release 2>/dev/null | cut -d= -f2- | tr -d '"' || echo unknown)
    ${C_CYAN}install dir${C_RESET} $INSTALL_DIR
    ${C_CYAN}bin dir${C_RESET}     $BIN_DIR

EOF
    draw_footer "press enter to continue · q to quit"
    local k; k=$(read_key)
    case "$k" in q|esc) exit 0 ;; esac

    # Components
    local packed=""
    multi_select "Select components" packed \
        "${COMP_DEFAULTS[@]}" -- "${COMP_LABELS[@]}" || { log "cancelled"; exit 0; }

    local -a SELECTED=()
    local i
    for i in "${!COMP_IDS[@]}"; do
        [ "${packed:$i:1}" = "1" ] && SELECTED+=("${COMP_IDS[$i]}")
    done
    [ "${#SELECTED[@]}" -gt 0 ] || { log_err "nothing selected"; exit 1; }

    # Build source choice (skip if user passed --source)
    if [ "$FROM_SOURCE" = "0" ]; then
        local build_choice=""
        single_select "Source" build_choice \
            "Build from source (requires cargo, git)" \
            "Download release (not yet implemented)" || exit 0
        if [ "$build_choice" = "0" ]; then FROM_SOURCE=1
        else log_err "release download not implemented — use source for now"; exit 1
        fi
    fi

    # Summary + confirm
    tui_clear
    draw_header "Summary"
    printf '\n  %sWill install these components:%s\n' "$C_BOLD" "$C_RESET"
    for c in "${SELECTED[@]}"; do
        printf '    %s✓%s %s\n' "$C_GREEN" "$C_RESET" "$c"
    done
    printf '\n  %sFilesystem:%s\n' "$C_BOLD" "$C_RESET"
    printf '    install dir  %s\n' "$INSTALL_DIR"
    printf '    bin dir      %s\n' "$BIN_DIR"
    if [[ " ${SELECTED[*]} " =~ " sudoers " ]]; then
        printf '    sudoers      /etc/sudoers.d/asahi-tools\n'
    fi
    if [[ " ${SELECTED[*]} " =~ " services " ]]; then
        printf '    systemd      /etc/systemd/system/asahi-*.service\n'
    fi
    printf '\n'
    draw_footer "enter install · q cancel"
    local k2=""; k2=$(read_key)
    case "$k2" in
        q|esc) log "cancelled"; exit 0 ;;
        enter) : ;;
        *)     : ;;
    esac

    execute_plan "${SELECTED[@]}"
}

# ── execute plan ───────────────────────────────────────────
execute_plan() {
    local -a comps=("$@")

    # SOURCE_DIR defaults to current dir; action_build overrides if it clones.
    SOURCE_DIR="$PWD"

    tui_cleanup
    tui_clear

    local want_bins=""
    local want_scripts=0
    local want_desktop=0
    local want_services=0
    local want_sudoers=0
    local c

    for c in "${comps[@]}"; do
        case "$c" in
            scripts)  want_scripts=1 ;;
            desktop)  want_desktop=1 ;;
            services) want_services=1 ;;
            sudoers)  want_sudoers=1 ;;
            *)        want_bins="$want_bins $(comp_bins "$c")" ;;
        esac
    done

    if [ -n "${want_bins// /}" ]; then
        action_build "$want_bins"
        action_install_bins "$want_bins"
    fi
    if [ "$want_scripts" = "1" ];  then action_install_scripts;  fi
    if [ "$want_desktop" = "1" ];  then action_install_desktop;  fi
    if [ "$want_services" = "1" ]; then action_install_services; fi
    if [ "$want_sudoers" = "1" ];  then action_install_sudoers;  fi
    action_ensure_path

    log ""
    if [ "$DRY_RUN" = "1" ]; then
        log_warn "dry run — nothing was changed"
    else
        log_ok "install complete"
        if [ -d "$INSTALL_DIR" ]; then
            echo "0.3.0" > "$INSTALL_DIR/VERSION"
        fi
        log ""
        log "  Try:  ${C_BOLD}asahi-tui${C_RESET}   or   ${C_BOLD}asahi-gtk${C_RESET}"
        log "  Open a new shell so PATH is picked up."
    fi
    return 0
}

# ── non-interactive path ───────────────────────────────────
noninteractive_flow() {
    [ -n "$WITH" ] || WITH="tui,gui,daemons,scripts,desktop,services"

    local -a comps=()
    IFS=',' read -ra parts <<< "$WITH"
    for p in "${parts[@]}"; do
        case "$p" in
            all)     comps+=(tui gui daemons scripts desktop services) ;;
            tui|gui|daemons|scripts|desktop|services|sudoers) comps+=("$p") ;;
            *) die "unknown component: $p" ;;
        esac
    done

    if [ "$FROM_SOURCE" = "0" ]; then FROM_SOURCE=1; fi  # only source works today

    execute_plan "${comps[@]}"
}

# ── main ───────────────────────────────────────────────────
main() {
    if [ "${UPGRADE:-0}" = "1" ]; then
        local verfile="$INSTALL_DIR/VERSION"
        if [ -f "$verfile" ]; then
            local cur
            cur=$(cat "$verfile" 2>/dev/null || echo "unknown")
            echo "Current version: $cur"
            echo "Target version:  0.3.0"
            if [ "$cur" = "0.3.0" ]; then
                echo "Already at 0.3.0 — reinstalling components"
            fi
        else
            echo "No existing install at $INSTALL_DIR; doing fresh install"
        fi
        echo
    fi
    if [ "$UNINSTALL" = "1" ]; then
        if [ "$ASSUME_YES" = "0" ] && [ -t 0 ] && [ "$DRY_RUN" = "0" ]; then
            prompt_yn "Remove all asahi-tools files, services, and sudoers rules?" n || exit 0
        fi
        tui_cleanup
        action_uninstall
        return
    fi

    if [ "$ASSUME_YES" = "1" ] || [ ! -t 0 ]; then
        noninteractive_flow
    else
        interactive_flow
    fi
}

main "$@"
