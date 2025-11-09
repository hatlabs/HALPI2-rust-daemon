# Migration Guide: Python halpid 4.x → Rust halpid 5.x

This guide helps you migrate from the Python `halpid` 4.x to the Rust `halpid` 5.x daemon.

## Overview

The Rust `halpid` 5.x is a **100% API-compatible** drop-in replacement for Python `halpid` 4.x, offering:
- **5x lower memory footprint** (~10 MB vs ~50 MB)
- **20x faster startup** (<100ms vs ~2s)
- **Single static binary** with zero runtime dependencies
- **Identical configuration** and API

## Compatibility Matrix

| Feature | Python 4.x | Rust 5.x | Notes |
|---------|-----------|----------|-------|
| Configuration file format | YAML | YAML | ✅ Identical |
| Unix socket path | `/run/halpid/halpid.sock` | `/run/halpid/halpid.sock` | ✅ Same |
| HTTP API endpoints | All | All | ✅ Compatible |
| CLI commands | All | All | ✅ Compatible |
| JSON response format | - | - | ✅ Identical |
| I2C protocol | RP2040 firmware | RP2040 firmware | ✅ Same |
| Systemd service | `halpid.service` | `halpid.service` | ✅ Same |
| Package name | `halpid` | `halpid` | ✅ Same |

## Migration Steps

### 1. Backup Current Configuration

```bash
# Backup configuration file
sudo cp /etc/halpid/halpid.conf /etc/halpid/halpid.conf.backup

# Record current daemon status
halpi status > ~/halpi-status-before.txt
halpi config > ~/halpi-config-before.txt
```

### 2. Stop Python Daemon

```bash
# Stop the Python daemon
sudo systemctl stop halpid

# Disable Python daemon (don't remove package yet)
sudo systemctl disable halpid

# Verify it's stopped
sudo systemctl status halpid
```

### 3. Install Rust Daemon

#### Option A: From APT Repository (Recommended)

```bash
# Add HAT Labs APT repository (if not already added)
echo "deb [trusted=yes] https://apt.hatlabs.fi stable main" | \
    sudo tee /etc/apt/sources.list.d/hatlabs.list

# Update package list
sudo apt update

# Install Rust halpid (this will replace Python version)
sudo apt install halpid

# The service should start automatically
sudo systemctl status halpid
```

#### Option B: From Source

```bash
# Clone repository
git clone https://github.com/hatlabs/HALPI2-rust-daemon.git
cd HALPI2-rust-daemon

# Build for ARM64
./run build:cross --release

# Stop Python daemon if running
sudo systemctl stop halpid

# Install binaries
sudo cp target/aarch64-unknown-linux-musl/release/halpid /usr/bin/halpid
sudo cp target/aarch64-unknown-linux-musl/release/halpi /usr/bin/halpi

# Install systemd service
sudo cp systemd/halpid.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable halpid
sudo systemctl start halpid
```

### 4. Verify Configuration

The configuration file format is identical, but verify it's correct:

```bash
# Check configuration file exists
ls -l /etc/halpid/halpid.conf

# Verify daemon can read it
sudo halpid --conf /etc/halpid/halpid.conf --help

# Check daemon status
sudo systemctl status halpid
```

### 5. Test Functionality

```bash
# Test CLI connectivity
halpi version

# Check system status
halpi status

# Verify configuration is read correctly
halpi config

# Compare with backup
diff <(cat ~/halpi-status-before.txt | sort) <(halpi status | sort)
```

### 6. Monitor for Issues

```bash
# Watch daemon logs for any errors
sudo journalctl -u halpid -f

# Check daemon is feeding watchdog
halpi status | grep -i watchdog

# Verify USB control works
halpi usb
```

### 7. Remove Python Version (Optional)

Once you've verified everything works:

```bash
# Remove Python halpid package
sudo apt remove python3-halpid

# Or if installed via pip
pip3 uninstall halpid
```

## Configuration Differences

The configuration file format is **identical**, but here are the supported fields for reference:

### Python 4.x

```yaml
i2c_bus: 1
i2c_addr: 0x6D
blackout_time_limit: 10.0
blackout_voltage_limit: 9.0
socket: /run/halpid/halpid.sock
socket_group: adm
poweroff: /sbin/poweroff
```

### Rust 5.x

```yaml
i2c_bus: 1
i2c_addr: 0x6D
blackout_time_limit: 10.0
blackout_voltage_limit: 9.0
socket: /run/halpid/halpid.sock
socket_group: adm
poweroff: /sbin/poweroff
```

✅ **Identical** - No changes needed!

## API Compatibility

All HTTP API endpoints are 100% compatible:

| Endpoint | Python 4.x | Rust 5.x | Compatible |
|----------|-----------|----------|------------|
| `GET /` | ✅ | ✅ | ✅ |
| `GET /version` | ✅ | ✅ | ✅ |
| `GET /values` | ✅ | ✅ | ✅ |
| `GET /values/:key` | ✅ | ✅ | ✅ |
| `GET /config` | ✅ | ✅ | ✅ |
| `GET /config/:key` | ✅ | ✅ | ✅ |
| `PUT /config/:key` | ✅ | ✅ | ✅ |
| `GET /usb` | ✅ | ✅ | ✅ |
| `GET /usb/:port` | ✅ | ✅ | ✅ |
| `PUT /usb/:port` | ✅ | ✅ | ✅ |
| `POST /shutdown` | ✅ | ✅ | ✅ |
| `POST /standby` | ✅ | ✅ | ✅ |
| `POST /flash` | ✅ | ✅ | ✅ |

### Example: API Compatibility Test

```bash
# This script works identically with both versions
SOCKET=/run/halpid/halpid.sock

# Get version
curl --unix-socket $SOCKET http://localhost/version | jq

# Get all values
curl --unix-socket $SOCKET http://localhost/values | jq

# Get config
curl --unix-socket $SOCKET http://localhost/config | jq

# Get USB ports
curl --unix-socket $SOCKET http://localhost/usb | jq
```

## CLI Compatibility

All CLI commands are identical:

```bash
# Python 4.x          →  Rust 5.x
halpi status            halpi status         # ✅ Same
halpi version           halpi version        # ✅ Same
halpi config            halpi config         # ✅ Same
halpi config KEY        halpi config KEY     # ✅ Same
halpi usb               halpi usb            # ✅ Same
halpi usb enable 0      halpi usb enable 0   # ✅ Same
halpi usb disable all   halpi usb disable all# ✅ Same
halpi shutdown          halpi shutdown       # ✅ Same
halpi shutdown --standby --time 300          # ✅ Same
halpi flash firmware.bin halpi flash firmware.bin # ✅ Same
```

## Performance Improvements

After migration, you should see:

### Memory Usage

```bash
# Before (Python):
ps aux | grep halpid
# halpid   1234  ... 50.2 MB ...

# After (Rust):
ps aux | grep halpid
# halpid   1234  ...  8.4 MB ...
```

**Result**: ~5x reduction in memory footprint

### Startup Time

```bash
# Before (Python):
time sudo systemctl restart halpid
# real: 2.1s

# After (Rust):
time sudo systemctl restart halpid
# real: 0.08s
```

**Result**: ~20x faster startup

### Binary Size

```bash
# Before (Python):
# Requires Python runtime + dependencies (~100+ MB)

# After (Rust):
ls -lh /usr/bin/halpid
# -rwxr-xr-x 1 root root 2.8M halpid
```

**Result**: Single 2.8 MB static binary, no dependencies

## Behavioral Differences

While the API is 100% compatible, there are subtle implementation differences:

### 1. State Machine Polling

**Both versions poll at 0.1 second intervals** - No change.

### 2. Logging

**Python 4.x**:
- Uses Python `logging` module
- Logs to systemd journal via Python logging handler

**Rust 5.x**:
- Uses `tracing` crate
- Logs directly to systemd journal
- More structured logging with spans

**Log format is similar but not identical**. Set log level via environment:

```bash
# Python 4.x
HALPI_LOG_LEVEL=DEBUG halpid

# Rust 5.x
RUST_LOG=debug halpid
```

### 3. Error Messages

Error messages are similar but may have different wording. The HTTP status codes and error semantics are identical.

### 4. Startup Behavior

**Rust version starts significantly faster** (<100ms vs ~2s), which may affect:
- Systemd dependency ordering
- Initialization scripts
- Boot-time race conditions

If you have scripts that assume halpid takes time to start, they may need adjustment.

## Troubleshooting Migration Issues

### Daemon Won't Start

```bash
# Check systemd status
sudo systemctl status halpid

# Check detailed logs
sudo journalctl -u halpid -n 100 --no-pager

# Verify binary is correct version
halpid --version  # Should show "halpid 5.x.x"

# Check I2C permissions
ls -l /dev/i2c-1
sudo usermod -a -G i2c root  # Ensure root has I2C access
```

### CLI Can't Connect

```bash
# Verify daemon is running
sudo systemctl status halpid

# Check socket exists
ls -l /run/halpid/halpid.sock

# Verify socket permissions
# Socket should be owned by root:adm with mode 0660

# Add user to adm group
sudo usermod -a -G adm $USER
# Log out and back in for group membership to take effect
```

### Different Behavior

```bash
# Compare configurations
diff /etc/halpid/halpid.conf.backup /etc/halpid/halpid.conf

# Check environment variables
sudo systemctl show halpid | grep Environment

# Verify firmware version is compatible
halpi status | grep firmware_version
# Firmware should be 2.1.0 or later
```

### Rollback to Python Version

If you need to rollback:

```bash
# Stop Rust daemon
sudo systemctl stop halpid
sudo systemctl disable halpid

# Reinstall Python version
sudo apt install python3-halpid

# Restore backup configuration
sudo cp /etc/halpid/halpid.conf.backup /etc/halpid/halpid.conf

# Enable and start Python daemon
sudo systemctl enable halpid
sudo systemctl start halpid

# Verify
halpi status
```

## Testing Migration

Before migrating production systems, test on a development HALPI2:

1. **Setup test environment**
   - Clone production configuration
   - Deploy Rust daemon
   - Run for 24-48 hours

2. **Monitor metrics**
   ```bash
   # Check memory usage over time
   while true; do
       ps aux | grep halpid | grep -v grep >> memory.log
       sleep 60
   done

   # Check for any errors
   sudo journalctl -u halpid | grep -i error
   ```

3. **Test all features**
   - CLI commands
   - HTTP API calls
   - USB port control
   - Shutdown/standby
   - Firmware upload
   - Watchdog feeding
   - Blackout detection

4. **Validate API compatibility**
   ```bash
   # Use your existing monitoring/automation scripts
   # They should work without modification
   ```

## Migration Checklist

- [ ] Backup current configuration
- [ ] Record current system state (halpi status, config)
- [ ] Stop Python daemon
- [ ] Install Rust daemon (APT or source)
- [ ] Verify configuration is loaded correctly
- [ ] Test CLI commands
- [ ] Test HTTP API endpoints
- [ ] Monitor logs for 24 hours
- [ ] Test blackout behavior (if safe to do so)
- [ ] Test USB control
- [ ] Test firmware upload (if needed)
- [ ] Remove Python package (optional)
- [ ] Update monitoring scripts (if log format changed)
- [ ] Document any behavioral differences observed

## Getting Help

If you encounter issues during migration:

- **GitHub Issues**: https://github.com/hatlabs/HALPI2-rust-daemon/issues
- **Documentation**: https://docs.hatlabs.fi/halpi2
- **HAT Labs Support**: support@hatlabs.fi

## Version History

- **5.0.0** (2025): Initial Rust release, 100% API compatible with Python 4.x
- **4.x** (2024): Python implementation (reference version)

## Recommended Migration Timeline

- **Week 1**: Test in development environment
- **Week 2**: Deploy to 1-2 production systems
- **Week 3**: Monitor and validate
- **Week 4+**: Roll out to remaining systems

The gradual approach ensures any unexpected issues are caught early.
