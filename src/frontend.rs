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
use crate::config::FrontendConfig;
use crate::error;
use async_trait::async_trait;
use snafu::ResultExt;
use std::fmt::{Debug, Formatter};
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::time::timeout;

/// Frontend component checker
pub struct FrontendChecker {
    config: FrontendConfig,
}

impl Debug for FrontendChecker {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "FrontendChecker")
    }
}

impl FrontendChecker {
    /// Create a new FrontendChecker with the given configuration
    pub fn new(config: FrontendConfig) -> Self {
        Self { config }
    }

    /// Check connectivity to metasrv endpoints
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

    /// Parse address string into host and port
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

    /// Check server configuration if present
    async fn check_server_config(&self) -> CheckResult {
        let mut details = Vec::new();

        // Check HTTP server configuration
        if let Some(http_config) = &self.config.http {
            if let Some(addr) = &http_config.addr {
                match self.parse_address(addr) {
                    Ok(_) => {
                        details.push(CheckDetail::pass(
                            "HTTP Server Address Configuration".to_string(),
                            format!("HTTP server address '{}' is valid", addr),
                            None,
                        ));
                    }
                    Err(e) => {
                        details.push(CheckDetail::fail(
                            "HTTP Server Address Configuration".to_string(),
                            format!("Invalid HTTP server address '{}': {}", addr, e),
                            None,
                            Some("Check HTTP server address format (host:port)".to_string()),
                        ));
                    }
                }
            }
        }

        // Check gRPC server configuration
        if let Some(grpc_config) = &self.config.grpc {
            if let Some(addr) = &grpc_config.addr {
                match self.parse_address(addr) {
                    Ok(_) => {
                        details.push(CheckDetail::pass(
                            "gRPC Server Address Configuration".to_string(),
                            format!("gRPC server address '{}' is valid", addr),
                            None,
                        ));
                    }
                    Err(e) => {
                        details.push(CheckDetail::fail(
                            "gRPC Server Address Configuration".to_string(),
                            format!("Invalid gRPC server address '{}': {}", addr, e),
                            None,
                            Some("Check gRPC server address format (host:port)".to_string()),
                        ));
                    }
                }
            }
        }

        if details.is_empty() {
            details.push(CheckDetail::warning(
                "Server Configuration".to_string(),
                "No HTTP or gRPC server configuration found".to_string(),
                None,
                Some("Consider configuring http and grpc sections for server endpoints".to_string()),
            ));
        }

        CheckResult::from_details(details)
    }
}

#[async_trait]
impl ComponentChecker for FrontendChecker {
    async fn check(&self) -> CheckResult {
        let mut all_details = Vec::new();

        // Check metasrv connectivity
        let metasrv_result = self.check_metasrv_connectivity().await;
        all_details.extend(metasrv_result.details);

        // Check server configuration
        let server_result = self.check_server_config().await;
        all_details.extend(server_result.details);

        CheckResult::from_details(all_details)
    }

    fn component_name(&self) -> &'static str {
        "Frontend"
    }
}
