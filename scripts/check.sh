#!/usr/bin/env bash
# The gate that must pass before every commit, matching `nix flake check`.
set -euo pipefail

cd "$(dirname "$0")/.."

cargo check --workspace --all-targets
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
