# HALPI2 Rust Daemon

Rust reimplementation of the HALPI2 power monitor and watchdog daemon (`halpid`) with improved performance, reliability, and resource utilization.

## Features

- **Power Management**: Monitors input voltage and supercapacitor charge, managing graceful shutdowns during blackout events
- **Watchdog**: Provides watchdog functionality to detect system hangs
- **HTTP API**: REST API over Unix socket for status queries and control
- **CLI**: Command-line interface for interacting with the daemon
- **Firmware Updates**: Supports over-the-air firmware updates via I2C (DFU protocol)
- **USB Port Control**: Power cycling capabilities for USB ports
- **100% API Compatible**: Drop-in replacement for Python `halpid` version 4.x

## Performance

- **Memory**: <10MB footprint (vs ~50MB for Python version)
- **Startup**: <100ms (vs ~2s for Python version)
- **Binary**: Single static binary with no external dependencies

## Quick Start

### Building

```bash
# Build all workspace members
./run build

# Build release version
./run build --release

# Cross-compile for ARM64 (Raspberry Pi CM5)
./run build:cross --release
```

### Running

```bash
# Run daemon (requires root for I2C access)
sudo ./target/release/halpid

# Use CLI to check status
./target/release/halpi status
```

### Development

```bash
# Run all checks (format, lint, test)
./run ci:check

# Format code
./run fmt

# Run tests
./run test

# Build Debian package
./run package:deb:cross
```

## Components

- **halpid**: Power monitor and watchdog daemon
- **halpi**: Command-line interface
- **halpi-common**: Shared types and utilities

## Requirements

- Rust 1.91+ (edition 2024)
- Target: `aarch64-unknown-linux-musl` for Raspberry Pi CM5
- I2C hardware: HALPI2 board with RP2040 firmware

## Documentation

- [Technical Specification](docs/SPEC.md)
- [Architecture](docs/ARCHITECTURE.md)
- [HALPI2 Documentation](https://docs.hatlabs.fi/halpi2)

## License

BSD-3-Clause - see [LICENSE](LICENSE) for details

## Development

See [CLAUDE.md](CLAUDE.md) for repository-specific development instructions.
