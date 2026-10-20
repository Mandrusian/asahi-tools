# asahi-tools — common tasks
# usage: just <recipe>

default:
    @just --list

build:
    cargo build --release --workspace

check:
    cargo check --workspace --all-targets

fmt:
    cargo fmt --all

clippy:
    cargo clippy --workspace --all-targets

test:
    cargo test --release -p asahi-core

install:
    bash install.sh --source --yes --with all

install-tui:
    bash install.sh --source --yes --with tui,daemons,scripts

dry-run:
    bash install.sh --source --yes --dry-run

soak duration="30m":
    ./scripts/asahi-soak {{duration}}

release version:
    #!/bin/sh
    set -e
    sed -i "s/^version = \".*\"$/version = \"{{version}}\"/" Cargo.toml
    cargo build --release --workspace
    git add -A
    git commit -m "release: v{{version}}"
    git tag -a "v{{version}}" -m "Release v{{version}}"
    echo "Tagged v{{version}} — push with: git push && git push origin v{{version}}"

clean:
    cargo clean

daemons:
    sudo systemctl status asahi-priv-daemon asahi-charge-daemon asahi-sysmon-daemon asahi-force-click --no-pager | head -30

logs daemon:
    sudo journalctl -u {{daemon}} -n 50 --no-pager
