# HALPI2 Rust Daemon

High-performance Rust reimplementation of the HALPI2 power monitor and watchdog daemon (`halpid`) with improved reliability, performance, and resource utilization.

[![License](https://img.shields.io/badge/License-BSD%203--Clause-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.91%2B-orange.svg)](https://www.rust-lang.org/)

## Overview

The HALPI2 Rust Daemon provides power management, watchdog, and system control capabilities for HALPI2-equipped Raspberry Pi systems. It's a **100% API-compatible** drop-in replacement for the Python `halpid` version 4.x, offering significant performance improvements.

### Key Features

- **Power Management**: Monitors input voltage and supercapacitor charge, coordinating graceful shutdowns during blackout events
- **Watchdog**: Hardware watchdog integration to detect and recover from system hangs
- **HTTP API**: RESTful API over Unix socket for status queries and control
- **CLI**: Comprehensive command-line interface (`halpi`) for system interaction
- **Firmware Updates**: Over-the-air firmware updates via I2C DFU protocol
- **USB Port Control**: Power cycling capabilities for individual USB ports
- **State Machine**: 0.1-second polling interval for responsive power management
- **Static Binary**: Single self-contained executable with no runtime dependencies

### Performance Benefits

Compared to Python `halpid` 4.x:

| Metric | Python 4.x | Rust 5.x | Improvement |
|--------|-----------|----------|-------------|
| Memory footprint | ~50 MB | <10 MB | **5x reduction** |
| Startup time | ~2 seconds | <100 ms | **20x faster** |
| Binary size | N/A | ~3 MB | Single static binary |
| Dependencies | Python + libs | None | Zero runtime deps |

## Installation

### From Debian Package (Recommended)

```bash
# Add HAT Labs APT repository
echo "deb [trusted=yes] https://apt.hatlabs.fi stable main" | sudo tee /etc/apt/sources.list.d/hatlabs.list

# Update and install
sudo apt update
sudo apt install halpid

# Service starts automatically
sudo systemctl status halpid
```

### From Source

```bash
# Clone repository
git clone https://github.com/hatlabs/HALPI2-rust-daemon.git
cd HALPI2-rust-daemon

# Build for current architecture
./run build --release

# Or cross-compile for ARM64 (Raspberry Pi)
./run build:cross --release

# Install binaries
# For native build:
sudo cp target/release/halpid /usr/bin/
sudo cp target/release/halpi /usr/bin/

# For cross-compiled binaries:
# sudo cp target/aarch64-unknown-linux-musl/release/halpid /usr/bin/
# sudo cp target/aarch64-unknown-linux-musl/release/halpi /usr/bin/

# Install systemd service
sudo cp systemd/halpid.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable halpid
sudo systemctl start halpid
```

## Quick Start

### Daemon Usage

The `halpid` daemon runs as a system service and requires root privileges for I2C access:

```bash
# Start daemon (managed by systemd)
sudo systemctl start halpid

# Check status
sudo systemctl status halpid

# View logs
sudo journalctl -u halpid -f

# Stop daemon
sudo systemctl stop halpid
```

### CLI Usage

The `halpi` CLI communicates with the daemon via Unix socket (no root required for most operations):

```bash
# Check system status
halpi status

# Get daemon version
halpi version

# Get all configuration values
halpi config

# Get specific configuration
halpi config blackout-time-limit

# Control USB ports
halpi usb              # Show all port states
halpi usb enable 0     # Enable port 0
halpi usb disable all  # Disable all ports

# System shutdown
halpi shutdown

# Enter standby mode
halpi shutdown --standby --time 300  # Wake after 300 seconds
halpi shutdown --standby --time "2025-12-31T23:59:59"  # Wake at datetime

# Upload firmware
halpi flash firmware.bin
```

## Configuration

Configuration file: `/etc/halpid/halpid.conf` (YAML format)

```yaml
# I2C bus configuration
i2c-bus: 1
i2c-addr: 0x6D

# Blackout detection thresholds
blackout-time-limit: 10.0      # seconds
blackout-voltage-limit: 9.0    # volts

# Unix socket for HTTP API
socket: /run/halpid/halpid.sock
socket-group: adm

# Shutdown command (empty string for dry-run testing)
poweroff: /sbin/poweroff
```

### Configuration via CLI Arguments

Override configuration file settings:

```bash
halpid --conf /etc/halpid/halpid.conf \
       --i2c-bus 1 \
       --i2c-addr 109 \
       --socket /run/halpid/halpid.sock \
       --blackout-time-limit 10.0 \
       --blackout-voltage-limit 9.0 \
       --poweroff /sbin/poweroff
```

## HTTP API

The daemon exposes a RESTful API on a Unix socket (default: `/run/halpid/halpid.sock`).

### Endpoints

#### Health and Version

```bash
# Health check
curl --unix-socket /run/halpid/halpid.sock http://localhost/

# Get version
curl --unix-socket /run/halpid/halpid.sock http://localhost/version
```

#### System Values

```bash
# Get all measurements
curl --unix-socket /run/halpid/halpid.sock http://localhost/values

# Response example:
{
  "dcin_voltage": 12.5,
  "supercap_voltage": 11.2,
  "input_current": 0.8,
  "mcu_temperature": 35.2,
  "pcb_temperature": 33.8,
  "power_state": "OperationalCoOp",
  "firmware_version": "2.1.0",
  "hardware_version": "2.0.0",
  "device_id": "e66164840bce7521"
}
```

#### Configuration

```bash
# Get all config
curl --unix-socket /run/halpid/halpid.sock http://localhost/config

# Get specific config value
curl --unix-socket /run/halpid/halpid.sock http://localhost/config/blackout_time_limit
```

#### USB Port Control

```bash
# Get USB port states
curl --unix-socket /run/halpid/halpid.sock http://localhost/usb

# Enable USB port 0
curl --unix-socket /run/halpid/halpid.sock \
     -X PUT -H "Content-Type: application/json" \
     http://localhost/usb/0 -d 'true'

# Disable USB port 1
curl --unix-socket /run/halpid/halpid.sock \
     -X PUT -H "Content-Type: application/json" \
     http://localhost/usb/1 -d 'false'
```

#### Shutdown and Standby

```bash
# Request shutdown
curl --unix-socket /run/halpid/halpid.sock \
     -X POST http://localhost/shutdown

# Request standby with delay (seconds)
curl --unix-socket /run/halpid/halpid.sock \
     -X POST -H "Content-Type: application/json" \
     http://localhost/standby -d '{"delay": 300}'

# Request standby at specific datetime
curl --unix-socket /run/halpid/halpid.sock \
     -X POST -H "Content-Type: application/json" \
     http://localhost/standby -d '{"datetime": "2025-12-31T23:59:59"}'
```

## Architecture

### Components

- **halpid**: Power monitor and watchdog daemon
  - I2C communication with RP2040 firmware
  - HTTP server on Unix socket
  - State machine for power management
  - Signal handling for graceful shutdown

- **halpi**: Command-line interface
  - Communicates with daemon via HTTP/Unix socket
  - User-friendly status display
  - System control operations

- **halpi-common**: Shared library
  - Data types and protocol definitions
  - Configuration management
  - Error handling

### System Integration

```
┌─────────────────────────────────────────────────────┐
│  Raspberry Pi CM5 (Linux)                           │
│  ┌──────────────────────────────────────────┐      │
│  │  halpid (Rust daemon)                    │      │
│  │  - Monitors power state                  │      │
│  │  - Feeds watchdog                        │      │
│  │  - Orchestrates shutdown                 │      │
│  │  - HTTP API server                       │      │
│  └────────────────┬─────────────────────────┘      │
└───────────────────┼──────────────────────────────────┘
                    │ I2C (bus 1, addr 0x6d)
┌───────────────────┼──────────────────────────────────┐
│  RP2040 MCU       │                                  │
│  ┌────────────────┴─────────────────────────┐       │
│  │  HALPI2-firmware (Rust/Embassy)          │       │
│  │  - State machine (power management)      │       │
│  │  - GPIO control (power rails, USB, LEDs) │       │
│  │  - Analog monitoring (VIN, VSCAP, IIN)   │       │
│  │  - I2C secondary device                  │       │
│  └──────────────────────────────────────────┘       │
└──────────────────────────────────────────────────────┘
```

For detailed architecture information, see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Development

### Prerequisites

- Rust 1.91+ (edition 2024)
- Cross-compilation tools (for ARM64 target)
- HALPI2 hardware (for integration testing)

### Building

```bash
# Build all workspace members
./run build

# Build release version
./run build --release

# Cross-compile for ARM64 (Raspberry Pi CM5)
./run build:cross --release

# Build Debian package
./run package:deb:cross
```

### Testing

```bash
# Run all tests
./run test

# Run unit tests only
cargo test --lib

# Run tests with coverage (requires cargo-llvm-cov)
./run test:coverage

# Run pre-commit checks (format, lint, test)
./run ci:check
```

### Code Quality

```bash
# Format code
./run fmt

# Check formatting
./run fmt:check

# Run clippy linter
cargo clippy --all-targets -- -D warnings

# Generate documentation
cargo doc --no-deps --open
```

### Pre-commit Hooks

This project uses [lefthook](https://github.com/evilmartians/lefthook) for pre-commit hooks to run format and lint checks locally before commits.

```bash
# Install lefthook (one-time)
brew install lefthook

# Enable hooks in this repo
./run hooks-install
```

**What it checks:**

- `cargo fmt --all -- --check` - Code formatting
- `cargo clippy --all-targets -- -D warnings` - Linting

**Skip hooks when needed:**

```bash
git commit --no-verify -m "WIP: message"
```

## Migrating from Python halpid 4.x

The Rust daemon is 100% API compatible with Python `halpid` 4.x. Migration is straightforward:

1. **Stop Python daemon**:
   ```bash
   sudo systemctl stop halpid
   sudo systemctl disable halpid
   ```

2. **Install Rust version**:
   ```bash
   sudo apt install halpid  # From APT repository
   # Or build from source
   ```

3. **Verify configuration**:
   - Configuration file format is identical
   - Unix socket path remains the same
   - All API endpoints unchanged

4. **Start Rust daemon**:
   ```bash
   sudo systemctl start halpid
   sudo systemctl status halpid
   ```

5. **Test compatibility**:
   ```bash
   halpi status  # Should work identically
   ```

For detailed migration information, see [docs/MIGRATION.md](docs/MIGRATION.md).

## Troubleshooting

### Daemon won't start

```bash
# Check I2C permissions
ls -l /dev/i2c-1

# Check socket directory exists
sudo mkdir -p /run/halpid
sudo chmod 755 /run/halpid

# Check daemon logs
sudo journalctl -u halpid -n 50
```

### CLI can't connect to daemon

```bash
# Verify daemon is running
sudo systemctl status halpid

# Check socket exists and has correct permissions
ls -l /run/halpid/halpid.sock

# Check socket group membership
groups  # Should include 'adm' group
```

### I2C communication errors

```bash
# Verify I2C device exists
ls -l /dev/i2c-1

# Check I2C address is correct
sudo i2cdetect -y 1  # Should show device at 0x6D

# Verify firmware version
halpi version
```

## Documentation

- [Technical Specification](docs/SPEC.md) - Detailed API and protocol specs
- [Architecture](docs/ARCHITECTURE.md) - System design and implementation
- [Testing Strategy](docs/TESTING.md) - Unit and integration testing approach
- [Migration Guide](docs/MIGRATION.md) - Upgrading from Python version
- [HALPI2 Hardware Docs](https://docs.hatlabs.fi/halpi2) - Product documentation

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests: `./run ci:check`
5. Submit a pull request

See [CLAUDE.md](CLAUDE.md) for development guidelines.

## Related Projects

- [HALPI2-firmware](https://github.com/hatlabs/HALPI2-firmware) - RP2040 embedded firmware (Rust/Embassy)
- [HALPI2-hardware](https://github.com/hatlabs/HALPI2-hardware) - Carrier board PCB design (KiCad)
- [HALPI2-tests](https://github.com/hatlabs/HALPI2-tests) - Hardware production test suite
- [halpi2](https://github.com/hatlabs/halpi2) - User documentation

## License

BSD-3-Clause - see [LICENSE](LICENSE) for details.

## Support

- **Product Page**: https://shop.hatlabs.fi/products/halpi2
- **Documentation**: https://docs.hatlabs.fi/halpi2
- **Issues**: https://github.com/hatlabs/HALPI2-rust-daemon/issues
- **APT Repository**: https://apt.hatlabs.fi
