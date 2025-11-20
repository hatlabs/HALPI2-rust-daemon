#!/bin/bash
# Generate release notes for a release
# Usage: generate-release-notes.sh <VERSION> <TAG_VERSION> <RELEASE_TYPE>
# RELEASE_TYPE: "prerelease", "draft", or "stable"

set -e

VERSION="${1:?Version required}"
TAG_VERSION="${2:?Tag version required}"
RELEASE_TYPE="${3:?Release type required}"

if [ -z "$VERSION" ] || [ -z "$TAG_VERSION" ] || [ -z "$RELEASE_TYPE" ]; then
  echo "Usage: generate-release-notes.sh <VERSION> <TAG_VERSION> <RELEASE_TYPE>"
  exit 1
fi

SHORT_SHA="${GITHUB_SHA:0:7}"

# Get the latest published (non-prerelease) release
LAST_TAG=$(gh release list --limit 100 --json tagName,isPrerelease,isDraft \
  --jq '.[] | select(.isDraft == false and .isPrerelease == false) | .tagName' | head -n1)

if [ -n "$LAST_TAG" ]; then
  echo "Generating changelog since $LAST_TAG"
  CHANGELOG=$(git log "${LAST_TAG}"..HEAD --pretty=format:"- %s (%h)" --no-merges --)
else
  echo "No previous published releases found, using recent commits"
  CHANGELOG=$(git log -10 --pretty=format:"- %s (%h)" --no-merges)
fi

case "$RELEASE_TYPE" in
  prerelease)
    cat > release_notes.md <<EOF
## HALPI2 Rust Daemon v${TAG_VERSION} (Pre-release)

⚠️ **This is a pre-release build from the main branch. Use for testing only.**

**Build Information:**
- Commit: ${SHORT_SHA} (\`${GITHUB_SHA}\`)
- Built: $(date -u '+%Y-%m-%d %H:%M:%S UTC')

### Recent Changes

${CHANGELOG}

### Installation

To install this pre-release on your HALPI2:

\`\`\`bash
# Add Hat Labs repository (if not already added)
curl -fsSL https://apt.hatlabs.fi/hat-labs-apt-key.asc | sudo gpg --dearmor -o /usr/share/keyrings/hatlabs-apt-key.gpg

# Add unstable channel
echo "deb [signed-by=/usr/share/keyrings/hatlabs-apt-key.gpg] https://apt.hatlabs.fi unstable main" | sudo tee /etc/apt/sources.list.d/hatlabs-unstable.list

# Update and install
sudo apt update
sudo apt install halpi2-daemon
\`\`\`

EOF
    ;;

  draft)
    cat > release_notes.md <<EOF
## HALPI2 Rust Daemon v${VERSION}

High-performance Rust reimplementation of the HALPI2 power monitor and watchdog daemon.

### Changes

${CHANGELOG}

### Features

- **Power Management**: Monitor input voltage and supercapacitor charge, coordinate graceful shutdowns
- **Watchdog**: Hardware watchdog integration for system hang detection and recovery
- **HTTP API**: RESTful API over Unix socket for status and control
- **CLI**: Comprehensive command-line interface (\`halpi\`)
- **Firmware Updates**: Over-the-air firmware updates via I2C DFU protocol
- **USB Port Control**: Power cycling capabilities for individual USB ports
- **High Performance**: 5x memory reduction, 20x faster startup vs Python version

### Installation

This is the source code release. For Debian packages:

\`\`\`bash
sudo apt install halpi2-daemon
\`\`\`

See [apt.hatlabs.fi](https://github.com/hatlabs/apt.hatlabs.fi) for repository setup.

### Development

For development setup and build commands, see:
- [README.md](https://github.com/hatlabs/HALPI2-rust-daemon/blob/main/README.md) - Installation and usage
- \`./run help\` - Available build and development commands
EOF
    ;;

  stable)
    cat > release_notes.md <<EOF
## HALPI2 Rust Daemon v${VERSION}

High-performance Rust reimplementation of the HALPI2 power monitor and watchdog daemon.

### Changes

${CHANGELOG}

### Features

- **Power Management**: Monitor input voltage and supercapacitor charge, coordinate graceful shutdowns
- **Watchdog**: Hardware watchdog integration for system hang detection and recovery
- **HTTP API**: RESTful API over Unix socket for status and control
- **CLI**: Comprehensive command-line interface (\`halpi\`)
- **Firmware Updates**: Over-the-air firmware updates via I2C DFU protocol
- **USB Port Control**: Power cycling capabilities for individual USB ports
- **High Performance**: 5x memory reduction, 20x faster startup vs Python version

### Installation

This is the source code release. For Debian packages:

\`\`\`bash
sudo apt install halpi2-daemon
\`\`\`

See [apt.hatlabs.fi](https://github.com/hatlabs/apt.hatlabs.fi) for repository setup.

### Development

For development setup and build commands, see:
- [README.md](https://github.com/hatlabs/HALPI2-rust-daemon/blob/main/README.md) - Installation and usage
- \`./run help\` - Available build and development commands
EOF
    ;;

  *)
    echo "Error: Unknown RELEASE_TYPE '$RELEASE_TYPE'. Use 'prerelease', 'draft', or 'stable'"
    exit 1
    ;;
esac

echo "Release notes created"
