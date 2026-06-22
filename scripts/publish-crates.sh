#!/usr/bin/env bash
# Publish intent-lang workspace crates to crates.io in dependency order.
# Requires: cargo login (or CARGO_REGISTRY_TOKEN)
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="0.1.3"

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

cargo publish -p intent-lang-syntax "$@"
wait_for_crate intent-lang-syntax "${VERSION}"

cargo publish -p intent-lang-core "$@"
wait_for_crate intent-lang-core "${VERSION}"

cargo publish -p intent-lang-visualizer "$@"
wait_for_crate intent-lang-visualizer "${VERSION}"

cargo publish -p intent-lang-cli "$@"

echo "Done. Install CLI: cargo install intent-lang-cli"
