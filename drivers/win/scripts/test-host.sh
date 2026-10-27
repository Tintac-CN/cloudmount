#!/usr/bin/env bash
# Host-only tests and driver type-check. No WDK, signing, or admin required.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo test --workspace
cargo check -p cmflt-driver
echo "OK: host tests + driver type-check"
