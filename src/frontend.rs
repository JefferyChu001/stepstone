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

        if let Some(server_config) = &self.config.server {
            // Check if configured addresses are valid
            if let Some(addr) = &server_config.addr {
                match self.parse_address(addr) {
                    Ok(_) => {
                        details.push(CheckDetail::pass(
                            "Server Address Configuration".to_string(),
                            format!("Server address '{}' is valid", addr),
                            None,
                        ));
                    }
                    Err(e) => {
                        details.push(CheckDetail::fail(
                            "Server Address Configuration".to_string(),
                            format!("Invalid server address '{}': {}", addr, e),
                            None,
                            Some("Check server address format".to_string()),
                        ));
                    }
                }
            }

            if let Some(http_addr) = &server_config.http_addr {
                match self.parse_address(http_addr) {
                    Ok(_) => {
                        details.push(CheckDetail::pass(
                            "HTTP Address Configuration".to_string(),
                            format!("HTTP address '{}' is valid", http_addr),
                            None,
                        ));
                    }
                    Err(e) => {
                        details.push(CheckDetail::fail(
                            "HTTP Address Configuration".to_string(),
                            format!("Invalid HTTP address '{}': {}", http_addr, e),
                            None,
                            Some("Check HTTP address format".to_string()),
                        ));
                    }
                }
            }

            if let Some(grpc_addr) = &server_config.grpc_addr {
                match self.parse_address(grpc_addr) {
                    Ok(_) => {
                        details.push(CheckDetail::pass(
                            "gRPC Address Configuration".to_string(),
                            format!("gRPC address '{}' is valid", grpc_addr),
                            None,
                        ));
                    }
                    Err(e) => {
                        details.push(CheckDetail::fail(
                            "gRPC Address Configuration".to_string(),
                            format!("Invalid gRPC address '{}': {}", grpc_addr, e),
                            None,
                            Some("Check gRPC address format".to_string()),
                        ));
                    }
                }
            }
        } else {
            details.push(CheckDetail::warning(
                "Server Configuration".to_string(),
                "No server configuration found, using defaults".to_string(),
                None,
                Some("Consider adding server configuration for production use".to_string()),
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
