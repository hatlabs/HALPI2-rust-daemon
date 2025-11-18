#!/bin/bash
# Read version from Cargo.toml workspace.package section
# Sets version and tag_version in GitHub output

set -e

VERSION=$(grep -m1 '^version =' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
# For daemon, version and tag_version are the same
TAG_VERSION="$VERSION"

echo "version=$VERSION" >> "$GITHUB_OUTPUT"
echo "tag_version=$TAG_VERSION" >> "$GITHUB_OUTPUT"
echo "Version from Cargo.toml: $VERSION (tag version: $TAG_VERSION)"
