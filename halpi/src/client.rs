//! HTTP client for communicating with halpid daemon via Unix socket

use anyhow::{Context, Result};
use http_body_util::BodyExt;
use hyper::{Method, Request, StatusCode};
use hyper_util::client::legacy::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use hyperlocal::{UnixClientExt, UnixConnector, Uri};

/// Default Unix socket path for halpid daemon
const DEFAULT_SOCKET_PATH: &str = "/run/halpid/halpid.sock";

/// HTTP client for communicating with halpid daemon
pub struct HalpiClient {
    socket_path: PathBuf,
    #[cfg(unix)]
    client: Client<UnixConnector, String>,
}

impl HalpiClient {
    /// Create a new client with default socket path
    pub fn new() -> Self {
        Self::with_socket_path(DEFAULT_SOCKET_PATH)
    }

    /// Create a new client with custom socket path
    pub fn with_socket_path<P: AsRef<Path>>(path: P) -> Self {
        #[cfg(unix)]
        let client = Client::unix();

        Self {
            socket_path: path.as_ref().to_path_buf(),
            #[cfg(unix)]
            client,
        }
    }

    /// Send a GET request to the specified path
    #[cfg(unix)]
    async fn get(&self, path: &str) -> Result<Value> {
        let url = Uri::new(&self.socket_path, path);
        let response = self
            .client
            .get(url.into())
            .await
            .context("Failed to connect to daemon")?;

        let status = response.status();
        let body_bytes = response
            .into_body()
            .collect()
            .await
            .context("Failed to read response body")?
            .to_bytes();

        if status != StatusCode::OK {
            let error_msg = String::from_utf8_lossy(&body_bytes);
            anyhow::bail!("Request failed ({}): {}", status, error_msg);
        }

        serde_json::from_slice(&body_bytes).context("Failed to parse JSON response")
    }

    /// Send a PUT request with JSON body
    #[cfg(unix)]
    async fn put(&self, path: &str, body: &Value) -> Result<()> {
        let url = Uri::new(&self.socket_path, path);
        let body_str = serde_json::to_string(body)?;

        let req = Request::builder()
            .method(Method::PUT)
            .uri::<hyper::Uri>(url.into())
            .header("Content-Type", "application/json")
            .body(body_str)
            .context("Failed to build request")?;

        let response = self
            .client
            .request(req)
            .await
            .context("Failed to connect to daemon")?;

        let status = response.status();
        if status != StatusCode::NO_CONTENT && status != StatusCode::OK {
            let body_bytes = response
                .into_body()
                .collect()
                .await
                .context("Failed to read error response")?
                .to_bytes();
            let error_msg = String::from_utf8_lossy(&body_bytes);
            anyhow::bail!("Request failed ({}): {}", status, error_msg);
        }

        Ok(())
    }

    /// Send a POST request with JSON body
    #[cfg(unix)]
    async fn post(&self, path: &str, body: &Value) -> Result<()> {
        let url = Uri::new(&self.socket_path, path);
        let body_str = serde_json::to_string(body)?;

        let req = Request::builder()
            .method(Method::POST)
            .uri::<hyper::Uri>(url.into())
            .header("Content-Type", "application/json")
            .body(body_str)
            .context("Failed to build request")?;

        let response = self
            .client
            .request(req)
            .await
            .context("Failed to connect to daemon")?;

        let status = response.status();
        if status != StatusCode::NO_CONTENT && status != StatusCode::OK {
            let body_bytes = response
                .into_body()
                .collect()
                .await
                .context("Failed to read error response")?
                .to_bytes();
            let error_msg = String::from_utf8_lossy(&body_bytes);
            anyhow::bail!("Request failed ({}): {}", status, error_msg);
        }

        Ok(())
    }

    /// Get all sensor values and device information
    pub async fn get_values(&self) -> Result<HashMap<String, Value>> {
        #[cfg(unix)]
        {
            let value = self.get("/values").await?;
            serde_json::from_value(value).context("Failed to parse values response")
        }

        #[cfg(not(unix))]
        anyhow::bail!("Unix sockets not supported on this platform")
    }

    /// Get a specific value by key
    ///
    /// This method is currently unused, but is retained for potential future API expansion
    /// where fetching individual values by key may be required.
    #[allow(dead_code)]
    pub async fn get_value(&self, key: &str) -> Result<Value> {
        #[cfg(unix)]
        {
            self.get(&format!("/values/{}", key)).await
        }

        #[cfg(not(unix))]
        anyhow::bail!("Unix sockets not supported on this platform")
    }

    /// Get daemon configuration
    pub async fn get_config(&self) -> Result<HashMap<String, Value>> {
        #[cfg(unix)]
        {
            let value = self.get("/config").await?;
            serde_json::from_value(value).context("Failed to parse config response")
        }

        #[cfg(not(unix))]
        anyhow::bail!("Unix sockets not supported on this platform")
    }

    /// Set a configuration value
    pub async fn set_config(&self, key: &str, value: Value) -> Result<()> {
        #[cfg(unix)]
        {
            self.put(&format!("/config/{}", key), &value).await
        }

        #[cfg(not(unix))]
        anyhow::bail!("Unix sockets not supported on this platform")
    }

    /// Get USB port states
    pub async fn get_usb_ports(&self) -> Result<HashMap<String, bool>> {
        #[cfg(unix)]
        {
            let value = self.get("/usb").await?;
            serde_json::from_value(value).context("Failed to parse USB port response")
        }

        #[cfg(not(unix))]
        anyhow::bail!("Unix sockets not supported on this platform")
    }

    /// Set USB port state
    pub async fn set_usb_port(&self, port: u8, enabled: bool) -> Result<()> {
        #[cfg(unix)]
        {
            let body = serde_json::json!(enabled);
            self.put(&format!("/usb/{}", port), &body).await
        }

        #[cfg(not(unix))]
        anyhow::bail!("Unix sockets not supported on this platform")
    }

    /// Request system shutdown
    pub async fn shutdown(&self) -> Result<()> {
        #[cfg(unix)]
        {
            self.post("/shutdown", &serde_json::json!({})).await
        }

        #[cfg(not(unix))]
        anyhow::bail!("Unix sockets not supported on this platform")
    }

    /// Request system standby with wakeup time
    pub async fn standby_with_delay(&self, delay_seconds: u32) -> Result<()> {
        #[cfg(unix)]
        {
            let body = serde_json::json!({"delay": delay_seconds});
            self.post("/standby", &body).await
        }

        #[cfg(not(unix))]
        anyhow::bail!("Unix sockets not supported on this platform")
    }

    /// Request system standby with specific datetime
    pub async fn standby_at_datetime(&self, datetime: &str) -> Result<()> {
        #[cfg(unix)]
        {
            let body = serde_json::json!({"datetime": datetime});
            self.post("/standby", &body).await
        }

        #[cfg(not(unix))]
        anyhow::bail!("Unix sockets not supported on this platform")
    }

    /// Upload firmware file to device
    pub async fn upload_firmware(&self, firmware_data: Vec<u8>, filename: &str) -> Result<()> {
        #[cfg(unix)]
        {
            use http_body_util::Full;
            use hyper::body::Bytes;
            use hyper_util::client::legacy::Client;
            use std::time::{SystemTime, UNIX_EPOCH};

            // Generate a unique boundary for multipart form data
            // Use timestamp as a simple alphanumeric boundary
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let boundary = format!("WebKitFormBoundary{}", timestamp);

            // Construct multipart body manually
            let mut body = Vec::new();

            // Add firmware field (boundary in body has -- prefix)
            body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
            body.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"firmware\"; filename=\"{}\"\r\n",
                    filename
                )
                .as_bytes(),
            );
            body.extend_from_slice(b"Content-Type: application/octet-stream\r\n");
            body.extend_from_slice(b"\r\n");
            body.extend_from_slice(&firmware_data);
            body.extend_from_slice(b"\r\n");

            // Add closing boundary (has -- prefix and -- suffix)
            body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

            // Send POST request using a separate client for binary bodies
            let url = Uri::new(&self.socket_path, "/flash");
            let content_type = format!("multipart/form-data; boundary={}", boundary);

            // Create a client that can handle binary bodies
            let binary_client: Client<UnixConnector, Full<Bytes>> = Client::unix();

            let req = Request::builder()
                .method(Method::POST)
                .uri::<hyper::Uri>(url.into())
                .header("Content-Type", content_type)
                .header("Content-Length", body.len())
                .body(Full::new(Bytes::from(body)))
                .context("Failed to build request")?;

            let response = binary_client
                .request(req)
                .await
                .context("Failed to connect to daemon")?;

            let status = response.status();
            if status != StatusCode::NO_CONTENT && status != StatusCode::OK {
                let body_bytes = response
                    .into_body()
                    .collect()
                    .await
                    .context("Failed to read error response")?
                    .to_bytes();
                let error_msg = String::from_utf8_lossy(&body_bytes);
                anyhow::bail!("Firmware upload failed ({}): {}", status, error_msg);
            }

            Ok(())
        }

        #[cfg(not(unix))]
        anyhow::bail!("Unix sockets not supported on this platform")
    }
}

impl Default for HalpiClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_new() {
        let client = HalpiClient::new();
        assert_eq!(client.socket_path.to_str().unwrap(), DEFAULT_SOCKET_PATH);
    }

    #[test]
    fn test_client_with_socket_path() {
        let custom_path = "/tmp/test.sock";
        let client = HalpiClient::with_socket_path(custom_path);
        assert_eq!(client.socket_path.to_str().unwrap(), custom_path);
    }

    #[test]
    fn test_client_default() {
        let client = HalpiClient::default();
        assert_eq!(client.socket_path.to_str().unwrap(), DEFAULT_SOCKET_PATH);
    }

    #[test]
    fn test_default_socket_path_value() {
        assert_eq!(DEFAULT_SOCKET_PATH, "/run/halpid/halpid.sock");
    }
}
