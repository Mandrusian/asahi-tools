# asahi-tools

> **⚠️ ALPHA STATUS (v0.4.0-alpha.1)**
>
> This is an **unstable alpha release**. The widget system, GTK UI, and calibration tools are functional but may contain bugs. Tested on Fedora Asahi Remix (M1 Pro). Expect rough edges. Report issues on GitHub.


[![CI](https://github.com/Mandrusian/asahi-tools/actions/workflows/ci.yml/badge.svg)](https://github.com/Mandrusian/asahi-tools/actions/workflows/ci.yml)
[![Release](https://github.com/Mandrusian/asahi-tools/actions/workflows/release.yml/badge.svg)](https://github.com/Mandrusian/asahi-tools/actions/workflows/release.yml)
![platform](https://img.shields.io/badge/platform-Asahi%20Linux-blue)
![license](https://img.shields.io/badge/license-MIT-green)

Native settings, monitoring, and control tools for Apple Silicon Macs on Asahi Linux — the stuff GNOME/KDE Settings doesn't expose.

A GTK4 app for daily use, a terminal dashboard for power users, and a set of small focused CLIs and daemons underneath.

    asahi-gtk    ── the settings app (10 pages)
    asahi-tui    ── terminal dashboard + launcher
                  │
                  ├─ asahi-priv-daemon     privileged ops (sysfs writes)
                  ├─ asahi-charge-daemon   battery threshold enforcement
                  ├─ sysmon-daemon         fan control + thermal safety
                  ├─ asahi-force-click     trackpad pressure right-click
                  ├─ asahi-notify-hub      desktop notifications
                  ├─ asahi-lid             lid close handler
                  └─ scripts/              battery, display, wifi, kernel, ...

## What it actually does

Every Asahi Linux install has gaps that stock settings panels don't fill: no battery threshold control, no manual fan targets, no trackpad force-click, no safe kernel update helper, no WiFi drop logger, no sensor explorer. This project fills them, with a coherent GUI wrapping it all.

| Category | Features |
|---|---|
| Battery | Charge threshold (stop at N%, resume at M%), 2-hour full-charge override, health history sparkline, low/critical alerts |
| Thermal | Live sensor grid, per-fan manual RPM target, Auto/Max buttons, thermal safety net (fan ramps to max at 85 °C) |
| Power | Quiet / Balanced / Performance platform profile, live power rail readings |
| Display | Backlight brightness slider, internal / external / extend output modes |
| Input | Keyboard backlight slider, Fn-row mode cycling, trackpad pressure visualizer, calibrated pressure-based right click |
| Network | WiFi SSID/IP/signal, Bluetooth connected devices, one-tap module restart, WiFi drop logger |
| System | Kernel update helper (dnf + m1n1 + rollback), Btrfs snapshot management, diagnostic bundle generator, sensor explorer |
| Services | Start/stop/enable all daemons from the app |

## Install

### Requirements

- Apple Silicon Mac running Asahi Linux (Fedora Asahi Remix tested)
- Kernel 6.10+ (m1n1 firmware update needed for full trackpad support)
- For source builds:
    - rust toolchain (1.75+)
    - gtk4-devel, libadwaita-devel (for the GUI)
    - NetworkManager, bluez (for the network tools)

### One-liner (source build — recommended)

    curl -fsSL https://raw.githubusercontent.com/Mandrusian/asahi-tools/main/install.sh | sh -s -- --source

Runs an interactive TUI: pick components, confirm, install. Takes ~3 min for a full build.

### Non-interactive

    # Everything except sudoers rules
    bash install.sh --source --yes --with tui,gui,daemons,scripts,desktop,services

    # Just the TUI (no GTK)
    bash install.sh --source --yes --with tui,daemons,scripts

    # Dry run — shows every action, changes nothing
    bash install.sh --source --yes --dry-run

### After install

- Launch "Asahi Settings" from your app menu, or run `asahi-gtk` in a terminal
- `asahi-tui` for the terminal dashboard
- Open a fresh shell so PATH picks up `~/.local/bin`

### What gets installed where

| Location | Contents |
|---|---|
| ~/.local/bin/ | All CLI tools + symlinks to daemons |
| ~/.local/share/asahi-tools/bin/ | Actual daemon + tool binaries |
| ~/.local/share/asahi-tools/scripts/ | Bash helpers (lid, display, wifi-log, etc.) |
| ~/.local/share/applications/ | Desktop entries (asahi-gtk, asahi-tui, asahi-kernel-manager) |
| ~/.local/share/icons/hicolor/scalable/apps/ | App icon |
| /usr/local/bin/ | System daemons (SELinux-safe bin_t label) |
| /etc/systemd/system/ | asahi-priv-daemon, asahi-charge-daemon, asahi-sysmon-daemon, asahi-force-click |
| ~/.config/systemd/user/ | asahi-notify-hub, asahi-lid |
| ~/.config/asahi-tools/ | trackpad.json, notifications.json, lid.conf, charge-daemon.conf |
| ~/.local/share/asahi-tools/ | battery-health.sqlite, wifi-log.jsonl |

### Uninstall

    bash install.sh --uninstall

Removes all binaries, scripts, desktop entries, and systemd units. Preserves `~/.config/asahi-tools/` (your settings) and `~/.local/share/asahi-tools/*.sqlite` (your history). Add `--dry-run` to preview.

## What's new in v0.4.0-alpha.1

Builds on the v0.3.0 consolidation release with an Adwaita sidebar UI, customizable dashboard widgets, power profiles, and ambient light support:

- **GUI (`asahi-gtk`)**: 10 pages (Dashboard, Battery, Thermal, Profiles, Network, Display, Input, System, Services, About) with a collapsible sidebar and customizable dashboard widget grid
- **Power profiles**: `quiet`, `balanced`, and `performance` presets via `asahi-priv-daemon` + battery charge cap synchronization
- **Ambient light auto-brightness (ALS)**: integrated into `asahi-priv-daemon` with smooth curve interpolation and manual override window
- **Power & sleep utilities**: `asahi-power-draw` live SMC rail monitor and `asahi-sleep-log` sleep/wake telemetry logger
- **Unified CLI & completions**: `asahi` dispatcher and `asahi-completions bash|zsh|fish`
- **Portable smoke & unit tests**: `cargo test --workspace --exclude asahi-gtk` runs without GTK4/hardware

<details>
<summary>Previous release notes (v0.3.0)</summary>

- Removed redundant tools superseded by `asahi-gtk` and daemons (`asahi-gui`, `battery-limiter`, `kbd-backlight`, `asahi-charge-notify`)
- Added `asahi` meta-command, `asahi-completions`, `asahi-core` smoke tests, and `install.sh --upgrade`
- Excluded `asahi-trackpad` and `asahi-gtk` from cross-compiled release tarballs (build locally with `install.sh --source`)

</details>

## Usage

### Asahi Settings (GUI)

    asahi-gtk

Ten pages:

- Dashboard — customizable widget grid (battery, CPU temp, fans, memory, Wi-Fi, profile, uptime, brightness, quick actions)
- Battery — threshold editor, 2-hour override, live charge graph
- Thermal — sensor grid, per-fan manual RPM, Auto/Max
- Profiles — quiet/balanced/performance presets
- Network — WiFi status, clipboard copy, module restart
- Display — brightness slider, output switcher
- Input — keyboard backlight, layout, Fn mode, trackpad force-click settings
- System — kernel/uptime/bootloader info, diagnostic bundle, Btrfs snapshot
- Services — start/stop/enable every daemon
- About — version and license info

Sliders and toggles act immediately — no Save button on controls. Privileged operations go through `asahi-priv-daemon` over a Unix socket, so there are no auth prompts after the daemon starts.

### Asahi Tools (TUI)

    asahi-tui

Four tabs: Tools (13 categorized tools), Battery, Thermal, Network. Arrow keys navigate; Enter launches the selected tool; `1`-`4` or `Tab` jump between tabs.

Prompts for sudo once at startup, then caches for the session. All privileged actions use `sudo -n` after that.

## Architecture

    ┌────────────────────────────────────────────────────────────┐
    │  asahi-gtk  /  asahi-tui                                   │
    │  (unprivileged user)                                       │
    └─────────┬──────────────────────────────┬───────────────────┘
              │ Unix socket                  │ Unix socket
              │ /run/asahi-priv.sock         │ /run/asahi-charge.sock
              ▼                              ▼
    ┌────────────────────┐         ┌────────────────────┐
    │ asahi-priv-daemon  │         │ asahi-charge-...   │
    │ (root, systemd)    │         │ (root, systemd)    │
    ├────────────────────┤         ├────────────────────┤
    │ keyboard backlight │         │ sysfs threshold    │
    │ display brightness │         │ override timer     │
    │ platform profile   │         │ temp safety        │
    │ module restart     │         └────────────────────┘
    │ fnmode             │
    │ service control    │         ┌────────────────────┐
    └────────────────────┘         │ sysmon-daemon      │
                                   │ (root, systemd)    │
                                   ├────────────────────┤
                                   │ fan target         │
                                   │ thermal watchdog   │
                                   └────────────────────┘

Design principle: the GUI and TUI never write to sysfs. Every privileged operation flows through a daemon that owns the file descriptor. The daemon whitelists operations (e.g., `RestartModule` only accepts 7 specific modules), so a compromised client can't DoS the system. Socket mode is `0666` — the whitelist is the safety boundary, not the file permission.

## CLI reference

Every tool follows the same conventions: `--help` for usage, `--json` for machine-readable output where applicable.

### Battery

    asahi-charge-daemon status                 # JSON: current state
    asahi-charge-daemon override 100 2h        # charge to 100% for 2 hours
    asahi-charge-daemon cancel                 # cancel active override
    asahi-charge-daemon set-target 80          # persistent threshold
    asahi-battery-health                       # TUI: history sparkline + live stats

### Thermal + fans

    sys-monitor                                # TUI: temperatures + fan control
    asahi-priv-daemon                          # systemd service (run as root)

Fan control via the GUI or by talking to `/run/asahi-sysmon.sock` directly.

### Trackpad

    asahi-force-click --calibrate              # interactive: 3 taps + 3 force presses
    asahi-force-click                          # daemon (normally via systemd)
    asahi-trackpad-viz                         # TUI: pressure + touch visualization

Thresholds are stored in `~/.config/asahi-tools/trackpad.json` — edit or use the GUI.

### Scripts

    asahi-lid status                           # current lid state + config
    asahi-lid set-action suspend               # lock | suspend | hibernate | ignore | custom
    asahi-display internal                     # output mode (also: external, extend, mirror)
    asahi-wifi-log start                       # log WiFi drops to ~/.local/share/asahi-tools/
    asahi-kbd-fn-remap set fn                  # F1-F12 vs media keys
    asahi-snapshot before-update               # Btrfs snapshot helper
    asahi-diag                                 # bundle everything for a bug report
    asahi-sensor-explorer                      # list every hwmon input

### System

    asahi-kernel-manager                       # TUI: kernel updates + m1n1 + journal

## Building from source

    git clone https://github.com/Mandrusian/asahi-tools
    cd asahi-tools
    cargo build --release --workspace

Binaries land in `target/release/`. To run from the source tree:

    ./target/release/asahi-tui
    ./target/release/asahi-gtk

Both tools auto-detect the source tree and find sibling binaries, so you can develop without installing.

### Optional: passwordless sudo

For a smoother experience, drop this into `/etc/sudoers.d/asahi-tools` (replace `youruser` with your username):

    youruser ALL=(ALL) NOPASSWD: /usr/local/bin/asahi-priv-daemon
    youruser ALL=(ALL) NOPASSWD: /usr/local/bin/asahi-charge-daemon
    youruser ALL=(ALL) NOPASSWD: /usr/local/bin/sysmon-daemon
    youruser ALL=(ALL) NOPASSWD: /usr/local/bin/asahi-force-click
    youruser ALL=(ALL) NOPASSWD: /usr/sbin/modprobe

`bash install.sh --sudoers` (or `--with sudoers`) writes these automatically.

## Troubleshooting

### "GTK4 headers not found" during build

    sudo dnf install gtk4-devel libadwaita-devel

### Daemon fails with status 203/EXEC

SELinux is blocking execution from `/home/`. Daemons must be installed under `/usr/local/bin/`. `install.sh` does this; if you built manually:

    sudo install -m 755 target/release/asahi-priv-daemon /usr/local/bin/

### Trackpad right-click doesn't fire

1. Run calibration: Input → Force click → Run calibration
2. Enable the daemon: Services → Trackpad right click → toggle
3. Check `systemctl status asahi-force-click` — if it says `no Apple trackpad`, verify `ls /dev/input/event*` shows your trackpad
4. If it fires too easily, increase `press limit` in the Input tab (calibration sets this automatically)

### Keyboard backlight / display slider prompts for password

`asahi-priv-daemon` isn't running:

    sudo systemctl enable --now asahi-priv-daemon
    sudo systemctl status asahi-priv-daemon

### Charge threshold resets after reboot

`asahi-charge-daemon` isn't enabled:

    sudo systemctl enable --now asahi-charge-daemon

### Notification hub doesn't fire

Check user unit:

    systemctl --user status asahi-notify-hub
    systemctl --user enable --now asahi-notify-hub

Verify `notify-send` works: `notify-send "Test" "Hello"` — if nothing appears, install `libnotify` (`sudo dnf install libnotify`).

### "No such device" when opening /run/asahi-priv.sock

The daemon isn't running, or the socket was left stale:

    sudo systemctl restart asahi-priv-daemon
    ls -la /run/asahi-priv.sock

## Contributing

Pull requests welcome. The build is:

    cargo fmt --all -- --check    # CI enforces
    cargo clippy --workspace
    cargo test --workspace        # or `--exclude asahi-gtk` without GTK4 headers
    cargo build --release --workspace

CI runs on `ubuntu-24.04` (with GTK4/Adwaita/evdev headers) across the full workspace, while tag releases cross-compile CLI/daemon crates to `aarch64-unknown-linux-gnu` (excluding `asahi-gtk` and `asahi-trackpad`).

## Acknowledgments

Asahi Linux community — especially the kernel devs working on `spi-hid-apple`, `macsmc-hwmon`, and the DCP display stack. The GUI follows Adwaita conventions; the TUI follows ratatui idioms; nothing here would exist without those upstream projects.

The haptic feedback driver (Taptic Engine force click) is not implemented — that's kernel-side work in progress upstream. This project does pressure-based right-click without haptics.

## License

MIT — see LICENSE.
