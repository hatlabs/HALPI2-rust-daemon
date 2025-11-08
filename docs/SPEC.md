# Technical Specification: halpid Rust Reimplementation

## Project Overview

This project reimplements the HALPI2 power monitoring and watchdog daemon (`halpid`) in Rust. The current Python implementation provides robust functionality but has opportunities for improvement in performance, reliability, and resource utilization. This Rust reimplementation maintains complete API compatibility while delivering:

- **Lower memory footprint** - No Python interpreter overhead
- **Faster startup time** - Native binary execution
- **Improved reliability** - Rust's type safety and memory safety guarantees
- **Better resource utilization** - More efficient system resource usage

## Goals

### Primary Goals

1. **API Compatibility**: Maintain 100% backward compatibility with existing interfaces:
   - I2C register map and communication protocol
   - HTTP REST API (Unix socket endpoints)
   - CLI commands and arguments
   - Configuration file format (YAML)

2. **Behavioral Equivalence**: Preserve existing system behavior:
   - Power management state machine logic
   - Watchdog feeding patterns
   - Blackout detection and shutdown orchestration
   - Firmware update (DFU) protocol

3. **Performance Improvements**:
   - Reduce memory footprint from ~50MB (Python + dependencies) to <10MB
   - Improve startup time from ~2s to <100ms
   - Maintain responsive I2C communication and HTTP API

4. **Reliability**:
   - Leverage Rust's type safety to eliminate entire classes of runtime errors
   - Ensure safe concurrent access to shared resources
   - Graceful error handling with proper recovery

### Secondary Goals

1. **Maintainability**: Clear, idiomatic Rust code following best practices
2. **Testing**: Comprehensive unit and integration tests
3. **Documentation**: Well-documented code and user-facing documentation
4. **Packaging**: Debian package (.deb) for easy deployment

## Core Features

### 1. I2C Communication Layer

**Purpose**: Communicate with RP2040 firmware over I2C bus

**Requirements**:
- I2C bus 1, device address 0x6D (configurable)
- Support for all existing I2C registers (0x03-0x45)
- Atomic read/write operations using Linux I2C device interface
- Proper error handling and retry logic for transient I2C errors

**Register Categories**:
- Version and identification (hardware version, firmware version, device ID)
- Power control and status (output enable, watchdog, thresholds, state)
- Analog measurements (voltages, current, temperatures)
- Shutdown commands
- Firmware update (DFU) protocol
- USB port control

**Data Encoding**:
- Multi-byte values: Big-endian encoding
- Analog values: 16-bit scaled (value = raw / 65536.0 * scale)
- Temperatures: Kelvin (displayed as Celsius in CLI)
- Firmware version detection affects read methods (byte vs word in v1 vs v2+)

### 2. HTTP REST API Server

**Purpose**: Provide IPC interface for CLI and external tools

**Requirements**:
- HTTP server on Unix domain socket
- Default path: `/var/run/halpid.sock` (root) or `~/.halpid.sock` (non-root)
- Socket permissions: 0660, group ownership configurable (default: `adm`)
- JSON request/response format
- Async I/O for concurrent request handling

**Endpoints** (must match exactly):
- `GET /` - Health check
- `GET /version` - Daemon version
- `POST /shutdown` - Initiate system shutdown
- `POST /standby` - Enter standby mode with RTC wakeup
- `GET /config` - Get all configuration
- `GET /config/{key}` - Get specific config value
- `PUT /config/{key}` - Set config value
- `GET /values` - Get all measurements and state
- `GET /values/{key}` - Get specific value
- `GET /usb` - Get all USB port states
- `GET /usb/{port}` - Get specific USB port state
- `PUT /usb` - Set multiple USB ports
- `PUT /usb/{port}` - Set specific USB port
- `POST /flash` - Upload firmware (multipart form data)

### 3. Command-Line Interface (CLI)

**Purpose**: User-friendly interface for system management

**Requirements**:
- Binary name: `halpi` (user-facing CLI tool)
- Communicates with daemon via Unix socket HTTP API
- Pretty-printed output using tables and formatting
- Exit codes: 0 (success), 1 (error)

**Commands**:
- `halpi status` - Show all measurements and state
- `halpi version` - Show CLI version
- `halpi get <key>` - Get specific value
- `halpi config` - Show all config
- `halpi config get <key>` - Get config value
- `halpi config set <key> <value>` - Set config value
- `halpi shutdown` - Normal shutdown
- `halpi shutdown --standby --time <time>` - Standby with wakeup
- `halpi usb` - Show USB port states
- `halpi usb enable <0-3|all>` - Enable USB port(s)
- `halpi usb disable <0-3|all>` - Disable USB port(s)
- `halpi flash <file>` - Upload firmware

**Standby Time Parsing**:
- The Rust CLI should support common time formats (integer seconds, ISO 8601 datetime)
- Full compatibility with Python's `dateparser` library is **not required**
- Implementation can use simpler, more predictable parsing (e.g., `humantime` crate for durations, standard datetime parsing)
- Document supported formats clearly in help text

**Output Format**:
- Human-readable tables and formatting (can differ from Python implementation)
- Machine-parseable output modes (consider adding --json flag)
- Colored output when TTY detected

### 4. Power Management State Machine

**Purpose**: Monitor power state and orchestrate graceful shutdown

**Requirements**:
- Four-state FSM: START → OK → BLACKOUT → SHUTDOWN → DEAD
- Poll interval: 0.1 seconds
- Blackout detection: `V_in < blackout_voltage_limit` (default 9.0V)
- Shutdown trigger: Blackout duration exceeds `blackout_time_limit` (default 5.0s)
- Watchdog initialization: Set 10-second timeout on startup
- Graceful shutdown sequence:
  1. Call I2C shutdown command (register 0x30)
  2. Execute poweroff command (default `/sbin/poweroff`)

**State Transitions**:
- `START → OK`: After watchdog initialization
- `OK → BLACKOUT`: When V_in drops below threshold
- `BLACKOUT → OK`: When V_in recovers above threshold
- `BLACKOUT → SHUTDOWN`: After timeout expires
- `SHUTDOWN → DEAD`: After poweroff command execution

### 5. Firmware Update (DFU) Support

**Purpose**: Update RP2040 firmware via I2C

**Requirements**:
- 4KB block size (matches flash sector)
- CRC32 validation per block
- State machine: IDLE → PREPARING → UPDATING → READY_TO_COMMIT
- Error states: QUEUE_FULL, CRC_ERROR, DATA_LENGTH_ERROR, WRITE_ERROR, PROTOCOL_ERROR
- Retry logic with timeout for QUEUE_FULL
- Progress reporting
- Abort capability

**Protocol**:
1. Start update (0x40): Send total firmware size
2. Upload blocks (0x43): [CRC32][block_num][block_len][data]
3. Poll status (0x41): Check DFU state
4. Commit (0x44): Finalize update
5. Abort (0x45): Cancel on error

### 6. Configuration Management

**Purpose**: Support flexible configuration from file and CLI

**Requirements**:
- Config file format: YAML
- Default location: `/etc/halpid/halpid.conf`
- Command-line override support for all options
- Key name normalization: dashes to underscores

**Configuration Options**:
- `i2c-bus` (int): I2C bus number (default: 1)
- `i2c-addr` (hex): I2C device address (default: 0x6d)
- `blackout-time-limit` (float): Seconds before shutdown (default: 5.0)
- `blackout-voltage-limit` (float): Voltage threshold in volts (default: 9.0)
- `socket` (path): Unix socket path (default: `/run/halpid.sock`)
- `socket-group` (string): Socket group ownership (default: `adm`)
- `poweroff` (string): Shutdown command (default: `/sbin/poweroff`)

**Precedence**: CLI args > Config file > Built-in defaults

### 7. Daemon Process Management

**Purpose**: Run as system service with proper lifecycle management

**Requirements**:
- Binary name: `halpid` (daemon)
- Run as root (required for I2C access and shutdown)
- Three concurrent async tasks:
  1. State machine loop (1 second interval)
  2. HTTP server (event-driven)
  3. Signal handler (SIGINT, SIGTERM)
- Graceful shutdown:
  - Disable watchdog on exit
  - Remove socket file
  - Clean up resources
- Systemd integration:
  - Service type: `simple`
  - Auto-restart on failure (10s delay)
  - Unbuffered output for logging

## Technical Requirements

### Rust Edition and Dependencies

- **Rust Edition**: 2024
- **MSRV** (Minimum Supported Rust Version): 1.91+ (or latest stable)

### Core Dependencies

- **tokio** - Async runtime with full features (rt-multi-thread, macros, fs, signal)
- **axum** - HTTP framework for Unix socket server
- **serde** / **serde_json** / **serde_yaml** - Serialization for config and API
- **clap** - CLI argument parsing (derive feature)
- **linux-embedded-hal** or **i2cdev** - I2C device access
- **anyhow** - Error handling
- **tracing** / **tracing-subscriber** - Structured logging
- **tokio-util** - Unix socket utilities

### Build and Packaging

- **cargo-deb** - Debian package generation
- **cross** - Cross-compilation tool for building on x86_64 or ARM64 development machines
- Build target: `aarch64-unknown-linux-musl` (static binary for universal ARM64 Linux compatibility)
- Cross-compilation: Required from initial phase for development workflow
- Package name: `halpid` (version 5.0.0 to signal major rewrite)

### Development Dependencies

- **cargo-watch** - Development file watching
- **cargo-nextest** - Modern test runner
- **mockall** - Mocking for unit tests (if needed)
- **criterion** - Benchmarking (optional)

## Constraints and Assumptions

### Hard Constraints

1. **API Compatibility**: External interfaces must remain unchanged
   - I2C register addresses and protocols
   - HTTP endpoint paths and JSON schemas
   - CLI command structure and arguments
   - Configuration file format and keys

2. **Behavioral Compatibility**: System behavior must match
   - State machine timing and transitions
   - Watchdog feeding patterns
   - Shutdown orchestration sequence
   - Firmware update protocol

3. **Platform Requirements**:
   - Linux kernel with I2C device support (`/dev/i2c-*`)
   - Unix domain socket support
   - Systemd for service management
   - Root privileges for I2C access and shutdown

### Assumptions

1. **Deployment**: Target is Raspberry Pi CM5 running Debian/Trixie ARM64
2. **I2C Hardware**: RP2040 firmware implements expected protocol (HALPI2-firmware)
3. **Permissions**: Daemon runs as root; CLI can run as any user in socket group
4. **Filesystem**: Standard Linux filesystem layout (`/etc`, `/var/run`, `/sbin`)
5. **RTC**: `rtcwake` utility available for standby mode support

## Non-Functional Requirements

### Performance

- **Memory footprint**: <10MB RSS during normal operation
- **Startup time**: <100ms from exec to ready
- **I2C latency**: <1ms per register read/write
- **HTTP response time**: <5ms for simple queries
- **State machine responsiveness**: 0.1 second polling interval

### Reliability

- **Crash resistance**: No panics in normal operation; graceful error handling
- **Recovery**: Automatic retry for transient I2C errors
- **Data integrity**: CRC validation for firmware uploads
- **Watchdog safety**: Always disable watchdog before daemon exit

### Security

- **Privilege separation**: CLI communicates via socket (no root required)
- **Socket permissions**: Restrictive (0660) with configurable group
- **Input validation**: Sanitize all external inputs (CLI args, HTTP requests, config)
- **Dependency audit**: Regular security audits with `cargo audit`

### Maintainability

- **Code style**: Follow Rust conventions (`rustfmt`, `clippy`)
- **Documentation**: Rustdoc for all public APIs
- **Testing**: >70% code coverage target
- **Error messages**: Clear, actionable error messages

## Out of Scope

### Initial Phase (v5.0.0)

The following are explicitly **not** included in the initial implementation:

1. **New features** - Only reimplementation of existing functionality
2. **Prometheus/metrics export** - Could be added in future versions
3. **systemd socket activation** - Not required for current use case
4. **Configuration hot-reload** - Requires daemon restart for config changes
5. **IPv4/IPv6 HTTP API** - Unix socket only (security)
6. **Multi-instance support** - Single daemon per system

### Bug Fixes During Reimplementation

If bugs are discovered in the Python implementation:
- **Document** the bug and current behavior
- **Discuss** with maintainer before fixing
- **Preserve** buggy behavior if external tools depend on it (add compatibility flag)
- **Fix** if clearly incorrect and unlikely to break downstream users

## Success Criteria

### Functional Completeness

- [ ] All I2C registers readable/writable with correct encoding
- [ ] All HTTP API endpoints functional with matching JSON schemas
- [ ] All CLI commands produce equivalent output (format can differ aesthetically)
- [ ] State machine transitions match Python implementation
- [ ] Firmware update completes successfully
- [ ] Configuration loading from YAML and CLI overrides works
- [ ] Systemd service starts, runs, and stops cleanly

### Quality Metrics

- [ ] Unit tests for all core modules (>70% coverage)
- [ ] Integration tests for HTTP API endpoints
- [ ] I2C hardware tests (run on actual HALPI2 hardware)
- [ ] Memory usage <10MB RSS
- [ ] No `clippy::pedantic` warnings
- [ ] Documentation complete (README, rustdoc, man pages)

### Deployment

- [ ] Debian package builds successfully
- [ ] Package installs and upgrades from Python version cleanly
- [ ] Systemd service runs without errors
- [ ] CLI accessible to users in `adm` group
- [ ] Migration guide documented

## Migration Strategy

### Package Transition

1. **New package version**: 5.0.0 (major version bump)
2. **Package name**: `halpid` (same as Python version)
3. **Binary names**: `halpid` (daemon), `halpi` (CLI)
4. **Conflicts**: Debian package conflicts with old `halpid` (<5.0.0)
5. **Upgrade path**: `apt upgrade` replaces Python with Rust version
6. **Rollback**: Downgrade via `apt install halpid=4.x.x`

### Compatibility Testing

Before release:
- [ ] Test on HALPI2 hardware with all firmware versions (v2.x, v3.x)
- [ ] Verify existing scripts using CLI continue to work
- [ ] Test HTTP API with external tools (if any)
- [ ] Validate configuration migration (old YAML files still work)

## References

- Python implementation: `halpid` repository (HALPI2-daemon)
- Firmware specification: `HALPI2-firmware` repository
- I2C protocol: Defined in firmware source (`src/i2c_regs.rs`)
- Product documentation: https://docs.hatlabs.fi/halpi2
