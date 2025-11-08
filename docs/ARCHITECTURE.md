# System Architecture: halpid Rust Reimplementation

## Overview

This document describes the architecture of the Rust reimplementation of `halpid`, the HALPI2 power monitoring and watchdog daemon. The system follows a modular, layered architecture with clear separation of concerns:

```
┌─────────────────────────────────────────────────────────┐
│  CLI (halpi)                                            │
│  • User commands                                        │
│  • Pretty output formatting                             │
└──────────────────────┬──────────────────────────────────┘
                       │ HTTP over Unix Socket
┌──────────────────────┴──────────────────────────────────┐
│  Daemon (halpid)                                        │
│  ┌────────────────────────────────────────────────────┐ │
│  │  HTTP API Server (axum)                            │ │
│  │  • REST endpoints                                  │ │
│  │  • Request validation                              │ │
│  └─────────────┬──────────────────────────────────────┘ │
│                │                                         │
│  ┌─────────────┴──────────────────────────────────────┐ │
│  │  State Machine                                     │ │
│  │  • Power monitoring                                │ │
│  │  • Blackout detection                              │ │
│  │  • Shutdown orchestration                          │ │
│  └─────────────┬──────────────────────────────────────┘ │
│                │                                         │
│  ┌─────────────┴──────────────────────────────────────┐ │
│  │  I2C Communication Layer                           │ │
│  │  • Register read/write                             │ │
│  │  • DFU protocol                                    │ │
│  │  • Error handling                                  │ │
│  └─────────────┬──────────────────────────────────────┘ │
└────────────────┼────────────────────────────────────────┘
                 │ I2C Bus 1, Address 0x6D
┌────────────────┴────────────────────────────────────────┐
│  RP2040 Firmware (HALPI2-firmware)                      │
│  • Power management state machine                       │
│  • GPIO control (power rails, USB, LEDs)                │
│  • Analog monitoring (voltages, current, temperature)   │
└─────────────────────────────────────────────────────────┘
```

## System Components

### 1. I2C Communication Layer

**Module**: `src/i2c/`

**Purpose**: Low-level communication with RP2040 firmware over I2C

**Components**:
- `device.rs` - Main `HalpiDevice` struct, high-level operations
- `registers.rs` - Register address constants and data types
- `protocol.rs` - Read/write primitives, encoding/decoding
- `dfu.rs` - Firmware update protocol implementation
- `error.rs` - I2C-specific error types

**Key Types**:

- **HalpiDevice** - Main device interface containing the Linux I2C device handle, bus number, device address, and cached firmware version for optimization
- **Register** - Enumeration of all I2C register addresses (0x03 through 0x45) for type-safe register access
- **Measurements** - Structure holding all sensor readings (input voltage, supercap voltage, input current, MCU temperature, PCB temperature) and current power state

**Responsibilities**:
- Open I2C device (`/dev/i2c-{bus}`)
- Atomic register read/write operations
- Big-endian multi-byte value encoding/decoding
- Analog value scaling (16-bit → float)
- Firmware version detection (affects read methods)
- DFU block upload with CRC32 validation
- Error handling and retry logic

### 2. Data Models and Types

**Module**: `src/types/`

**Purpose**: Core domain types used throughout the system

**Components**:
- `state.rs` - Power management states enum
- `config.rs` - Configuration structures
- `measurements.rs` - Measurement data structures
- `version.rs` - Version parsing and comparison

**Key Types**:

- **PowerState** - Enumeration of all 14 firmware power states (PowerOff through Standby), serializable to JSON for API responses
- **Config** - Configuration structure with fields for I2C bus/address, blackout timing and voltage thresholds, Unix socket path and permissions, and poweroff command
- **Version** - Semantic version structure with major, minor, patch numbers and optional alpha designation (255 indicates release version)

### 3. HTTP API Server

**Module**: `src/server/`

**Purpose**: REST API over Unix domain socket for IPC

**Components**:
- `app.rs` - Axum application setup and routing
- `handlers/` - Endpoint handler functions
  - `health.rs` - `/` and `/version`
  - `shutdown.rs` - `/shutdown`, `/standby`
  - `config.rs` - `/config` and `/config/{key}`
  - `values.rs` - `/values` and `/values/{key}`
  - `usb.rs` - `/usb` and `/usb/{port}`
  - `flash.rs` - `/flash` (firmware upload)
- `state.rs` - Shared application state (`Arc<AppState>`)
- `error.rs` - HTTP error responses

**Key Types**:

- **AppState** - Shared application state containing thread-safe references to the I2C device (mutex-protected), configuration (read-write lock), and daemon version string
- **Handler functions** - Async functions that receive app state and return JSON responses or appropriate HTTP status codes with error messages

**Routing**:

- `GET /` - Health check endpoint
- `GET /version` - Daemon version information
- `POST /shutdown` - Initiate system shutdown
- `POST /standby` - Enter standby mode with RTC wakeup
- `GET /config` - Retrieve all configuration values
- `GET /config/{key}` - Retrieve specific configuration value
- `PUT /config/{key}` - Update configuration value
- `GET /values` - Retrieve all measurements and status
- `GET /values/{key}` - Retrieve specific measurement
- `GET /usb` - Get all USB port states
- `GET /usb/{port}` - Get specific USB port state
- `PUT /usb` - Set multiple USB port states
- `PUT /usb/{port}` - Set specific USB port state
- `POST /flash` - Upload firmware (multipart form data)

**Responsibilities**:
- Bind to Unix domain socket
- Set socket permissions (0660) and group ownership
- Route HTTP requests to handlers
- Serialize/deserialize JSON
- Coordinate access to shared `HalpiDevice` (via `Arc<Mutex<>>`)
- Return appropriate HTTP status codes and error messages

### 4. State Machine

**Module**: `src/state_machine/`

**Purpose**: Monitor power state and orchestrate graceful shutdown

**Components**:
- `machine.rs` - Main state machine implementation
- `states.rs` - State definitions and transitions
- `actions.rs` - State entry/exit actions

**Key Types**:

- **DaemonState** - Internal state enumeration with five states: Start (initialization), Ok (normal operation), Blackout (power loss detected), Shutdown (shutting down), Dead (waiting for power loss)
- **StateMachine** - Holds current state, thread-safe references to I2C device and configuration, and optional blackout start timestamp for duration tracking

**State Transitions**:
```
START
  │ entry: initialize watchdog (10s timeout)
  ↓
OK
  │ loop: monitor V_in
  │ if V_in < threshold → BLACKOUT
  ↓
BLACKOUT
  │ entry: record blackout start time
  │ loop: check V_in and elapsed time
  │ if V_in > threshold → OK
  │ if elapsed > time_limit → SHUTDOWN
  ↓
SHUTDOWN
  │ entry: call I2C shutdown (0x30)
  │ entry: execute poweroff command
  ↓
DEAD
  │ loop: wait for power loss
```

**Main Loop**:

The state machine runs an infinite async loop that dispatches to appropriate handler functions based on current state, then sleeps for one second between iterations. Each handler performs state-specific logic and may transition to a new state.

**Responsibilities**:
- Initialize watchdog on daemon startup
- Poll voltage measurements every second
- Detect blackout conditions
- Track blackout duration
- Trigger shutdown sequence when threshold exceeded
- Execute system poweroff command

### 5. Command-Line Interface

**Module**: `src/cli/`

**Purpose**: User-facing command-line tool

**Components**:
- `main.rs` - CLI entry point, argument parsing
- `commands/` - Command implementations
  - `status.rs` - Display system status
  - `config.rs` - Configuration management
  - `shutdown.rs` - Shutdown and standby
  - `usb.rs` - USB port control
  - `flash.rs` - Firmware upload
- `client.rs` - HTTP client for Unix socket communication
- `output.rs` - Formatted output (tables, colors)

**Argument Parsing**:

The CLI uses derive macros for argument parsing with a main structure that accepts an optional socket path and a required subcommand. Supported commands include status, version, get (retrieve specific value), config (with subcommands), shutdown (with standby options), usb (port control), and flash (firmware upload).

**HTTP Client**:

A client structure wraps the HTTP client and socket path, providing async methods for each API endpoint including retrieving values, setting configuration, controlling USB ports, and uploading firmware. All methods return typed results with appropriate error handling.

**Responsibilities**:
- Parse command-line arguments
- Connect to daemon via Unix socket
- Send HTTP requests and handle responses
- Format output for human readability
- Handle errors gracefully with clear messages
- Return appropriate exit codes

### 6. Daemon Orchestration

**Module**: `src/daemon/`

**Purpose**: Main daemon process coordination

**Components**:
- `main.rs` - Daemon entry point
- `runner.rs` - Concurrent task orchestration
- `signals.rs` - Signal handler (SIGINT, SIGTERM)
- `shutdown.rs` - Graceful shutdown coordination

**Main Function Flow**:

The daemon's main async function orchestrates startup and runtime in seven phases:

1. Load configuration from file and CLI arguments
2. Initialize structured logging subsystem
3. Open I2C device and wrap in thread-safe mutex
4. Create shared application state with device, config, and version
5. Spawn three concurrent async tasks (HTTP server, state machine, signal handler)
6. Wait for any task to complete using async select (typically signal handler)
7. Execute graceful shutdown cleanup routine

**Concurrent Tasks**:
1. **HTTP Server**: Axum server listening on Unix socket
2. **State Machine**: 1-second polling loop for power monitoring
3. **Signal Handler**: Listens for SIGINT/SIGTERM

**Graceful Shutdown**:
- Stop accepting new HTTP requests
- Cancel state machine loop
- Disable watchdog (I2C command)
- Remove Unix socket file
- Flush logs and exit

**Responsibilities**:
- Initialize all components
- Coordinate concurrent async tasks
- Handle shutdown signals
- Clean up resources on exit
- Ensure watchdog is disabled before exit

### 7. Configuration Management

**Module**: `src/config/`

**Purpose**: Load and manage daemon configuration

**Components**:
- `loader.rs` - Load from YAML file and CLI args
- `validation.rs` - Validate configuration values
- `defaults.rs` - Default configuration values

**Loading Precedence**:

Configuration is loaded in four stages: start with built-in defaults, merge values from YAML file if it exists, override with any CLI arguments provided, then validate the final configuration for correctness (ranges, required values, path existence).

**YAML Parsing**:
- Use `serde_yaml` for deserialization
- Convert dashes to underscores in keys
- Handle missing fields with defaults
- Validate types and ranges

**Responsibilities**:
- Parse YAML configuration file
- Merge configuration sources by precedence
- Validate configuration values
- Provide default values
- Normalize key names (dash → underscore)

## Technology Stack

### Core Dependencies

| Crate | Purpose | Rationale |
|-------|---------|-----------|
| **tokio** | Async runtime | Industry standard, excellent ecosystem, mature |
| **axum** | HTTP framework | Modern, ergonomic, built on hyper/tower, great with tokio |
| **serde** + **serde_json** + **serde_yaml** | Serialization | De facto standard for Rust serialization |
| **clap** | CLI parsing | Most popular, derive macros are ergonomic |
| **i2cdev** or **linux-embedded-hal** | I2C access | Linux I2C device interface |
| **anyhow** | Error handling | Simple error propagation for application code |
| **tracing** + **tracing-subscriber** | Logging | Structured logging, async-aware, great ecosystem |

### Supporting Dependencies

| Crate | Purpose |
|-------|---------|
| **reqwest** | HTTP client for CLI |
| **tokio-util** | Unix socket utilities |
| **tower** | Middleware (via axum) |
| **hyper** | HTTP (via axum) |
| **crc32fast** | CRC32 for DFU protocol |
| **once_cell** | Lazy static values |
| **chrono** or **time** | Date/time parsing for standby mode (basic ISO 8601 support sufficient) |
| **humantime** | Duration parsing for standby mode (e.g., "1h", "30m") |

**Note on standby time parsing**: Full compatibility with Python's `dateparser` library is not required. The Rust implementation should support common formats like integer seconds, ISO 8601 datetime strings, and simple duration expressions. Implementation can use standard Rust datetime parsing and `humantime` crate rather than attempting to replicate Python's flexible parsing.

### Build Tools

| Tool | Purpose |
|------|---------|
| **cargo** | Build system |
| **cargo-deb** | Debian package generation |
| **cross** | Cross-compilation tool for building on x86_64 or ARM64 hosts |
| **rustfmt** | Code formatting |
| **clippy** | Linting |
| **cargo-nextest** | Test runner |
| **cargo-watch** | Development file watching |

### Build Configuration

**Target**: `aarch64-unknown-linux-musl` (static binary)

**Rationale for static linking with musl:**
- **Universal compatibility** - Single binary works on any ARM64 Linux distribution (Raspberry Pi OS, Victron OS, Alpine, Ubuntu, Debian variants, etc.)
- **No runtime dependencies** - Eliminates glibc version conflicts and dependency issues
- **Lower memory usage** - musl's simpler allocator typically uses 1-3 MB less RAM than glibc (~7-10 MB RSS vs 10-15 MB)
- **Simplified distribution** - Single binary for all Linux variants, ideal for GitHub releases
- **Predictable behavior** - No system library version surprises across different OS installations
- **Binary size trade-off acceptable** - ~1-2 MB larger binary is negligible on modern systems with multi-GB storage

**Cross-Compilation Setup:**

Development typically happens on x86_64 or ARM64 hosts, cross-compiling to ARM64 Linux target.

**Option 1: Using `cross` (recommended for simplicity):**

Install the cross tool via cargo, then build using the cross command targeting aarch64-unknown-linux-musl. No additional toolchain setup needed as cross handles everything in Docker containers.

**Option 2: Native toolchain (for faster builds):**

Install the musl target via rustup, then install a platform-specific linker (musl-tools on Ubuntu/Debian, filosottile musl-cross on macOS, or zig as a cross-platform alternative). Configure the linker in .cargo/config.toml, then build normally with cargo targeting aarch64-unknown-linux-musl.

**Development Workflow:**

1. Build on development machine (x86_64 or ARM64)
2. Transfer binary to Raspberry Pi using scp to a test location
3. Test on actual HALPI2 hardware (I2C requires real hardware)
4. Iterate based on test results

**Build Process:**

- Development builds: Use cross or cargo with the musl target
- Release builds: Add --release flag for optimizations
- CPU-specific optimizations: Set RUSTFLAGS with target-cpu=cortex-a72 for Raspberry Pi CM5

**No dynamic builds needed** - Static musl binary covers all deployment scenarios, including:
- Standard Raspberry Pi OS installations
- Alternative distributions (Victron OS, etc.)
- Container environments
- Minimal/embedded Linux systems

## Data Flow and Integration Points

### 1. CLI → Daemon Communication

```
User runs: halpi status
    ↓
CLI parses arguments (clap)
    ↓
CLI creates HTTP client (reqwest)
    ↓
CLI sends GET /values to Unix socket
    ↓
Daemon's axum server receives request
    ↓
Handler acquires lock on HalpiDevice
    ↓
Handler reads I2C registers
    ↓
Handler releases lock
    ↓
Handler returns JSON response
    ↓
CLI receives JSON
    ↓
CLI formats output (pretty table)
    ↓
User sees formatted status
```

### 2. State Machine → Shutdown Flow

```
State machine polls V_in every 1 second
    ↓
V_in < 9.0V (blackout detected)
    ↓
State: OK → BLACKOUT
Record blackout_start = Instant::now()
    ↓
Continue polling...
    ↓
Elapsed time > 5.0 seconds
    ↓
State: BLACKOUT → SHUTDOWN
    ↓
Acquire lock on HalpiDevice
    ↓
Write 0x01 to register 0x30 (I2C shutdown request)
    ↓
Release lock
    ↓
Execute command: /sbin/poweroff
    ↓
State: SHUTDOWN → DEAD
    ↓
Wait for system to power down
```

### 3. Firmware Upload Flow

```
User runs: halpi flash firmware.bin
    ↓
CLI reads firmware file into memory
    ↓
CLI sends POST /flash with multipart form data
    ↓
Daemon handler receives file
    ↓
Handler validates file size and format
    ↓
Handler acquires lock on HalpiDevice
    ↓
Handler calls device.start_dfu(total_size)
    ↓
For each 4KB block:
  - Calculate CRC32
  - Upload block via I2C register 0x43
  - Poll DFU status (register 0x41)
  - Wait if QUEUE_FULL, retry
  - Abort if error state
    ↓
Handler calls device.commit_dfu()
    ↓
Handler releases lock
    ↓
Handler returns 204 No Content (success)
    ↓
CLI displays success message
```

## Deployment Architecture

### Distribution Model

**Static binary distribution** - Single self-contained binary works across all ARM64 Linux distributions:
- Raspberry Pi OS (Bookworm, Bullseye)
- Victron OS (Venus OS)
- Alpine Linux
- Ubuntu/Debian variants
- Any ARM64 Linux with kernel 4.4+

**Distribution channels:**
1. **Debian package** (`.deb`) - Recommended for Raspberry Pi OS and Debian-based systems
2. **Raw binary** - GitHub releases for alternative distributions (Victron OS, etc.)
3. **Container image** - Future enhancement for containerized deployments

### System Integration

```
┌─────────────────────────────────────────────────────────┐
│  Systemd                                                │
│  ┌────────────────────────────────────────────────────┐ │
│  │  halpid.service                                    │ │
│  │  Type=simple                                       │ │
│  │  ExecStart=/usr/bin/halpid                         │ │
│  │  Restart=on-failure                                │ │
│  │  RestartSec=10                                     │ │
│  │  User=root                                         │ │
│  └────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
                         │
                         ↓
┌─────────────────────────────────────────────────────────┐
│  File System                                            │
│  /usr/bin/halpid          (static binary - daemon)      │
│  /usr/bin/halpi           (static binary - CLI)         │
│  /etc/halpid/halpid.conf  (YAML config)                 │
│  /run/halpid.sock         (Unix socket, 0660, group=adm)│
│  /lib/systemd/system/halpid.service  (systemd unit)     │
└─────────────────────────────────────────────────────────┘
                         │
                         ↓
┌─────────────────────────────────────────────────────────┐
│  Kernel                                                 │
│  /dev/i2c-1               (I2C device node)             │
│  /dev/rtc0                (RTC for standby mode)        │
└─────────────────────────────────────────────────────────┘
```

### Debian Package Structure

```
halpid_5.0.0_arm64.deb
├── usr/bin/halpid                    (daemon binary)
├── usr/bin/halpi                     (CLI binary)
├── etc/halpid/halpid.conf            (default config)
├── lib/systemd/system/halpid.service (systemd unit)
└── DEBIAN/
    ├── control                       (package metadata)
    ├── postinst                      (post-install script)
    └── prerm                         (pre-removal script)
```

**Package Metadata**:
```
Package: halpid
Version: 5.0.0
Architecture: arm64
Conflicts: halpid (<< 5.0.0)
Description: HALPI2 power monitoring and watchdog daemon (static binary)
```

**Note**: No `libc6` dependency - binaries are statically linked with musl libc and have no runtime library dependencies.

**Post-Install Script**:
- Enable systemd service
- Start daemon
- Create `adm` group if needed

**Pre-Removal Script**:
- Stop daemon
- Disable systemd service

## Security Considerations

### Privilege Requirements

**Daemon (halpid)**:
- **Must run as root**:
  - I2C device access (`/dev/i2c-1` requires root or `i2c` group)
  - System shutdown (`/sbin/poweroff` requires root)
  - Unix socket in `/var/run` (requires root)

**CLI (halpi)**:
- **Can run as any user in socket group** (default: `adm`)
- Socket permissions: 0660 (owner + group read/write)

### Attack Surface

**External Inputs**:
1. **CLI arguments** - Validated by clap, type-safe
2. **HTTP requests** - Validated by axum handlers, JSON schema
3. **Configuration file** - Parsed by serde_yaml, validated after load
4. **I2C responses** - Binary data, validate lengths and ranges

**Mitigations**:
- Input validation at boundaries
- Type safety (Rust)
- Bounds checking on array access
- CRC validation for firmware uploads
- No arbitrary command execution (poweroff command is configurable but validated)

### Dependency Security

- Regular `cargo audit` runs
- Pin dependencies in `Cargo.lock`
- Minimal dependency tree
- Prefer well-maintained, popular crates

## File Tree Structure

```
HALPI2-rust-daemon/
├── Cargo.toml                   # Workspace root
├── Cargo.lock                   # Dependency lock file
├── README.md                    # Project overview
├── LICENSE                      # License file
├── CLAUDE.md                    # Claude Code instructions
├── .gitignore
├── .cargo/
│   └── config.toml              # Cross-compilation configuration
├── .github/
│   └── workflows/
│       ├── ci.yml               # Build and test
│       └── release.yml          # Debian package build
│
├── docs/
│   ├── SPEC.md                  # Technical specification
│   ├── ARCHITECTURE.md          # This file
│   └── MIGRATION.md             # Python → Rust migration guide
│
├── halpid/                      # Daemon binary crate
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs              # Daemon entry point
│       ├── config/              # Configuration management
│       │   ├── mod.rs
│       │   ├── loader.rs
│       │   ├── validation.rs
│       │   └── defaults.rs
│       ├── daemon/              # Daemon orchestration
│       │   ├── mod.rs
│       │   ├── runner.rs
│       │   ├── signals.rs
│       │   └── shutdown.rs
│       ├── i2c/                 # I2C communication layer
│       │   ├── mod.rs
│       │   ├── device.rs
│       │   ├── registers.rs
│       │   ├── protocol.rs
│       │   ├── dfu.rs
│       │   └── error.rs
│       ├── server/              # HTTP API server
│       │   ├── mod.rs
│       │   ├── app.rs
│       │   ├── state.rs
│       │   ├── error.rs
│       │   └── handlers/
│       │       ├── mod.rs
│       │       ├── health.rs
│       │       ├── shutdown.rs
│       │       ├── config.rs
│       │       ├── values.rs
│       │       ├── usb.rs
│       │       └── flash.rs
│       ├── state_machine/       # Power management state machine
│       │   ├── mod.rs
│       │   ├── machine.rs
│       │   ├── states.rs
│       │   └── actions.rs
│       └── types/               # Shared data types
│           ├── mod.rs
│           ├── state.rs
│           ├── config.rs
│           ├── measurements.rs
│           └── version.rs
│
├── halpi/                       # CLI binary crate
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs              # CLI entry point
│       ├── client.rs            # HTTP client
│       ├── output.rs            # Formatted output
│       └── commands/            # Command implementations
│           ├── mod.rs
│           ├── status.rs
│           ├── config.rs
│           ├── shutdown.rs
│           ├── usb.rs
│           └── flash.rs
│
├── halpi-common/                # Shared library crate
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── types.rs             # Common types
│       ├── protocol.rs          # I2C protocol constants
│       └── error.rs             # Common error types
│
├── debian/                      # Debian packaging
│   ├── control                  # Package metadata
│   ├── changelog                # Debian changelog
│   ├── rules                    # Build rules
│   ├── halpid.install           # File installation map
│   ├── halpid.service           # Systemd unit file
│   ├── halpid.postinst          # Post-install script
│   ├── halpid.prerm             # Pre-removal script
│   └── copyright                # Copyright information
│
├── config/
│   └── halpid.conf              # Default YAML config
│
└── tests/                       # Integration tests
    ├── integration_test.rs      # Full system tests
    ├── api_test.rs              # HTTP API tests
    └── fixtures/
        └── test_firmware.bin    # Test firmware file
```

## Error Handling Strategy

### Error Types Hierarchy

The error handling uses a hierarchical approach with specialized error types:

- **AppError** - Top-level application errors with automatic conversion from I2C, configuration, and server errors, plus a device-not-found variant and catch-all for other errors
- **I2cError** - I2C-specific errors including device errors (from underlying I2C library), register read/write failures, and DFU operation errors
- **ServerError** - HTTP-specific errors mapped to status codes: BadRequest (400), NotFound (404), and Internal (500)

### Error Handling Patterns

**I2C Layer**:

Implements retry logic for transient errors. Register read operations attempt up to 3 times with 10ms delays between attempts for transient failures. Persistent errors are propagated immediately.

**HTTP Handlers**:

Server errors implement automatic conversion to HTTP responses with appropriate status codes and JSON error messages. Each error variant maps to a specific HTTP status code with the error message in the response body.

**CLI**:

The main function catches errors from the run function, prints user-friendly error messages to stderr, and exits with code 1. This provides clear feedback without exposing technical implementation details.

## Testing Strategy

### Unit Tests

- **I2C layer**: Mock I2C device, test encoding/decoding
- **Data types**: Test serialization, validation
- **Configuration**: Test loading, merging, validation
- **State machine**: Test transitions, timing

### Integration Tests

- **HTTP API**: Test all endpoints with test server
- **CLI**: Test commands with mock daemon
- **DFU**: Test firmware upload with simulated device

### Hardware Tests

- **On actual HALPI2 hardware**:
  - I2C communication
  - Analog readings
  - Watchdog feeding
  - State machine transitions
  - Firmware upload

### Continuous Integration

- GitHub Actions workflow
- Build on ARM64 (cross-compile or native)
- Run unit and integration tests
- Build Debian package
- Run `cargo clippy` and `cargo fmt --check`
- Run `cargo audit`

## Performance Considerations

### Memory Usage

**Target**: <10MB RSS

**Strategies**:
- Use `Arc` for shared data (avoid cloning)
- Stream firmware uploads (don't load entire file)
- Limit log buffer size
- Use compact data structures

### CPU Usage

**Target**: <1% CPU during idle

**Strategies**:
- Event-driven HTTP server (tokio)
- Sleep between state machine polls (1 second)
- Efficient I2C operations (atomic, no polling)

### Latency

**Targets**:
- I2C read/write: <1ms
- HTTP API response: <5ms
- CLI command: <50ms total

**Strategies**:
- Use async I/O (tokio)
- Minimize lock contention (short critical sections)
- Optimize hot paths (state machine loop)

## Future Enhancements

*Not in scope for v5.0.0, but documented for future reference:*

### Monitoring and Metrics

- Prometheus metrics endpoint
- Grafana dashboard
- Alerting on blackout events

### Enhanced Features

- Configuration hot-reload (SIGHUP)
- Multiple concurrent firmware uploads
- Prometheus metrics export
- JSON output mode for CLI (scripting)
- systemd socket activation

### Performance Optimizations

- Zero-copy I2C operations
- Lock-free shared state (if measurable contention)
- Profile-guided optimization (PGO)

## Summary

This architecture provides:

✅ **Modularity** - Clear separation of concerns
✅ **Testability** - Mockable interfaces, isolated components
✅ **Performance** - Async I/O, efficient resource usage
✅ **Reliability** - Strong typing, error handling, graceful degradation
✅ **Maintainability** - Idiomatic Rust, well-documented
✅ **Compatibility** - Identical external interfaces to Python version

The implementation follows Rust best practices while preserving the proven design of the Python version.
