#!/bin/bash
# Read version from VERSION file
# Sets upstream version in GitHub output

set -e

UPSTREAM_VERSION=$(cat VERSION | tr -d '\n\r ')

echo "upstream=$UPSTREAM_VERSION" >> "$GITHUB_OUTPUT"
echo "Upstream version: $UPSTREAM_VERSION"
