#!/usr/bin/env bash
# performance:work collector: prints coding-tooling/work-evidence/v1 for the
# operations declared in .performance/contract.json (schemaVersion 2).
# Only the evidence JSON goes to stdout; build output goes to stderr.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

contract=".performance/contract.json"
if command -v sha256sum >/dev/null 2>&1; then
  sha="$(sha256sum "$contract" | cut -d' ' -f1)"
else
  sha="$(shasum -a 256 "$contract" | cut -d' ' -f1)"
fi

cargo build --release -p raid-defense-sim --bin raid-defense-sim >&2
target_dir="$(cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
binary="${target_dir:-$root/target}/release/raid-defense-sim"

exec "$binary" --work-evidence --contract-sha256 "$sha"
