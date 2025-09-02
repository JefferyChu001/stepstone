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
use crate::error;
use async_trait::async_trait;
use opendal::services::S3;
use opendal::Operator;
use snafu::ResultExt;
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

        let metasrv_addrs = if let Some(meta_client) = &self.config.meta_client {
            &meta_client.metasrv_addrs
        } else {
            details.push(CheckDetail::fail(
                "Metasrv Configuration".to_string(),
                "No meta_client configuration found".to_string(),
                None,
                Some("Configure meta_client section in the configuration file".to_string()),
            ));
            return CheckResult::from_details(details);
        };

        if metasrv_addrs.is_empty() {
            details.push(CheckDetail::fail(
                "Metasrv Configuration".to_string(),
                "No metasrv addresses configured".to_string(),
                None,
                Some("Configure metasrv_addrs in the meta_client section".to_string()),
            ));
            return CheckResult::from_details(details);
        }

        for (index, addr) in metasrv_addrs.iter().enumerate() {
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
        let storage_config = match &self.config.storage {
            Some(config) => config,
            None => {
                return CheckResult::failure(
                    "No storage configuration found".to_string(),
                    vec![CheckDetail::fail(
                        "Storage Configuration".to_string(),
                        "Storage configuration is missing".to_string(),
                        None,
                        Some("Add storage configuration section".to_string()),
                    )],
                );
            }
        };

        let storage_type = storage_config.storage_type.as_deref().unwrap_or("File");
        match storage_type {
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

        // Get S3 configuration from storage config
        let storage_config = self.config.storage.as_ref().unwrap();

        let bucket = match &storage_config.bucket {
            Some(bucket) => bucket,
            None => {
                details.push(CheckDetail::fail(
                    "S3 Configuration".to_string(),
                    "S3 bucket name is required".to_string(),
                    None,
                    Some("Set bucket name in storage configuration".to_string()),
                ));
                return CheckResult::from_details(details);
            }
        };

        let access_key_id = storage_config.access_key_id.as_deref().unwrap_or("");
        let secret_access_key = storage_config.secret_access_key.as_deref().unwrap_or("");
        let endpoint = storage_config.endpoint.as_deref().unwrap_or("https://s3.amazonaws.com");
        let region = storage_config.region.as_deref().unwrap_or("us-east-1");

        // Build S3 operator
        let builder = S3::default()
            .root(storage_config.root.as_deref().unwrap_or(""))
            .bucket(bucket)
            .access_key_id(access_key_id)
            .secret_access_key(secret_access_key)
            .endpoint(endpoint)
            .region(region);

        match Operator::new(builder) {
            Ok(op) => {
                let op = op.finish();
                details.push(CheckDetail::pass(
                    "S3 Client Creation".to_string(),
                    "S3 client created successfully".to_string(),
                    Some(start.elapsed()),
                ));

                // First, test bucket access permissions
                self.test_s3_bucket_permissions(&op, &mut details).await;

                // Test basic operations
                let test_key = format!("stepstone-test/{}", Uuid::new_v4());
                let test_data = b"stepstone-test-data";

                // PUT test (this tests write permissions)
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

                                // Performance tests
                                self.test_s3_performance(&op, &mut details).await;
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
        let storage_config = self.config.storage.as_ref().unwrap();
        let root_path = storage_config.data_home.as_deref().unwrap_or("./greptimedb_data");

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
    fn parse_address(&self, addr: &str) -> error::Result<(String, u16)> {
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
    fn parse_host_port(&self, addr: &str) -> error::Result<(String, u16)> {
        if let Some(colon_pos) = addr.rfind(':') {
            let host = addr[..colon_pos].to_string();
            let port_str = &addr[colon_pos + 1..];

            // Remove any path component
            let port_str = if let Some(slash_pos) = port_str.find('/') {
                &port_str[..slash_pos]
            } else {
                port_str
            };

            port_str.parse::<u16>()
                .map(|port| (host, port))
                .context(error::InvalidPortSnafu {
                    address: addr.to_string(),
                    port_str: port_str.to_string(),
                })
        } else {
            error::MissingPortSnafu {
                address: addr.to_string(),
            }.fail()
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

impl DatanodeChecker {
    /// Test S3 storage performance (throughput and latency)
    async fn test_s3_performance(&self, op: &opendal::Operator, details: &mut Vec<CheckDetail>) {
        use std::time::Instant;
        use tokio::time::{timeout, Duration};

        // Test small file performance (64MB)
        let small_data = vec![0u8; 64 * 1024 * 1024]; // 64MB
        let small_key = "stepstone_perf_test_64mb";

        let start = Instant::now();
        match timeout(Duration::from_secs(120), op.write(small_key, small_data.clone())).await {
            Ok(Ok(_)) => {
                let write_duration = start.elapsed();
                let throughput_mbps = 64.0 / write_duration.as_secs_f64();

                details.push(CheckDetail::pass(
                    "S3 64MB File Write Performance".to_string(),
                    format!("64MB write: {:.2}ms ({:.2} MB/s)",
                           write_duration.as_millis(), throughput_mbps),
                    Some(write_duration),
                ));

                // Test read performance
                let start = Instant::now();
                match timeout(Duration::from_secs(120), op.read(small_key)).await {
                    Ok(Ok(data)) => {
                        let read_duration = start.elapsed();
                        let read_throughput_mbps = (data.len() as f64 / read_duration.as_secs_f64()) / (1024.0 * 1024.0);

                        details.push(CheckDetail::pass(
                            "S3 64MB File Read Performance".to_string(),
                            format!("64MB read: {:.2}ms ({:.2} MB/s)",
                                   read_duration.as_millis(), read_throughput_mbps),
                            Some(read_duration),
                        ));
                    }
                    Ok(Err(e)) => {
                        details.push(CheckDetail::warning(
                            "S3 64MB File Read Performance".to_string(),
                            format!("Read test failed: {}", e),
                            None,
                            Some("Performance test incomplete".to_string()),
                        ));
                    }
                    Err(_) => {
                        details.push(CheckDetail::warning(
                            "S3 64MB File Read Performance".to_string(),
                            "Read test timed out (>120s)".to_string(),
                            None,
                            Some("S3 read performance may be slow".to_string()),
                        ));
                    }
                }

                // Cleanup
                let _ = op.delete(small_key).await;
            }
            Ok(Err(e)) => {
                details.push(CheckDetail::warning(
                    "S3 64MB File Write Performance".to_string(),
                    format!("Write test failed: {}", e),
                    None,
                    Some("Performance test incomplete".to_string()),
                ));
            }
            Err(_) => {
                details.push(CheckDetail::warning(
                    "S3 64MB File Write Performance".to_string(),
                    "Write test timed out (>120s)".to_string(),
                    None,
                    Some("S3 write performance may be slow".to_string()),
                ));
            }
        }

        // Test larger file performance (1GB)
        let large_data = vec![0u8; 1024 * 1024 * 1024]; // 1GB
        let large_key = "stepstone_perf_test_1gb";

        let start = Instant::now();
        match timeout(Duration::from_secs(300), op.write(large_key, large_data.clone())).await {
            Ok(Ok(_)) => {
                let write_duration = start.elapsed();
                let throughput_mbps = 1024.0 / write_duration.as_secs_f64();

                details.push(CheckDetail::pass(
                    "S3 1GB File Write Performance".to_string(),
                    format!("1GB write: {:.2}ms ({:.2} MB/s)",
                           write_duration.as_millis(), throughput_mbps),
                    Some(write_duration),
                ));

                // Cleanup large file
                let _ = op.delete(large_key).await;
            }
            Ok(Err(e)) => {
                details.push(CheckDetail::warning(
                    "S3 1GB File Write Performance".to_string(),
                    format!("1GB file write test failed: {}", e),
                    None,
                    Some("May indicate bandwidth or timeout issues".to_string()),
                ));
            }
            Err(_) => {
                details.push(CheckDetail::warning(
                    "S3 1GB File Write Performance".to_string(),
                    "1GB file write test timed out (>300s)".to_string(),
                    None,
                    Some("S3 write performance for large files may be slow".to_string()),
                ));
            }
        }

        // Test concurrent operations
        self.test_s3_concurrent_performance(op, details).await;
    }

    /// Test S3 concurrent operation performance
    async fn test_s3_concurrent_performance(&self, op: &opendal::Operator, details: &mut Vec<CheckDetail>) {
        use std::time::Instant;
        use tokio::time::{timeout, Duration};

        let concurrent_count = 100;
        let data = vec![0u8; 512]; // 512 bytes per operation

        let start = Instant::now();
        let mut handles = Vec::new();

        for i in 0..concurrent_count {
            let op_clone = op.clone();
            let data_clone = data.clone();
            let key = format!("stepstone_concurrent_test_{}", i);
            let key_clone = key.clone();

            let handle = tokio::spawn(async move {
                op_clone.write(&key_clone, data_clone).await
            });
            handles.push((handle, key));
        }

        let mut successful_ops = 0;
        let mut keys_to_cleanup = Vec::new();

        for (handle, key) in handles {
            match timeout(Duration::from_secs(10), handle).await {
                Ok(Ok(Ok(_))) => {
                    successful_ops += 1;
                    keys_to_cleanup.push(key);
                }
                _ => {} // Failed or timed out
            }
        }

        let total_duration = start.elapsed();
        let ops_per_second = successful_ops as f64 / total_duration.as_secs_f64();

        if successful_ops == concurrent_count {
            details.push(CheckDetail::pass(
                "S3 Concurrent Operations".to_string(),
                format!("{} concurrent writes: {:.2}ms ({:.1} ops/s)",
                       concurrent_count, total_duration.as_millis(), ops_per_second),
                Some(total_duration),
            ));
        } else {
            details.push(CheckDetail::warning(
                "S3 Concurrent Operations".to_string(),
                format!("{}/{} concurrent writes succeeded: {:.2}ms ({:.1} ops/s)",
                       successful_ops, concurrent_count, total_duration.as_millis(), ops_per_second),
                Some(total_duration),
                Some("Some concurrent operations failed or timed out".to_string()),
            ));
        }

        // Cleanup
        for key in keys_to_cleanup {
            let _ = op.delete(&key).await;
        }
    }

    /// Test S3 bucket permissions (list, read, write, delete)
    async fn test_s3_bucket_permissions(&self, op: &opendal::Operator, details: &mut Vec<CheckDetail>) {
        use std::time::Instant;
        use tokio::time::{timeout, Duration};

        // Test 1: List bucket contents (requires ListBucket permission)
        let start = Instant::now();
        match timeout(Duration::from_secs(30), op.list("")).await {
            Ok(Ok(_)) => {
                details.push(CheckDetail::pass(
                    "S3 Bucket List Permission".to_string(),
                    "Successfully listed bucket contents (ListBucket permission verified)".to_string(),
                    Some(start.elapsed()),
                ));
            }
            Ok(Err(e)) => {
                let error_msg = format!("{}", e);
                if error_msg.contains("AccessDenied") || error_msg.contains("Forbidden") {
                    details.push(CheckDetail::fail(
                        "S3 Bucket List Permission".to_string(),
                        format!("Access denied for bucket listing: {}", e),
                        Some(start.elapsed()),
                        Some("Check if the AKSK has ListBucket permission for this bucket".to_string()),
                    ));
                } else if error_msg.contains("NoSuchBucket") {
                    details.push(CheckDetail::fail(
                        "S3 Bucket Existence".to_string(),
                        format!("Bucket does not exist: {}", e),
                        Some(start.elapsed()),
                        Some("Create the bucket or check the bucket name in configuration".to_string()),
                    ));
                } else if error_msg.contains("InvalidAccessKeyId") {
                    details.push(CheckDetail::fail(
                        "S3 Access Key Validation".to_string(),
                        format!("Invalid access key: {}", e),
                        Some(start.elapsed()),
                        Some("Check the access_key_id in configuration".to_string()),
                    ));
                } else if error_msg.contains("SignatureDoesNotMatch") {
                    details.push(CheckDetail::fail(
                        "S3 Secret Key Validation".to_string(),
                        format!("Invalid secret key: {}", e),
                        Some(start.elapsed()),
                        Some("Check the secret_access_key in configuration".to_string()),
                    ));
                } else {
                    details.push(CheckDetail::warning(
                        "S3 Bucket List Permission".to_string(),
                        format!("Bucket listing failed: {}", e),
                        Some(start.elapsed()),
                        Some("This may indicate network issues or other S3 service problems".to_string()),
                    ));
                }
            }
            Err(_) => {
                details.push(CheckDetail::warning(
                    "S3 Bucket List Permission".to_string(),
                    "Bucket listing timed out (>30s)".to_string(),
                    Some(start.elapsed()),
                    Some("Check network connectivity to S3 endpoint".to_string()),
                ));
            }
        }

        // Test 2: Write permission test (will be done in main PUT test)
        // Test 3: Read permission test (will be done in main GET test)
        // Test 4: Delete permission test (will be done in main DELETE test)

        // Test 5: Try to access a non-existent object to test error handling
        let non_existent_key = format!("stepstone-nonexistent-{}", uuid::Uuid::new_v4());
        match timeout(Duration::from_secs(10), op.read(&non_existent_key)).await {
            Ok(Err(e)) => {
                let error_msg = format!("{}", e);
                if error_msg.contains("NoSuchKey") || error_msg.contains("NotFound") {
                    details.push(CheckDetail::pass(
                        "S3 Read Permission (Error Handling)".to_string(),
                        "Correctly returned 'not found' for non-existent object".to_string(),
                        None,
                    ));
                } else if error_msg.contains("AccessDenied") || error_msg.contains("Forbidden") {
                    details.push(CheckDetail::fail(
                        "S3 Read Permission".to_string(),
                        format!("Access denied for reading objects: {}", e),
                        None,
                        Some("Check if the AKSK has GetObject permission for this bucket".to_string()),
                    ));
                } else {
                    details.push(CheckDetail::warning(
                        "S3 Read Permission (Error Handling)".to_string(),
                        format!("Unexpected error for non-existent object: {}", e),
                        None,
                        Some("This may indicate permission or configuration issues".to_string()),
                    ));
                }
            }
            Ok(Ok(_)) => {
                details.push(CheckDetail::warning(
                    "S3 Read Permission (Error Handling)".to_string(),
                    "Unexpectedly found data for non-existent object".to_string(),
                    None,
                    Some("This may indicate caching issues or incorrect object naming".to_string()),
                ));
            }
            Err(_) => {
                details.push(CheckDetail::warning(
                    "S3 Read Permission (Error Handling)".to_string(),
                    "Read test for non-existent object timed out".to_string(),
                    None,
                    Some("Check network connectivity to S3 endpoint".to_string()),
                ));
            }
        }
    }
}
