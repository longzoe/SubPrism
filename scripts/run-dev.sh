#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "Building SubPrism Liquid Glass UI..."
(cd ui && (command -v bun >/dev/null 2>&1 && bun run build || npm run build))

echo "Launching SubPrism (Release)..."
cargo run --release -p subprism
