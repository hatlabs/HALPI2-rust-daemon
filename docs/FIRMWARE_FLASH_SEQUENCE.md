# Firmware Flash Sequence - Python Implementation Analysis

This document provides a detailed flowchart of the complete firmware flashing process as implemented in the Python daemon (`halpid` 4.x), showing all I2C operations.

## Flowchart

```
┌─────────────────────────────────────────────────────────────┐
│ START: upload_firmware_with_progress(firmware_data)         │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│ 1. START DFU                                                │
│    I2C WRITE: 0x40 ← [size:4 bytes, big-endian u32]        │
│    start_firmware_update(len(firmware_data))                │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│ 2. Calculate total_blocks = ceil(len / 4096)               │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
        ┌────────────────────────┐
        │ For block_num in       │
        │ range(total_blocks)    │
        └──────┬─────────────────┘
               │
               ▼
    ┌──────────────────────────────────────────────────────┐
    │ 2a. PRE-BLOCK DELAY                                  │
    │     time.sleep(0.1)  # 100ms delay before block     │
    └──────┬───────────────────────────────────────────────┘
           │
           ▼
    ┌──────────────────────────────────────────────────────┐
    │ 2b. WAIT FOR DFU READY                               │
    │     wait_for_dfu_ready(timeout=30.0)                 │
    └──────┬───────────────────────────────────────────────┘
           │
           ▼
    ┌─────────────────────────────────────────────────────────┐
    │ Loop (max 30 seconds):                                  │
    │                                                           │
    │   I2C READ: 0x41 → status_byte (DFU status)             │
    │   get_dfu_status()                                       │
    │                                                           │
    │   ┌─────────────────────────────────────────┐          │
    │   │ status == UPDATING or READY_TO_COMMIT?  │          │
    │   └──┬──YES──────────────────────────NO──┬───┘          │
    │      │                                   │              │
    │      │ Return TRUE              ┌────────▼────────┐    │
    │      │                          │ Check state:    │    │
    │      │                          │                 │    │
    │      │                          │ PREPARING?      │    │
    │      │                          │   → sleep(0.5)  │    │
    │      │                          │   → continue    │    │
    │      │                          │                 │    │
    │      │                          │ QUEUE_FULL?     │    │
    │      │                          │   → sleep(0.1)  │    │
    │      │                          │   → continue    │    │
    │      │                          │                 │    │
    │      │                          │ IDLE?           │    │
    │      │                          │   → ERROR       │    │
    │      │                          │                 │    │
    │      │                          │ CRC/WRITE/      │    │
    │      │                          │ PROTOCOL ERROR? │    │
    │      │                          │   → ERROR       │    │
    │      │                          └─────────────────┘    │
    │      │                                   │              │
    │      │                          time.sleep(0.05)        │
    │      │                          # Poll delay            │
    │      │                                   │              │
    │      │                          Loop back ──────────┘   │
    └──────┼───────────────────────────────────────────────────┘
           │
           ▼
    ┌─────────────────────────────────────────────────────────┐
    │ 2c. UPLOAD BLOCK                                        │
    │     upload_firmware_block(block_num, block_data)        │
    │                                                           │
    │     Build message:                                       │
    │       payload = [block_num:2][block_len:2][data]        │
    │       crc32 = CRC32(payload)                            │
    │       message = [crc32:4] + payload                     │
    │                                                           │
    │     I2C WRITE: 0x43 ← message                           │
    │                                                           │
    │     Note: message can be up to 4104 bytes               │
    │           (4 + 2 + 2 + 4096)                            │
    └──────┬──────────────────────────────────────────────────┘
           │
           ▼
    ┌──────────────────────────────────────────────────────┐
    │ 2d. PROGRESS CALLBACK                                │
    │     progress_callback(block_num + 1, total_blocks)   │
    └──────┬───────────────────────────────────────────────┘
           │
           │ Loop back for next block
           └────────────┐
                        │
        ────────────────┘
               │
               ▼
┌─────────────────────────────────────────────────────────────┐
│ 3. WAIT FOR ALL BLOCKS WRITTEN TO FLASH                    │
│    Loop (max 5 seconds):                                    │
│                                                               │
│    time.sleep(0.1)                                          │
│                                                               │
│    I2C READ: 0x41 → status_byte                             │
│    status = get_dfu_status()                                │
│                                                               │
│    time.sleep(0.1)                                          │
│                                                               │
│    I2C READ: 0x42 → blocks_written (2 bytes, big-endian)   │
│    blocks_written = get_blocks_written()                    │
│                                                               │
│    ┌─────────────────────────────────────────────┐         │
│    │ status == READY_TO_COMMIT &&                │         │
│    │ blocks_written == total_blocks?             │         │
│    └──┬──YES──────────────────────NO──┬───────────┘         │
│       │                                │                    │
│       │ Break loop         ┌───────────▼──────────┐        │
│       │                    │ Check for errors:    │        │
│       │                    │ CRC_ERROR?           │        │
│       │                    │ PROTOCOL_ERROR?      │        │
│       │                    │ WRITE_ERROR?         │        │
│       │                    │   → ABORT & ERROR    │        │
│       │                    └──────────────────────┘        │
│       │                              │                      │
│       │                    time.sleep(0.5)                 │
│       │                    Loop back ────────────┘         │
└───────┼─────────────────────────────────────────────────────┘
        │
        ▼
┌─────────────────────────────────────────────────────────────┐
│ 4. COMMIT FIRMWARE UPDATE                                   │
│    time.sleep(0.1)  # Pre-commit delay                     │
│                                                               │
│    I2C WRITE: 0x44 ← [0x00]                                 │
│    commit_firmware_update()                                 │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│ SUCCESS: Firmware update completed                          │
└─────────────────────────────────────────────────────────────┘


ERROR PATH (from any step above):
┌─────────────────────────────────────────────────────────────┐
│ ABORT FIRMWARE UPDATE                                       │
│    time.sleep(0.1)  # Best effort delay                    │
│    I2C WRITE: 0x45 ← [0x00]                                 │
│    abort_firmware_update()                                  │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│ FAILURE: Return False                                       │
└─────────────────────────────────────────────────────────────┘
```

## I2C Register Map

| Register | Direction | Purpose                    | Data Format                           |
|----------|-----------|----------------------------|---------------------------------------|
| 0x40     | WRITE     | Start DFU                  | 4 bytes: u32 total size (big-endian) |
| 0x41     | READ      | DFU Status                 | 1 byte: DFUState enum                 |
| 0x42     | READ      | Blocks Written Count       | 2 bytes: u16 count (big-endian)       |
| 0x43     | WRITE     | Upload Block               | Variable: [CRC32:4][num:2][len:2][data] |
| 0x44     | WRITE     | Commit DFU                 | 1 byte: 0x00 (value ignored)          |
| 0x45     | WRITE     | Abort DFU                  | 1 byte: 0x00 (value ignored)          |

## DFU State Machine

```
IDLE (0)
  ↓ (after 0x40 write)
PREPARING (1) ← Device initializing flash
  ↓ (ready for blocks)
UPDATING (2) ← Accepting block uploads
  ↓ (queue full, temporary)
QUEUE_FULL (3) → wait 100ms → retry
  ↓ (all blocks uploaded and written)
READY_TO_COMMIT (4) ← All blocks in flash
  ↓ (after 0x44 write)
[Device reboots with new firmware]

Error States (abort required):
- CRC_ERROR (5)
- DATA_LENGTH_ERROR (6)
- WRITE_ERROR (7)
- PROTOCOL_ERROR (8)
```

## Critical Timing Details

### Pre-Block Delay (100ms)
- **Location**: Before each block, after wait_for_dfu_ready()
- **Purpose**: Allow device to respond/process previous state
- **Python line**: 465

### Wait-for-Ready Loop
- **Polls register 0x41** until UPDATING or READY_TO_COMMIT
- **PREPARING state**: Sleep 500ms between polls
- **QUEUE_FULL state**: Sleep 100ms between polls
- **Default polling**: Sleep 50ms between polls
- **Timeout**: 30 seconds
- **Python lines**: 358-397

### Post-Upload Verification Loop
- **Purpose**: Ensure all blocks are written to flash
- **Reads**: 0x41 (status) and 0x42 (blocks_written)
- **Delay**: 100ms before status read, 100ms before blocks_written read
- **Loop delay**: 500ms between iterations
- **Timeout**: 5 seconds
- **Python lines**: 483-511

### Pre-Commit Delay (100ms)
- **Location**: Before sending commit command
- **Purpose**: Ensure device is stable before commit
- **Python line**: 514

## Key Observations

1. **The device needs time to transition from IDLE → PREPARING → UPDATING** after start_dfu() is called. The Python code explicitly waits for this with `wait_for_dfu_ready()`.

2. **QUEUE_FULL is expected and normal** - the device has a limited queue for blocks being written to flash. The Python code handles this with automatic retry after 100ms delay.

3. **Multiple 100ms delays are sprinkled throughout** - these appear to be workarounds for I2C timing issues or device response delays.

4. **Block upload can take time** - up to 4104 bytes per I2C transaction. The device may not be immediately ready for the next block.

5. **Final verification is critical** - the Python code waits for both `READY_TO_COMMIT` status AND `blocks_written == total_blocks` before committing. This ensures all blocks are safely in flash.

## Comparison with Rust Implementation

### What's Missing in Rust:

1. ❌ **No wait-for-ready before first block** - Rust immediately uploads block 0 after start_dfu(), but device may still be in PREPARING state

2. ❌ **No 100ms pre-block delay** - Python has this between wait-for-ready and upload

3. ❌ **No final verification loop** - Rust doesn't wait for blocks to be written to flash before committing

4. ❌ **Status checks after upload, not before** - Rust checks status after writing block, Python waits for ready state first

5. ⚠️ **Handler doesn't use high-level method** - flash.rs handler manually loops instead of using `device.upload_firmware()` which has better error handling

### What Works in Rust:

1. ✅ **QUEUE_FULL retry logic** - `upload_block_with_retry()` handles this
2. ✅ **CRC32 calculation** - Matches Python implementation
3. ✅ **Block format** - Correct structure
4. ✅ **Error state detection** - Checks for error states and aborts

## Recommended Fixes

1. **Add wait_for_dfu_ready() equivalent** that checks status before uploading each block
2. **Add 100ms delay before each block upload**
3. **Add final verification loop** checking blocks_written and status
4. **Use the high-level `upload_firmware()` method** in the handler instead of manual loop
5. **Add 100ms delay before commit**
