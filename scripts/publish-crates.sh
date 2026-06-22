#!/usr/bin/env bash
# Publish intent-lang workspace crates to crates.io in dependency order.
# Requires: cargo login (or CARGO_REGISTRY_TOKEN)
set -euo pipefail
cd "$(dirname "$0")/.."

wait_for_crate() {
  local name="$1"
  local version="$2"
  echo "Waiting for ${name} ${version} on crates.io index..."
  for _ in $(seq 1 30); do
    if cargo search "${name}" --limit 1 2>/dev/null | grep -q "${name} = \"${version}\""; then
      echo "  ✓ ${name} ${version} is indexed"
      return 0
    fi
    sleep 10
  done
  echo "  ⚠ ${name} not visible yet; continuing anyway"
}

cargo publish -p intent-syntax "$@"
wait_for_crate intent-syntax 0.1.2

cargo publish -p intent-core "$@"
wait_for_crate intent-core 0.1.2

cargo publish -p intent-visualizer "$@"
wait_for_crate intent-visualizer 0.1.2

cargo publish -p intent-cli "$@"

echo "Done. Install CLI: cargo install intent-cli"
