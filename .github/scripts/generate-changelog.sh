#!/bin/bash
set -euo pipefail

# Generate version file for cargo-deb
# cargo-deb gets package metadata from Cargo.toml, but we need to override
# the version to include the calculated revision number

UPSTREAM=""
REVISION=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --upstream)
            UPSTREAM="$2"
            shift 2
            ;;
        --revision)
            REVISION="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
    esac
done

if [ -z "$UPSTREAM" ] || [ -z "$REVISION" ]; then
    echo "Error: --upstream and --revision are required" >&2
    exit 1
fi

DEBIAN_VERSION="${UPSTREAM}-${REVISION}"

echo "Skipping debian/changelog generation (cargo-deb project)"
echo "Writing Debian version to .debian-version: $DEBIAN_VERSION"

# Write version file for build-deb action to read
echo "$DEBIAN_VERSION" > .debian-version
