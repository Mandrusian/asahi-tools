# asahi-tools

Terminal utilities for Apple Silicon Macs on Asahi Linux, plus `asahi-tui`, a launcher and dashboard.

## What's inside

| Binary | Purpose | Needs root |
|---|---|---|
| `asahi-tui` | Launcher + at-a-glance dashboard | no (escalates on launch) |
| `kbd-backlight` | Adjust keyboard backlight | yes |
| `battery-limiter` | Set charge start/end thresholds | yes |
| `net-restarter` | Wi-Fi / Bluetooth info, scans, module restart | restart only |
| `sys-monitor` | Temperatures + manual fan control (TUI) | no (talks to daemon) |
| `sysmon-daemon` | Privileged fan daemon for `sys-monitor` | yes |

## Requirements

- Asahi Linux (tested on Fedora Asahi Remix, kernel 7.1.x-asahi)
- Apple Silicon Mac exposing `macsmc-hwmon`, `macsmc-battery`, `kbd_backlight` in sysfs
- `nmcli` (NetworkManager) and `bluetoothctl` (BlueZ) for the network tool
- Rust 1.75+ stable

## Build

    cargo build --release --workspace

Binaries land in `target/release/`.

## Run

    ./target/release/asahi-tui

Individual tools:

    sudo ./target/release/kbd-backlight
    sudo ./target/release/battery-limiter
    ./target/release/net-restarter
    sudo ./target/release/sysmon-daemon &
    ./target/release/sys-monitor

## Optional: passwordless sudo

Drop into `/etc/sudoers.d/asahi-tools` and `chmod 440` it (replace user and path):

    xavi ALL=(ALL) NOPASSWD: /usr/sbin/modprobe
    xavi ALL=(ALL) NOPASSWD: /home/xavi/asahi-tools/target/release/kbd-backlight
    xavi ALL=(ALL) NOPASSWD: /home/xavi/asahi-tools/target/release/battery-limiter
    xavi ALL=(ALL) NOPASSWD: /home/xavi/asahi-tools/target/release/net-restarter
    xavi ALL=(ALL) NOPASSWD: /home/xavi/asahi-tools/target/release/sys-monitor
    xavi ALL=(ALL) NOPASSWD: /home/xavi/asahi-tools/target/release/sysmon-daemon

## Fan control

`macsmc_hwmon` exposes fan targets but blocks writes unless loaded with `fan_control=1`:

    sudo modprobe -r macsmc_hwmon && sudo modprobe macsmc_hwmon fan_control=1

`sysmon-daemon` reports read-only mode if this hasn't been done.

## Architecture

    asahi-tools/
    ├── asahi-core/       library: sysfs access + error types
    ├── asahi-tui/        launcher + dashboard (ratatui)
    ├── kbd-backlight/    keyboard backlight TUI
    ├── battery-limiter/  charge threshold TUI
    ├── net-restarter/    Wi-Fi + Bluetooth dashboard
    └── sys-monitor/
        ├── src/main.rs           TUI frontend
        └── src/bin/daemon.rs     privileged fan daemon

`asahi-tui` reads only — all sysfs writes happen in the child tools. `sysmon-daemon` pins fans to max if any sensor crosses 85 °C.

## Notes

- `net-restarter` uses `nmcli -m multiline` so SSIDs containing `:` work correctly.
- `sysmon-daemon` auto-starts on first `sys-monitor` launch from `asahi-tui`.
- Bluetooth scanning in `net-restarter` is opt-in (`s`) and runs in a background thread.

## License

MIT — see `LICENSE`.
