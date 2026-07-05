#!/bin/sh
# Install the DPAA2 control plane onto a Debian target. Run as root.
#
# Layout (see packaging/README.md):
#   /usr/bin/dpaa2ctl
#   /etc/systemd/system/dpaa2-provision.service
#   /etc/udev/rules.d/99-dpaa2-provision.rules
#   /etc/dpaa2/topology.toml   (only if absent — never clobber operator intent)
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$here/.." && pwd)

BIN=${BIN:-"$repo/target/release/dpaa2ctl"}

if [ ! -x "$BIN" ]; then
    echo "building release binary..."
    ( cd "$repo" && cargo build --release --bin dpaa2ctl )
fi

echo "installing binary -> /usr/bin/dpaa2ctl"
install -Dm0755 "$BIN" /usr/bin/dpaa2ctl

echo "installing systemd unit"
install -Dm0644 "$here/systemd/dpaa2-provision.service" \
    /etc/systemd/system/dpaa2-provision.service

echo "installing udev rule"
install -Dm0644 "$here/udev/99-dpaa2-provision.rules" \
    /etc/udev/rules.d/99-dpaa2-provision.rules

if [ ! -e /etc/dpaa2/topology.toml ]; then
    echo "installing example topology -> /etc/dpaa2/topology.toml"
    install -Dm0644 "$here/dpaa2/topology.toml" /etc/dpaa2/topology.toml
else
    echo "keeping existing /etc/dpaa2/topology.toml"
fi

echo "reloading systemd and udev"
systemctl daemon-reload
udevadm control --reload

echo "done. Enable with: systemctl enable dpaa2-provision.service"
