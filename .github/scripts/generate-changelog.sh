#!/bin/bash
set -euo pipefail

# No-op for cargo-deb projects
# cargo-deb gets all package metadata from Cargo.toml, not debian/changelog
# This script exists to satisfy the shared-workflow's check for a local override

echo "Skipping debian/changelog generation (cargo-deb project)"
echo "Package metadata is sourced from Cargo.toml"
