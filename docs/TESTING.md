# Testing Strategy

## Overview

This document outlines the testing strategy for the HALPI2 Rust Daemon project.

## Test Coverage Summary

Current test coverage (as of last update):
- **halpi**: 18 unit tests
  - 14 CLI command parsing tests
  - 4 HTTP client construction tests
- **halpi-common**: 42 unit tests
  - Protocol encoding/decoding tests
  - Type conversion tests
  - JSON serialization tests
- **halpid**: 13 unit tests
  - 11 CLI argument parsing tests
  - 1 health endpoint test
  - 1 CLI verification test

**Total: 73 unit tests**

## Unit Tests

Unit tests are implemented for components that can be tested without hardware dependencies or extensive mocking.

### Covered by Unit Tests

#### halpi (CLI)
- ✅ CLI command parsing (`halpi/src/main.rs`)
  - All subcommands (status, version, config, shutdown, standby, usb, flash)
  - Argument validation and error cases
  - Short and long form arguments
- ✅ HTTP client construction (`halpi/src/client.rs`)
  - Client initialization with default and custom socket paths
  - Default trait implementation

#### halpi-common (Shared Library)
- ✅ I2C protocol encoding/decoding (`halpi-common/src/protocol.rs`)
  - Analog value scaling
  - Temperature conversion
  - State conversions
  - U32 and word encoding/decoding
- ✅ Type conversions and serialization (`halpi-common/src/types.rs`)
  - Power state conversions
  - Version parsing
  - JSON serialization/deserialization

#### halpid (Daemon)
- ✅ CLI argument parsing (`halpid/src/main.rs`)
  - Configuration file path
  - I2C bus and address configuration
  - Socket path configuration
  - Blackout limits configuration
  - Poweroff command configuration
  - All options combined
- ✅ Health check endpoint (`halpid/src/server/handlers/health.rs`)
  - Root endpoint returns 200 OK

### Requires Integration Tests

The following components require integration tests due to hardware dependencies or need for end-to-end testing:

#### I2C Device Communication (`halpid/src/i2c/`)
- **device.rs**: Requires actual I2C hardware or complex mocking
  - Reading analog values (VIN, VSCAP, IIN, temperature)
  - Reading power state
  - Reading USB port states
  - Feeding watchdog
  - Setting shutdown/standby modes
  - Configuring thresholds
- **dfu.rs**: Requires firmware update protocol testing
  - DFU state machine
  - Firmware upload process
  - Flash operations

#### State Machine (`halpid/src/state_machine/`)
- **machine.rs**: Requires time-based testing and I2C interaction
  - State transitions
  - Blackout detection
  - Watchdog feeding
  - Shutdown coordination

#### HTTP Server Handlers (`halpid/src/server/handlers/`)
- **values.rs**: Requires I2C device for reading measurements
- **config.rs**: Needs configuration state testing
- **usb.rs**: Requires I2C device for USB control
- **shutdown.rs**: Requires system shutdown coordination testing
- **flash.rs**: Requires firmware upload protocol testing

#### Signal Handling (`halpid/src/daemon/`)
- **signals.rs**: Requires process signal testing
  - SIGTERM/SIGINT handling
  - Graceful shutdown
  - Socket cleanup

#### HTTP Client API Calls (`halpi/src/client.rs`)
- All async API methods require running daemon or mock server:
  - get_values()
  - get_config()
  - get_usb_ports()
  - set_usb_port()
  - shutdown()
  - standby_with_delay()
  - standby_at_datetime()

## Integration Tests

Integration tests should be implemented in the `tests/` directory (not yet created) and should:

1. **Mock I2C Device**: Create a mock I2C device that simulates RP2040 firmware behavior
2. **Test HTTP API**: Test all HTTP endpoints end-to-end with the running server
3. **Test State Machine**: Verify state transitions and timing behavior
4. **Test CLI-Daemon Communication**: Verify halpi CLI can communicate with halpid daemon

### Recommended Integration Test Structure

```
tests/
├── i2c_mock.rs          # Mock I2C device for testing
├── server_tests.rs      # HTTP API endpoint tests
├── state_machine_tests.rs  # State machine behavior tests
└── cli_integration.rs   # CLI-daemon integration tests
```

## Running Tests

```bash
# Run all unit tests
cargo test --all

# Run tests for specific package
cargo test --package halpi
cargo test --package halpi-common
cargo test --package halpid

# Run tests with verbose output
cargo test --all --verbose

# Run specific test
cargo test test_cli_verify
```

## Coverage Goals

- **Unit test coverage**: Aim for >70% coverage of testable code
- **Integration test coverage**: All API endpoints and major workflows

## Test Development Guidelines

1. **Unit tests first**: Implement unit tests for all pure functions and logic
2. **Mock sparingly**: Only mock when necessary for integration tests
3. **Test behavior, not implementation**: Focus on public API contracts
4. **Keep tests fast**: Unit tests should run in milliseconds
5. **Use descriptive names**: Test names should clearly describe what is being tested
6. **Test error cases**: Don't just test the happy path

## Integration Testing Strategy

### Current Limitations

The current architecture presents challenges for comprehensive integration testing:

**Tight Coupling to Hardware**:
- `AppState` directly contains `HalpiDevice` (concrete type, not a trait)
- All HTTP handlers depend on `AppState` with real I2C device
- No dependency injection mechanism for mocking I2C operations

**Required Refactoring for Full Integration Tests**:

To enable comprehensive integration testing without hardware, the codebase needs:

1. **Trait-based I2C Device Interface**:
   ```rust
   pub trait I2CDeviceInterface {
       fn get_measurements(&mut self) -> Result<Measurements, I2cError>;
       fn feed_watchdog(&mut self) -> Result<(), I2cError>;
       // ... other methods
   }
   ```

2. **Generic AppState**:
   ```rust
   pub struct AppState<D: I2CDeviceInterface> {
       pub device: Arc<Mutex<D>>,
       pub config: Arc<RwLock<Config>>,
       pub version: &'static str,
   }
   ```

3. **Mock Implementation**:
   ```rust
   pub struct MockI2CDevice {
       // Configurable responses for testing
   }

   impl I2CDeviceInterface for MockI2CDevice {
       // Return predetermined test data
   }
   ```

This refactoring would enable:
- Testing all HTTP endpoints without hardware
- Testing state machine behavior with controlled I2C responses
- Testing error handling paths
- Fast, deterministic integration tests

### Current Integration Test Approach

Until the trait-based refactoring is complete, integration tests use this pattern:

```rust
#[tokio::test]
async fn test_endpoint() {
    // Skip test if I2C hardware not available
    let device = match HalpiDevice::new(1, 0x6D) {
        Ok(d) => Arc::new(Mutex::new(d)),
        Err(_) => return,  // Gracefully skip on CI/dev machines
    };

    // Test with real hardware when available
    // ...
}
```

This allows:
- ✅ Tests run on actual hardware when available
- ✅ Tests gracefully skip in CI/dev environments
- ✅ Basic endpoint validation
- ❌ Cannot test error paths systematically
- ❌ Cannot control I2C device state
- ❌ Cannot test without hardware

### Testable Endpoints Without I2C

Some endpoints can be tested without I2C device mocking:

- ✅ `/` (health check) - No I2C dependency
- ✅ `/version` - Only reads from AppState (needs minimal setup)
- ⚠️ `/config` - Reads/writes config (needs AppState but not I2C operations)

### Recommended Next Steps

1. **Create GitHub issue for trait-based refactoring**
   - Define `I2CDeviceInterface` trait
   - Refactor `AppState` to be generic over device type
   - Update all handlers to use trait bounds
   - Create `MockI2CDevice` implementation

2. **Implement integration tests after refactoring**:
   - HTTP API endpoint tests with mock device
   - State machine tests with controlled I2C responses
   - Error path testing
   - CLI integration tests

3. **Keep hardware tests separate**:
   - Production testing on real hardware remains in `HALPI2-tests/` repository
   - Integration tests focus on software behavior
   - Hardware tests focus on electrical characteristics and protocol compliance

## Notes

- Hardware-dependent code (I2C communication, state machine) cannot be easily unit tested on development machines without I2C hardware
- The daemon requires root privileges for I2C access, which complicates testing
- Cross-platform considerations: Some tests may need `#[cfg(target_os = "linux")]` guards
- **Integration testing blocked on trait-based refactoring** (see Integration Testing Strategy above)
