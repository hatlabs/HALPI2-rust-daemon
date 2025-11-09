# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

HALPI2 Rust Daemon is a reimplementation of the HALPI2 power monitor and watchdog daemon in Rust for improved performance, reliability, and resource utilization. It maintains 100% API compatibility with the Python `halpid` version 4.x.

## Development Environment

**IMPORTANT**: This is a Linux-only daemon. Use one of these approaches for development:

### Option 1: Dev Container (Recommended for macOS)
Open the project in VSCode and select "Reopen in Container". This provides:
- Full Linux environment for native builds and testing
- Pre-configured Rust toolchain with ARM64 cross-compilation support
- All tools pre-installed (`cross`, `cargo-deb`, `gh`)

### Option 2: Cross-Compilation (Limited)
Cross-compilation from macOS works for building but NOT for testing:
```bash
./run build:cross --release  # Compiles for ARM64 Linux
```
Note: Tests cannot run via cross-compilation - use dev container instead.

### Option 3: Native Linux
If running on Linux, all commands work natively without containers.

## Development Commands

Use the `./run` script for all development tasks:

### Core Development
- `./run build [--release]` - Build all workspace members
- `./run build:daemon [--release]` - Build halpid daemon only
- `./run build:cli [--release]` - Build halpi CLI only
- `./run clean` - Clean all build artifacts
- `./run check` - Run cargo check and clippy
- `./run fmt` - Format code with rustfmt
- `./run fmt:check` - Check code formatting

### Cross-Compilation
- `./run build:cross [--release]` - Build for ARM64 Linux (aarch64-unknown-linux-musl)
- `./run cross:setup` - Install cross-compilation tools

### Testing
- `./run test` - Run all tests
- `./run test:unit` - Run unit tests only
- `./run test:integration` - Run integration tests only
- `./run test:coverage` - Run tests with coverage report

### Package Management
- `./run package:deb` - Build Debian package (native)
- `./run package:deb:cross` - Build Debian package for ARM64

### Development Utilities
- `./run dev:daemon` - Run daemon in development mode
- `./run dev:clean:all` - Deep clean (cargo + artifacts + packages)
- `./run dev:version:bump <version>` - Bump version to specified version
- `./run dev:version:show` - Show current version

### CI/CD
- `./run ci:check` - Run CI verification checks
- `./run ci:build` - Full CI build pipeline

### Common Workflows
```bash
# Development cycle
./run build && ./run dev:daemon

# Full check before commit
./run ci:check

# Build release for Raspberry Pi
./run build:cross --release
./run package:deb:cross

# Version management
./run dev:version:show
./run dev:version:bump 5.1.0
```

## Pre-Commit Checklist for Claude Code

**IMPORTANT**: Before every commit, run these commands locally to catch issues before CI:

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test --verbose
```

This ensures:
1. **Formatting** - Code is formatted according to rustfmt standards (CI runs `cargo fmt --all -- --check`)
2. **Linting** - No clippy warnings (CI runs with `-D warnings` which treats warnings as errors)
3. **Tests** - All tests pass (use `--verbose` to match CI output)

**Why this matters**: The CI enforces these checks with `-D warnings`, meaning any warning becomes a build failure. Running locally first saves CI cycles and iteration time.

### CI Workflow Matches

The CI workflow (`.github/workflows/build.yml`) runs three jobs:

1. **Check job**:
   - `cargo check --all-targets --verbose`
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo fmt --all -- --check`

2. **Test job**:
   - `cargo test --verbose`

3. **Build job** (matrix: x86_64-unknown-linux-gnu, aarch64-unknown-linux-musl):
   - `cargo build --release --target $TARGET`

**Common pitfalls**:
- **Trait imports**: Always explicitly import traits needed for method calls (e.g., `use chrono::TimeZone`). macOS and Linux environments may have different implicit imports.
- **Platform differences**: Tests that skip on macOS (no I2C hardware) must also handle Linux CI (no I2C hardware). Use `match HalpiDevice::new() { Ok(d) => d, Err(_) => return }` pattern.
- **Verbose output**: Use `cargo test --verbose` to match CI output format and catch edge cases.

To run the exact same checks as CI locally:
```bash
# Run all CI checks sequentially
cargo check --all-targets --verbose && \
cargo clippy --all-targets -- -D warnings && \
cargo fmt --all -- --check && \
cargo test --verbose
```

## Architecture Overview

### Workspace Structure
- **halpid/**: Daemon binary (power monitor, watchdog, HTTP server)
- **halpi/**: CLI binary (communicates with daemon via Unix socket)
- **halpi-common/**: Shared library (data types, utilities)

### Core Components

#### Daemon (halpid)
- **I2C Communication**: Communicates with RP2040 firmware on I2C bus 1, address 0x6d
- **HTTP Server**: Axum-based server on Unix socket `/run/halpid/halpid.sock`
- **State Machine**: Power management state machine (0.1 second polling interval)
- **Watchdog**: Feeds hardware watchdog every ~5 seconds
- **Signal Handling**: Graceful shutdown on SIGTERM/SIGINT

#### CLI (halpi)
- **HTTP Client**: Connects to daemon via Unix socket
- **Clap-based**: Command-line argument parsing with subcommands
- **Commands**: status, config, shutdown, standby, usb, firmware

#### Shared Library (halpi-common)
- **Data Types**: Shared structures for API requests/responses
- **Error Types**: Common error handling
- **Constants**: I2C register addresses, API endpoints

### Key Technical Decisions

**Static Binary**
- Target: `aarch64-unknown-linux-musl`
- Universal ARM64 Linux compatibility
- No dynamic dependencies
- Lower memory footprint (~1-3MB less RAM vs dynamic)

**Async Runtime**
- Tokio for async I/O
- Axum for HTTP server
- Efficient resource utilization

**API Compatibility**
- **Hard constraint**: 100% backward compatibility with Python halpid 4.x
- I2C register addresses and protocols unchanged
- HTTP endpoint paths and JSON schemas unchanged
- CLI command structure and arguments unchanged
- Configuration file format and keys unchanged

**Behavioral Compatibility**
- State machine polling: **0.1 second interval** (not 1 second!)
- Watchdog feeding patterns identical to Python version
- Shutdown orchestration sequence identical
- Firmware update protocol identical

## Development Notes

### API Compatibility Testing
When implementing features, always verify compatibility with the Python version:
- Check endpoint paths match exactly
- Verify JSON request/response schemas match
- Test CLI command output format matches
- Ensure configuration file parsing is compatible

### I2C Communication
- Bus: I2C bus 1
- Address: 0x6d (HALPI2 RP2040 firmware)
- Registers: See `halpi-common/src/i2c.rs` for register definitions
- Reference: `halpid/src/i2c.py` in Python implementation

### State Machine Timing
**CRITICAL**: The state machine polls at **0.1 second intervals**, not 1 second. This is essential for correct power management behavior.

### Error Handling
- Use `anyhow::Result` for application errors
- Use `thiserror::Error` for library errors
- Include context with `.context()` when propagating errors
- Log errors with `tracing` before returning

### Testing
- Unit tests: Test individual functions and modules
- Integration tests: Test HTTP API endpoints
- Hardware tests: Run on actual HALPI2 hardware (see HALPI2-tests/)

### Cross-Compilation and Platform Support

**Important**: This daemon is Linux-only. It cannot be built natively on macOS or other platforms due to I2C hardware dependencies (the `i2cdev` crate requires Linux).

**Development Workflow**:
- **On macOS**: Use `./run build:cross` which compiles FOR Linux (target_os = "linux"), so all Linux-specific code is active
- **Testing on macOS**: Run tests in Docker container (Linux environment)
- **On Linux**: Native builds work fine with `./run build`

**Why no cfg(target_os = "linux") guards?**
- Cross-compilation: When running `cross build --target aarch64-unknown-linux-musl`, the **target** OS is Linux (not the host OS)
- This means `cfg(target_os = "linux")` is **true** during cross-compilation from macOS
- Docker tests: Running in Linux container means target_os is Linux
- Therefore, cfg guards add complexity without benefit

**Native macOS builds**: Will fail with "i2cdev::linux not found" - this is expected and correct behavior for Linux-only software.

## File Structure

```
.
├── Cargo.toml              # Workspace manifest
├── halpid/                 # Daemon crate
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs         # Entry point
│       ├── i2c.rs          # I2C device interface
│       ├── server.rs       # HTTP server
│       ├── state.rs        # State machine
│       └── config.rs       # Configuration
├── halpi/                  # CLI crate
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs         # Entry point and CLI parsing
│       └── client.rs       # HTTP client for Unix socket
├── halpi-common/           # Shared library
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── types.rs        # Shared data types
│       └── error.rs        # Error types
├── docs/
│   ├── SPEC.md            # Technical specification
│   └── ARCHITECTURE.md    # System architecture
├── .github/
│   └── workflows/         # CI/CD workflows
└── debian/                # Debian packaging files
```

## Related Projects

- **HALPI2-firmware**: RP2040 embedded firmware (Rust/Embassy) - `/Users/mairas/w/hatlabs/HALPI2/HALPI2-firmware/`
- **halpid**: Python daemon (reference implementation) - `/Users/mairas/w/hatlabs/HALPI2/halpid/`
- **HALPI2-tests**: Hardware production tests - `/Users/mairas/w/hatlabs/HALPI2/HALPI2-tests/`
- **halpi2**: User documentation - `/Users/mairas/w/hatlabs/HALPI2/halpi2/`

## Resources

- Product page: https://shop.hatlabs.fi/products/halpi2
- Documentation: https://docs.hatlabs.fi/halpi2
- GitHub Issues: https://github.com/hatlabs/HALPI2-rust-daemon/issues
- Python Reference: `/Users/mairas/w/hatlabs/HALPI2/halpid/`
- Firmware Reference: `/Users/mairas/w/hatlabs/HALPI2/HALPI2-firmware/`

## Important Constraints

- **Runtime**: Linux-only (requires I2C device access). Development and testing on macOS is supported via cross-compilation and Docker containers for non-hardware tests.
- **MSRV**: Rust 1.91+ (required for edition 2024)
- **Target**: aarch64-unknown-linux-musl for deployment, x86_64-unknown-linux-gnu for Linux testing
- **API Compatibility**: 100% backward compatible with Python halpid 4.x
- **State Machine**: 0.1 second polling interval
- **Runs as root**: Required for I2C access and shutdown privileges
