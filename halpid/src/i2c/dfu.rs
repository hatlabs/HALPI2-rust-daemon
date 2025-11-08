//! Firmware update (DFU - Device Firmware Update) implementation
//!
//! This module provides high-level firmware update functionality for the HALPI2 controller.
//! The DFU protocol uses a block-based upload with CRC32 verification.
//!
//! # Protocol Overview
//!
//! 1. Start DFU with total firmware size
//! 2. Upload firmware in blocks (up to 4096 bytes each)
//! 3. Check DFU status (handle QUEUE_FULL with retry)
//! 4. Commit firmware update when all blocks are uploaded
//! 5. Or abort on any error state
//!
//! # Block Format
//!
//! Each block consists of:
//! - CRC32 (4 bytes, big-endian) - calculated over block_num + block_len + data
//! - Block number (2 bytes, big-endian)
//! - Block length (2 bytes, big-endian)
//! - Data (up to 4096 bytes)

use super::device::{HalpiDevice, I2cError};
use halpi_common::protocol::{self, DFUState};
use std::thread;
use std::time::Duration;

/// Maximum block size for firmware upload (flash sector size)
pub const FLASH_BLOCK_SIZE: usize = 4096;

/// Maximum number of retries when firmware queue is full
const QUEUE_FULL_MAX_RETRIES: usize = 10;

/// Delay between retries when queue is full
const QUEUE_FULL_RETRY_DELAY: Duration = Duration::from_millis(100);

impl HalpiDevice {
    /// Start a firmware update process
    ///
    /// This initializes the DFU state machine on the controller with the total
    /// size of the firmware to be uploaded.
    ///
    /// # Arguments
    /// * `total_size` - Total size of the firmware in bytes
    ///
    /// # Errors
    /// Returns `I2cError` if the command cannot be sent to the device.
    pub fn start_dfu(&mut self, total_size: u32) -> Result<(), I2cError> {
        let bytes = protocol::encode_u32(total_size);
        self.write_bytes(protocol::REG_DFU_START, &bytes)
    }

    /// Upload a single firmware block
    ///
    /// This uploads a block of firmware data with CRC32 verification.
    /// The block format is: [CRC32:4][block_num:2][block_len:2][data]
    ///
    /// # Arguments
    /// * `block_num` - Block number (0-based index)
    /// * `data` - Block data (up to 4096 bytes)
    ///
    /// # Errors
    /// Returns `I2cError` if:
    /// - The block size exceeds 4096 bytes
    /// - The block cannot be written to the device
    pub fn upload_block(&mut self, block_num: u16, data: &[u8]) -> Result<(), I2cError> {
        if data.len() > FLASH_BLOCK_SIZE {
            return Err(I2cError::InvalidBlockSize {
                size: data.len(),
                max_size: FLASH_BLOCK_SIZE,
            });
        }

        let block_len = data.len() as u16;

        // Build the payload: block_num + block_len + data
        let mut payload = Vec::with_capacity(2 + 2 + data.len());
        payload.extend_from_slice(&protocol::encode_word(block_num));
        payload.extend_from_slice(&protocol::encode_word(block_len));
        payload.extend_from_slice(data);

        // Calculate CRC32 over the payload
        let crc32 = crc32fast::hash(&payload);

        // Build the full message: CRC32 + payload
        let mut message = Vec::with_capacity(4 + payload.len());
        message.extend_from_slice(&protocol::encode_u32(crc32));
        message.extend_from_slice(&payload);

        // Write to device
        self.write_bytes(protocol::REG_DFU_UPLOAD_BLOCK, &message)
    }

    /// Get the current DFU status
    ///
    /// # Errors
    /// Returns `I2cError` if the status cannot be read from the device.
    pub fn get_dfu_status(&mut self) -> Result<DFUState, I2cError> {
        let status_byte = self.read_byte(protocol::REG_DFU_STATUS)?;
        DFUState::from_byte(status_byte).map_err(|_| I2cError::InvalidDfuState {
            state: status_byte,
        })
    }

    /// Get the number of blocks written to flash
    ///
    /// This can be used to track upload progress.
    ///
    /// # Errors
    /// Returns `I2cError` if the block count cannot be read from the device.
    pub fn get_blocks_written(&mut self) -> Result<u16, I2cError> {
        self.read_word(protocol::REG_DFU_BLOCKS_WRITTEN)
    }

    /// Commit the firmware update
    ///
    /// This finalizes the firmware update process. The controller will verify
    /// the firmware and mark it as ready to boot.
    ///
    /// # Errors
    /// Returns `I2cError` if the commit command cannot be sent to the device.
    pub fn commit_dfu(&mut self) -> Result<(), I2cError> {
        self.write_byte(protocol::REG_DFU_COMMIT, 0x00)
    }

    /// Abort the firmware update
    ///
    /// This cancels the firmware update process and returns the controller
    /// to the idle state.
    ///
    /// # Errors
    /// Returns `I2cError` if the abort command cannot be sent to the device.
    pub fn abort_dfu(&mut self) -> Result<(), I2cError> {
        self.write_byte(protocol::REG_DFU_ABORT, 0x00)
    }

    /// Upload entire firmware with progress callback
    ///
    /// This is a high-level method that handles the complete firmware update process:
    /// 1. Starts DFU with total size
    /// 2. Splits firmware into blocks and uploads each one
    /// 3. Handles QUEUE_FULL state with automatic retry
    /// 4. Calls progress callback after each block
    /// 5. Detects and aborts on error states
    /// 6. Commits the update when complete
    ///
    /// # Arguments
    /// * `firmware` - Complete firmware data
    /// * `progress` - Callback function called with (blocks_written, total_blocks) after each block
    ///
    /// # Errors
    /// Returns `I2cError` if:
    /// - Any I2C operation fails
    /// - The DFU state machine enters an error state
    /// - QUEUE_FULL retry limit is exceeded
    ///
    /// # Example
    /// ```ignore
    /// use halpid::i2c::HalpiDevice;
    ///
    /// let mut device = HalpiDevice::new(1, 0x6D)?;
    /// let firmware = std::fs::read("firmware.bin")?;
    ///
    /// device.upload_firmware(&firmware, |written, total| {
    ///     println!("Progress: {}/{} blocks", written, total);
    /// })?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn upload_firmware(
        &mut self,
        firmware: &[u8],
        mut progress: impl FnMut(usize, usize),
    ) -> Result<(), I2cError> {
        // Start DFU
        self.start_dfu(firmware.len() as u32)?;

        // Calculate total blocks
        let total_blocks = firmware.len().div_ceil(FLASH_BLOCK_SIZE);

        // Upload each block
        for (block_num, chunk) in firmware.chunks(FLASH_BLOCK_SIZE).enumerate() {
            // Upload block with retry on QUEUE_FULL
            self.upload_block_with_retry(block_num as u16, chunk)?;

            // Check for error states
            let status = self.get_dfu_status()?;
            if matches!(
                status,
                DFUState::CrcError
                    | DFUState::DataLengthError
                    | DFUState::WriteError
                    | DFUState::ProtocolError
            ) {
                // Abort on error
                let _ = self.abort_dfu();
                return Err(I2cError::DfuError { state: status });
            }

            // Report progress
            progress(block_num + 1, total_blocks);
        }

        // Verify final state
        let status = self.get_dfu_status()?;
        if status != DFUState::ReadyToCommit {
            let _ = self.abort_dfu();
            return Err(I2cError::DfuUnexpectedState {
                expected: DFUState::ReadyToCommit,
                actual: status,
            });
        }

        // Commit the update
        self.commit_dfu()?;

        Ok(())
    }

    /// Upload a block with automatic retry on QUEUE_FULL
    fn upload_block_with_retry(&mut self, block_num: u16, data: &[u8]) -> Result<(), I2cError> {
        for attempt in 0..QUEUE_FULL_MAX_RETRIES {
            // Upload the block
            self.upload_block(block_num, data)?;

            // Check status
            let status = self.get_dfu_status()?;

            match status {
                DFUState::QueueFull => {
                    // Queue is full, wait and retry
                    if attempt < QUEUE_FULL_MAX_RETRIES - 1 {
                        thread::sleep(QUEUE_FULL_RETRY_DELAY);
                        continue;
                    } else {
                        // Max retries exceeded
                        return Err(I2cError::DfuQueueFullTimeout);
                    }
                }
                DFUState::Updating => {
                    // Block accepted
                    return Ok(());
                }
                _ => {
                    // Unexpected state
                    return Err(I2cError::DfuUnexpectedState {
                        expected: DFUState::Updating,
                        actual: status,
                    });
                }
            }
        }

        // Should never reach here
        Err(I2cError::DfuQueueFullTimeout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flash_block_size_constant() {
        assert_eq!(FLASH_BLOCK_SIZE, 4096);
    }

    #[test]
    fn test_crc32_calculation() {
        // Test CRC32 calculation matches expected format
        let block_num: u16 = 0;
        let block_len: u16 = 8;
        let data = b"testdata";

        let mut payload = Vec::new();
        payload.extend_from_slice(&protocol::encode_word(block_num));
        payload.extend_from_slice(&protocol::encode_word(block_len));
        payload.extend_from_slice(data);

        let crc32 = crc32fast::hash(&payload);

        // Verify CRC32 is calculated over the correct data
        assert_eq!(payload.len(), 4 + data.len());

        // Verify the CRC32 encoding produces 4 bytes
        let crc_bytes = protocol::encode_u32(crc32);
        assert_eq!(crc_bytes.len(), 4);
    }
}
