// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::common::{CheckDetail, CheckResult, ComponentChecker};
use crate::config::DatanodeConfig;
use async_trait::async_trait;
use opendal::services::S3;
use opendal::Operator;
use std::fmt::{Debug, Formatter};
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::time::timeout;
use uuid::Uuid;

/// Datanode component checker
pub struct DatanodeChecker {
    config: DatanodeConfig,
    include_performance: bool,
}

impl Debug for DatanodeChecker {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "DatanodeChecker")
    }
}

impl DatanodeChecker {
    /// Create a new DatanodeChecker with the given configuration
    pub fn new(config: DatanodeConfig, include_performance: bool) -> Self {
        Self { config, include_performance }
    }

    /// Check connectivity to metasrv endpoints (reuse logic from frontend)
    async fn check_metasrv_connectivity(&self) -> CheckResult {
        let mut details = Vec::new();

        if self.config.metasrv_addrs.is_empty() {
            details.push(CheckDetail::fail(
                "Metasrv Configuration".to_string(),
                "No metasrv addresses configured".to_string(),
                None,
                Some("Add metasrv addresses to metasrv_addrs configuration".to_string()),
            ));
            return CheckResult::from_details(details);
        }

        for (index, addr) in self.config.metasrv_addrs.iter().enumerate() {
            let start = Instant::now();

            // Parse address to extract host and port
            let (host, port) = match self.parse_address(addr) {
                Ok((h, p)) => (h, p),
                Err(e) => {
                    details.push(CheckDetail::fail(
                        format!("Metasrv Address {} Parsing", index + 1),
                        format!("Failed to parse address '{}': {}", addr, e),
                        None,
                        Some("Check address format (should be host:port)".to_string()),
                    ));
                    continue;
                }
            };

            // Test TCP connectivity
            match timeout(Duration::from_secs(10), TcpStream::connect((host.as_str(), port))).await {
                Ok(Ok(_stream)) => {
                    details.push(CheckDetail::pass(
                        format!("Metasrv Connectivity {}", index + 1),
                        format!("Successfully connected to metasrv at {}", addr),
                        Some(start.elapsed()),
                    ));
                }
                Ok(Err(e)) => {
                    details.push(CheckDetail::fail(
                        format!("Metasrv Connectivity {}", index + 1),
                        format!("Failed to connect to metasrv at {}: {}", addr, e),
                        Some(start.elapsed()),
                        Some("Check if metasrv is running and accessible".to_string()),
                    ));
                }
                Err(_) => {
                    details.push(CheckDetail::fail(
                        format!("Metasrv Connectivity {}", index + 1),
                        format!("Connection to metasrv at {} timed out", addr),
                        Some(start.elapsed()),
                        Some("Check network connectivity and metasrv availability".to_string()),
                    ));
                }
            }
        }

        CheckResult::from_details(details)
    }

    /// Check object storage configuration and connectivity
    async fn check_object_storage(&self) -> CheckResult {
        match self.config.storage.storage_type.as_str() {
            "S3" => self.check_s3_storage().await,
            "Oss" => self.check_oss_storage().await,
            "Azblob" => self.check_azblob_storage().await,
            "Gcs" => self.check_gcs_storage().await,
            "File" => self.check_file_storage().await,
            unknown => CheckResult::failure(
                format!("Unknown storage type: {}", unknown),
                vec![CheckDetail::fail(
                    "Storage Type".to_string(),
                    format!("Unsupported storage type: {}", unknown),
                    None,
                    Some("Use one of: S3, Oss, Azblob, Gcs, File".to_string()),
                )],
            ),
        }
    }

    /// Check S3-compatible storage
    async fn check_s3_storage(&self) -> CheckResult {
        let mut details = Vec::new();
        let start = Instant::now();

        // Parse S3 configuration
        let s3_config = match self.config.storage.as_s3_config() {
            Ok(config) => config,
            Err(e) => {
                details.push(CheckDetail::fail(
                    "S3 Configuration".to_string(),
                    format!("Failed to parse S3 configuration: {}", e),
                    None,
                    Some("Check S3 configuration parameters".to_string()),
                ));
                return CheckResult::from_details(details);
            }
        };

        // Build S3 operator
        let mut builder = S3::default()
            .root(s3_config.root.as_deref().unwrap_or(""))
            .bucket(&s3_config.bucket)
            .access_key_id(&s3_config.access_key_id)
            .secret_access_key(&s3_config.secret_access_key);

        if let Some(endpoint) = &s3_config.endpoint {
            builder = builder.endpoint(endpoint);
        }
        if let Some(region) = &s3_config.region {
            builder = builder.region(region);
        }

        match Operator::new(builder) {
            Ok(op) => {
                let op = op.finish();
                details.push(CheckDetail::pass(
                    "S3 Client Creation".to_string(),
                    "S3 client created successfully".to_string(),
                    Some(start.elapsed()),
                ));

                // Test basic operations
                let test_key = format!("stepstone-test/{}", Uuid::new_v4());
                let test_data = b"stepstone-test-data";

                // PUT test
                match op.write(&test_key, test_data.as_slice()).await {
                    Ok(_) => {
                        details.push(CheckDetail::pass(
                            "S3 PUT Operation".to_string(),
                            "PUT operation successful".to_string(),
                            None,
                        ));

                        // GET test
                        match op.read(&test_key).await {
                            Ok(data) => {
                                if data.to_vec() == test_data {
                                    details.push(CheckDetail::pass(
                                        "S3 GET Operation".to_string(),
                                        "GET operation successful and data matches".to_string(),
                                        None,
                                    ));
                                } else {
                                    details.push(CheckDetail::fail(
                                        "S3 GET Operation".to_string(),
                                        "GET operation returned incorrect data".to_string(),
                                        None,
                                        Some("Check S3 data consistency".to_string()),
                                    ));
                                }
                            }
                            Err(e) => {
                                details.push(CheckDetail::fail(
                                    "S3 GET Operation".to_string(),
                                    format!("GET operation failed: {}", e),
                                    None,
                                    Some("Check S3 read permissions".to_string()),
                                ));
                            }
                        }

                        // DELETE test (cleanup)
                        match op.delete(&test_key).await {
                            Ok(_) => {
                                details.push(CheckDetail::pass(
                                    "S3 DELETE Operation".to_string(),
                                    "DELETE operation successful".to_string(),
                                    None,
                                ));
                            }
                            Err(e) => {
                                details.push(CheckDetail::warning(
                                    "S3 DELETE Operation".to_string(),
                                    format!("DELETE operation failed: {}", e),
                                    None,
                                    Some("Test object may remain in S3, but this doesn't affect functionality".to_string()),
                                ));
                            }
                        }

                        // Performance test if requested
                        if self.include_performance {
                            let perf_result = self.performance_test_s3(&op).await;
                            details.extend(perf_result.details);
                        }
                    }
                    Err(e) => {
                        details.push(CheckDetail::fail(
                            "S3 PUT Operation".to_string(),
                            format!("PUT operation failed: {}", e),
                            None,
                            Some("Check S3 credentials, bucket permissions, and network connectivity".to_string()),
                        ));
                    }
                }
            }
            Err(e) => {
                details.push(CheckDetail::fail(
                    "S3 Client Creation".to_string(),
                    format!("Failed to create S3 client: {}", e),
                    Some(start.elapsed()),
                    Some("Check S3 configuration and credentials".to_string()),
                ));
            }
        }

        CheckResult::from_details(details)
    }

    /// Check OSS storage
    async fn check_oss_storage(&self) -> CheckResult {
        let mut details = Vec::new();

        details.push(CheckDetail::warning(
            "OSS Storage".to_string(),
            "OSS storage check not fully implemented yet".to_string(),
            None,
            Some("OSS support is planned for future versions".to_string()),
        ));

        CheckResult::from_details(details)
    }

    /// Check Azure Blob storage
    async fn check_azblob_storage(&self) -> CheckResult {
        let mut details = Vec::new();

        details.push(CheckDetail::warning(
            "Azure Blob Storage".to_string(),
            "Azure Blob storage check not fully implemented yet".to_string(),
            None,
            Some("Azure Blob support is planned for future versions".to_string()),
        ));

        CheckResult::from_details(details)
    }

    /// Check Google Cloud Storage
    async fn check_gcs_storage(&self) -> CheckResult {
        let mut details = Vec::new();

        details.push(CheckDetail::warning(
            "Google Cloud Storage".to_string(),
            "GCS storage check not fully implemented yet".to_string(),
            None,
            Some("GCS support is planned for future versions".to_string()),
        ));

        CheckResult::from_details(details)
    }

    /// Check file storage
    async fn check_file_storage(&self) -> CheckResult {
        let mut details = Vec::new();

        // For file storage, we mainly check if the directory exists and is writable
        let root_path = self.config.storage.config.get("root")
            .and_then(|v| v.as_str())
            .unwrap_or("./data");

        match std::fs::metadata(root_path) {
            Ok(metadata) => {
                if metadata.is_dir() {
                    details.push(CheckDetail::pass(
                        "File Storage Directory".to_string(),
                        format!("Storage directory '{}' exists", root_path),
                        None,
                    ));

                    // Test write permissions
                    let test_file = format!("{}/stepstone_test_{}", root_path, Uuid::new_v4());
                    match std::fs::write(&test_file, b"test") {
                        Ok(_) => {
                            details.push(CheckDetail::pass(
                                "File Storage Write Permission".to_string(),
                                "Write permission verified".to_string(),
                                None,
                            ));

                            // Cleanup
                            let _ = std::fs::remove_file(&test_file);
                        }
                        Err(e) => {
                            details.push(CheckDetail::fail(
                                "File Storage Write Permission".to_string(),
                                format!("Write permission test failed: {}", e),
                                None,
                                Some("Check directory permissions".to_string()),
                            ));
                        }
                    }
                } else {
                    details.push(CheckDetail::fail(
                        "File Storage Directory".to_string(),
                        format!("Storage path '{}' exists but is not a directory", root_path),
                        None,
                        Some("Ensure storage path points to a directory".to_string()),
                    ));
                }
            }
            Err(e) => {
                details.push(CheckDetail::fail(
                    "File Storage Directory".to_string(),
                    format!("Storage directory '{}' does not exist or is not accessible: {}", root_path, e),
                    None,
                    Some("Create the storage directory or check permissions".to_string()),
                ));
            }
        }

        CheckResult::from_details(details)
    }

    /// Perform S3 performance test
    async fn performance_test_s3(&self, op: &Operator) -> CheckResult {
        let mut details = Vec::new();

        // Test different data sizes
        let test_sizes = vec![
            (1024, "1KB"),
            (1024 * 1024, "1MB"),
            (10 * 1024 * 1024, "10MB"),
        ];

        for (size, size_name) in test_sizes {
            let test_key = format!("stepstone-perf-test/{}/{}", size_name, Uuid::new_v4());
            let test_data = vec![0u8; size];

            // Write latency test
            let start = Instant::now();
            match op.write(&test_key, test_data.clone()).await {
                Ok(_) => {
                    let write_latency = start.elapsed();
                    let write_throughput = (size as f64) / write_latency.as_secs_f64() / (1024.0 * 1024.0); // MB/s

                    details.push(CheckDetail::pass(
                        format!("S3 Write Latency ({})", size_name),
                        format!("Write latency: {:?} ({:.2} MB/s)", write_latency, write_throughput),
                        Some(write_latency),
                    ));

                    // Read latency test
                    let start = Instant::now();
                    match op.read(&test_key).await {
                        Ok(read_data) => {
                            let read_latency = start.elapsed();
                            let read_throughput = (read_data.len() as f64) / read_latency.as_secs_f64() / (1024.0 * 1024.0); // MB/s

                            if read_data.len() == size {
                                details.push(CheckDetail::pass(
                                    format!("S3 Read Latency ({})", size_name),
                                    format!("Read latency: {:?} ({:.2} MB/s)", read_latency, read_throughput),
                                    Some(read_latency),
                                ));
                            } else {
                                details.push(CheckDetail::fail(
                                    format!("S3 Read Verification ({})", size_name),
                                    format!("Data size mismatch: expected {}, got {}", size, read_data.len()),
                                    Some(read_latency),
                                    Some("Check S3 data integrity".to_string()),
                                ));
                            }
                        }
                        Err(e) => {
                            details.push(CheckDetail::fail(
                                format!("S3 Read Test ({})", size_name),
                                format!("Read failed: {}", e),
                                None,
                                Some("Check S3 read permissions and connectivity".to_string()),
                            ));
                        }
                    }

                    // Cleanup
                    let _ = op.delete(&test_key).await;
                }
                Err(e) => {
                    details.push(CheckDetail::fail(
                        format!("S3 Write Test ({})", size_name),
                        format!("Write failed: {}", e),
                        None,
                        Some("Check S3 write permissions and connectivity".to_string()),
                    ));
                }
            }
        }

        // Concurrent operations test
        let concurrent_result = self.performance_test_concurrent_s3(op).await;
        details.extend(concurrent_result.details);

        CheckResult::from_details(details)
    }

    /// Test concurrent S3 operations
    async fn performance_test_concurrent_s3(&self, op: &Operator) -> CheckResult {
        let mut details = Vec::new();

        let concurrent_count = 10;
        let test_data = vec![0u8; 1024]; // 1KB per operation

        let start = Instant::now();
        let mut handles = Vec::new();

        for i in 0..concurrent_count {
            let test_key = format!("stepstone-concurrent-test/{}", i);
            let test_key_clone = test_key.clone();
            let op_clone = op.clone();
            let data_clone = test_data.clone();

            let handle = tokio::spawn(async move {
                op_clone.write(&test_key_clone, data_clone).await
            });
            handles.push((handle, test_key));
        }

        let mut successful_writes = 0;
        let mut test_keys = Vec::new();

        for (handle, key) in handles {
            match handle.await {
                Ok(Ok(_)) => {
                    successful_writes += 1;
                    test_keys.push(key);
                }
                Ok(Err(_)) | Err(_) => {}
            }
        }

        let concurrent_write_duration = start.elapsed();

        if successful_writes == concurrent_count {
            let throughput = (concurrent_count as f64 * test_data.len() as f64) / concurrent_write_duration.as_secs_f64() / (1024.0 * 1024.0);
            details.push(CheckDetail::pass(
                "S3 Concurrent Write".to_string(),
                format!("Successfully wrote {} objects concurrently in {:?} ({:.2} MB/s)",
                    concurrent_count, concurrent_write_duration, throughput),
                Some(concurrent_write_duration),
            ));
        } else {
            details.push(CheckDetail::warning(
                "S3 Concurrent Write".to_string(),
                format!("Only {}/{} concurrent writes succeeded", successful_writes, concurrent_count),
                Some(concurrent_write_duration),
                Some("Check S3 rate limits and connection pool settings".to_string()),
            ));
        }

        // Cleanup concurrent test objects
        for key in test_keys {
            let _ = op.delete(&key).await;
        }

        CheckResult::from_details(details)
    }

    /// Parse address string into host and port (reuse from frontend)
    fn parse_address(&self, addr: &str) -> Result<(String, u16), String> {
        // Handle different address formats
        if addr.starts_with("http://") {
            let addr = addr.strip_prefix("http://").unwrap();
            self.parse_host_port(addr)
        } else if addr.starts_with("https://") {
            let addr = addr.strip_prefix("https://").unwrap();
            self.parse_host_port(addr)
        } else {
            self.parse_host_port(addr)
        }
    }

    /// Parse host:port format
    fn parse_host_port(&self, addr: &str) -> Result<(String, u16), String> {
        if let Some(colon_pos) = addr.rfind(':') {
            let host = addr[..colon_pos].to_string();
            let port_str = &addr[colon_pos + 1..];

            // Remove any path component
            let port_str = if let Some(slash_pos) = port_str.find('/') {
                &port_str[..slash_pos]
            } else {
                port_str
            };

            match port_str.parse::<u16>() {
                Ok(port) => Ok((host, port)),
                Err(_) => Err(format!("Invalid port number: {}", port_str)),
            }
        } else {
            Err("Address must contain port number (host:port)".to_string())
        }
    }
}

#[async_trait]
impl ComponentChecker for DatanodeChecker {
    async fn check(&self) -> CheckResult {
        let mut all_details = Vec::new();

        // Check metasrv connectivity
        let metasrv_result = self.check_metasrv_connectivity().await;
        all_details.extend(metasrv_result.details);

        // Check object storage
        let storage_result = self.check_object_storage().await;
        all_details.extend(storage_result.details);

        CheckResult::from_details(all_details)
    }

    fn component_name(&self) -> &'static str {
        "Datanode"
    }
}
