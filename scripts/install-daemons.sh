#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release --workspace
for b in asahi-charge-daemon sysmon-daemon; do
    sudo install -m 755 "target/release/$b" "/usr/local/bin/$b"
done
sudo systemctl daemon-reload
sudo systemctl try-restart asahi-charge-daemon.service 2>/dev/null || true
echo "daemons synced to /usr/local/bin"
