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

use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Configuration for Metasrv component (matches actual GreptimeDB format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetasrvConfig {
    /// Data home directory
    pub data_home: Option<String>,
    /// Store addresses
    pub store_addrs: Vec<String>,
    /// Store key prefix
    pub store_key_prefix: Option<String>,
    /// Backend type
    pub backend: String,
    /// Meta table name (for RDS backends)
    pub meta_table_name: Option<String>,
    /// Meta schema name (for PostgreSQL)
    pub meta_schema_name: Option<String>,
    /// Advisory lock id for PostgreSQL
    pub meta_election_lock_id: Option<i32>,
    /// Datanode selector type
    pub selector: Option<String>,
    /// Use memory store
    pub use_memory_store: Option<bool>,
    /// Enable region failover
    pub enable_region_failover: Option<bool>,
    /// gRPC server configuration
    pub grpc: Option<GrpcConfig>,
    /// HTTP server configuration
    pub http: Option<HttpConfig>,
    /// Backend TLS configuration
    pub backend_tls: Option<TlsConfig>,
}

/// Configuration for Frontend component (matches actual GreptimeDB format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendConfig {
    /// Data home directory
    pub data_home: Option<String>,
    /// Default timezone
    pub default_timezone: Option<String>,
    /// HTTP server configuration
    pub http: Option<HttpConfig>,
    /// gRPC server configuration
    pub grpc: Option<GrpcConfig>,
    /// Metasrv client configuration
    pub meta_client: Option<MetaClientConfig>,
    /// Heartbeat configuration
    pub heartbeat: Option<HeartbeatConfig>,
    /// Prometheus configuration
    pub prometheus: Option<PrometheusConfig>,
    /// Logging configuration
    pub logging: Option<LoggingConfig>,
}

/// Configuration for Datanode component (matches actual GreptimeDB format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatanodeConfig {
    /// Datanode identifier
    pub node_id: Option<u64>,
    /// Start services after regions have obtained leases
    pub require_lease_before_startup: Option<bool>,
    /// Initialize all regions in the background during startup
    pub init_regions_in_background: Option<bool>,
    /// Parallelism of initializing regions
    pub init_regions_parallelism: Option<u32>,
    /// Maximum concurrent queries allowed
    pub max_concurrent_queries: Option<u32>,
    /// Enable telemetry
    pub enable_telemetry: Option<bool>,
    /// HTTP server configuration
    pub http: Option<HttpConfig>,
    /// gRPC server configuration
    pub grpc: Option<GrpcConfig>,
    /// Heartbeat configuration
    pub heartbeat: Option<HeartbeatConfig>,
    /// Metasrv client configuration
    pub meta_client: Option<MetaClientConfig>,
    /// WAL configuration
    pub wal: Option<WalConfig>,
    /// Storage configuration
    pub storage: Option<DatanodeStorageConfig>,
    /// Query configuration
    pub query: Option<QueryConfig>,
    /// Logging configuration
    pub logging: Option<LoggingConfig>,
}

/// Store configuration for Metasrv
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreConfig {
    /// Store type: "etcd_store", "postgres_store", "mysql_store", "memory_store"
    pub store_type: String,
    /// Store addresses
    pub store_addrs: Vec<String>,
    /// Store key prefix
    pub store_key_prefix: Option<String>,
    /// Maximum transaction operations
    pub max_txn_ops: Option<usize>,
    /// Metadata table name (for SQL stores)
    pub meta_table_name: Option<String>,
    /// TLS configuration
    pub tls: Option<TlsConfig>,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server address
    pub addr: Option<String>,
    /// HTTP address
    pub http_addr: Option<String>,
    /// gRPC address
    pub grpc_addr: Option<String>,
}

/// gRPC server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcConfig {
    /// Bind address
    pub addr: Option<String>,
    /// Server address
    pub server_addr: Option<String>,
    /// Runtime size
    pub runtime_size: Option<u32>,
    /// Max receive message size
    pub max_recv_message_size: Option<String>,
    /// Max send message size
    pub max_send_message_size: Option<String>,
}

/// HTTP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    /// HTTP address
    pub addr: Option<String>,
    /// Request timeout
    pub timeout: Option<String>,
    /// Body limit
    pub body_limit: Option<String>,
    /// Max connections
    pub max_connections: Option<u32>,
}

/// Metasrv client configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaClientConfig {
    /// Metasrv addresses
    pub metasrv_addrs: Vec<String>,
    /// Operation timeout
    pub timeout: Option<String>,
    /// Heartbeat timeout
    pub heartbeat_timeout: Option<String>,
    /// DDL timeout
    pub ddl_timeout: Option<String>,
    /// Connect timeout
    pub connect_timeout: Option<String>,
    /// TCP nodelay
    pub tcp_nodelay: Option<bool>,
}

/// Heartbeat configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatConfig {
    /// Heartbeat interval
    pub interval: Option<String>,
    /// Retry interval
    pub retry_interval: Option<String>,
}

/// Prometheus configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrometheusConfig {
    /// Enable prometheus
    pub enable: Option<bool>,
    /// With metric engine
    pub with_metric_engine: Option<bool>,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    pub level: Option<String>,
    /// Log directory
    pub dir: Option<String>,
}

/// WAL configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalConfig {
    /// WAL provider
    pub provider: Option<String>,
    /// WAL directory
    pub dir: Option<String>,
    /// File size
    pub file_size: Option<String>,
    /// Purge threshold
    pub purge_threshold: Option<String>,
    /// Purge interval
    pub purge_interval: Option<String>,
    /// Read batch size
    pub read_batch_size: Option<u32>,
    /// Sync write
    pub sync_write: Option<bool>,
}

/// Datanode storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatanodeStorageConfig {
    /// Data home directory
    pub data_home: Option<String>,
    /// Storage type
    #[serde(rename = "type")]
    pub storage_type: Option<String>,
    /// Cache capacity
    pub cache_capacity: Option<String>,
    /// Cache path
    pub cache_path: Option<String>,
    /// S3 bucket
    pub bucket: Option<String>,
    /// S3 root
    pub root: Option<String>,
    /// Access key ID
    pub access_key_id: Option<String>,
    /// Secret access key
    pub secret_access_key: Option<String>,
    /// Endpoint
    pub endpoint: Option<String>,
    /// Region
    pub region: Option<String>,
}

/// Query configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryConfig {
    /// Query parallelism
    pub parallelism: Option<u32>,
    /// Allow query fallback
    pub allow_query_fallback: Option<bool>,
}

/// Storage configuration for Datanode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Storage type: "S3", "Oss", "Azblob", "Gcs", "File"
    pub storage_type: String,
    /// Storage configuration data
    #[serde(flatten)]
    pub config: HashMap<String, toml::Value>,
}

/// TLS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Certificate file path
    pub cert: Option<String>,
    /// Private key file path
    pub key: Option<String>,
    /// CA certificate file path
    pub ca: Option<String>,
    /// Server name for verification
    pub server_name: Option<String>,
}

/// S3-compatible storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Config {
    /// S3 bucket name
    pub bucket: String,
    /// Root path in the bucket
    pub root: Option<String>,
    /// Access key ID
    pub access_key_id: String,
    /// Secret access key
    pub secret_access_key: String,
    /// S3 endpoint URL
    pub endpoint: Option<String>,
    /// AWS region
    pub region: Option<String>,
}

/// OSS storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OssConfig {
    /// OSS bucket name
    pub bucket: String,
    /// Root path in the bucket
    pub root: Option<String>,
    /// Access key ID
    pub access_key_id: String,
    /// Access key secret
    pub access_key_secret: String,
    /// OSS endpoint
    pub endpoint: String,
}

/// Azure Blob storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzblobConfig {
    /// Container name
    pub container: String,
    /// Root path in the container
    pub root: Option<String>,
    /// Account name
    pub account_name: String,
    /// Account key
    pub account_key: String,
    /// Endpoint URL
    pub endpoint: Option<String>,
}

/// Google Cloud Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcsConfig {
    /// GCS bucket name
    pub bucket: String,
    /// Root path in the bucket
    pub root: Option<String>,
    /// Service account key (JSON)
    pub service_account: Option<String>,
    /// Service account key file path
    pub service_account_path: Option<String>,
}

/// Configuration parser utility
pub struct ConfigParser;

impl ConfigParser {
    /// Parse Metasrv configuration from TOML file
    pub fn parse_metasrv_config<P: AsRef<Path>>(path: P) -> crate::error::Result<MetasrvConfig> {
        let content = fs::read_to_string(&path).context(crate::error::FileSystemSnafu {
            message: format!("Failed to read config file: {:?}", path.as_ref()),
        })?;

        toml::from_str(&content).context(crate::error::TomlParsingSnafu {
            message: "Failed to parse metasrv TOML config".to_string(),
        })
    }

    /// Parse Frontend configuration from TOML file
    pub fn parse_frontend_config<P: AsRef<Path>>(path: P) -> crate::error::Result<FrontendConfig> {
        let content = fs::read_to_string(&path).context(crate::error::FileSystemSnafu {
            message: format!("Failed to read config file: {:?}", path.as_ref()),
        })?;

        toml::from_str(&content).context(crate::error::TomlParsingSnafu {
            message: "Failed to parse frontend TOML config".to_string(),
        })
    }

    /// Parse Datanode configuration from TOML file
    pub fn parse_datanode_config<P: AsRef<Path>>(path: P) -> crate::error::Result<DatanodeConfig> {
        let content = fs::read_to_string(&path).context(crate::error::FileSystemSnafu {
            message: format!("Failed to read config file: {:?}", path.as_ref()),
        })?;

        toml::from_str(&content).context(crate::error::TomlParsingSnafu {
            message: "Failed to parse datanode TOML config".to_string(),
        })
    }
}

impl StorageConfig {
    /// Convert to S3 configuration
    pub fn as_s3_config(&self) -> crate::error::Result<S3Config> {
        let bucket = self.config.get("bucket")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "default-bucket".to_string());

        let access_key_id = self.config.get("access_key_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "".to_string());

        let secret_access_key = self.config.get("secret_access_key")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "".to_string());

        Ok(S3Config {
            bucket,
            root: self.config.get("root").and_then(|v| v.as_str()).map(|s| s.to_string()),
            access_key_id,
            secret_access_key,
            endpoint: self.config.get("endpoint").and_then(|v| v.as_str()).map(|s| s.to_string()),
            region: self.config.get("region").and_then(|v| v.as_str()).map(|s| s.to_string()),
        })
    }

    /// Convert to OSS configuration
    pub fn as_oss_config(&self) -> crate::error::Result<OssConfig> {
        Ok(OssConfig {
            bucket: self.config.get("bucket").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_default(),
            root: self.config.get("root").and_then(|v| v.as_str()).map(|s| s.to_string()),
            access_key_id: self.config.get("access_key_id").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_default(),
            access_key_secret: self.config.get("access_key_secret").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_default(),
            endpoint: self.config.get("endpoint").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_default(),
        })
    }

    /// Convert to Azure Blob configuration
    pub fn as_azblob_config(&self) -> crate::error::Result<AzblobConfig> {
        Ok(AzblobConfig {
            container: self.config.get("container").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_default(),
            root: self.config.get("root").and_then(|v| v.as_str()).map(|s| s.to_string()),
            account_name: self.config.get("account_name").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_default(),
            account_key: self.config.get("account_key").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_default(),
            endpoint: self.config.get("endpoint").and_then(|v| v.as_str()).map(|s| s.to_string()),
        })
    }

    /// Convert to GCS configuration
    pub fn as_gcs_config(&self) -> crate::error::Result<GcsConfig> {
        Ok(GcsConfig {
            bucket: self.config.get("bucket").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_default(),
            root: self.config.get("root").and_then(|v| v.as_str()).map(|s| s.to_string()),
            service_account: self.config.get("service_account").and_then(|v| v.as_str()).map(|s| s.to_string()),
            service_account_path: self.config.get("service_account_path").and_then(|v| v.as_str()).map(|s| s.to_string()),
        })
    }
}

impl ConfigParser {
    /// Try to parse configuration from different possible formats
    pub fn parse_config_flexible<P: AsRef<Path>>(path: P) -> crate::error::Result<toml::Value> {
        let content = fs::read_to_string(&path).map_err(|e| crate::error::Error::ConfigLoad {
            message: format!("Failed to read config file: {:?}: {}", path.as_ref(), e),
        })?;

        // Try TOML first
        toml::from_str(&content).map_err(|e| crate::error::Error::ConfigLoad {
            message: format!("Failed to parse config as TOML: {}", e),
        })
    }

    /// Create a default metasrv config for testing
    pub fn default_metasrv_config() -> MetasrvConfig {
        MetasrvConfig {
            data_home: Some("./greptimedb_data".to_string()),
            store_addrs: vec!["127.0.0.1:2379".to_string()],
            store_key_prefix: Some("/greptime".to_string()),
            backend: "memory_store".to_string(),
            meta_table_name: Some("greptime_metasrv".to_string()),
            meta_schema_name: None,
            meta_election_lock_id: None,
            selector: None,
            use_memory_store: Some(true),
            enable_region_failover: None,
            grpc: None,
            http: None,
            backend_tls: None,
        }
    }

    /// Create a default frontend config for testing
    pub fn default_frontend_config() -> FrontendConfig {
        FrontendConfig {
            data_home: Some("./greptimedb_data".to_string()),
            default_timezone: Some("UTC".to_string()),
            http: None,
            grpc: None,
            meta_client: Some(MetaClientConfig {
                metasrv_addrs: vec!["127.0.0.1:3002".to_string()],
                timeout: Some("3s".to_string()),
                heartbeat_timeout: Some("500ms".to_string()),
                ddl_timeout: Some("10s".to_string()),
                connect_timeout: Some("1s".to_string()),
                tcp_nodelay: Some(true),
            }),
            heartbeat: None,
            prometheus: None,
            logging: None,
        }
    }

    /// Create a default datanode config for testing
    pub fn default_datanode_config() -> DatanodeConfig {
        DatanodeConfig {
            node_id: Some(1),
            require_lease_before_startup: Some(false),
            init_regions_in_background: Some(false),
            init_regions_parallelism: Some(16),
            max_concurrent_queries: Some(0),
            enable_telemetry: Some(true),
            http: Some(HttpConfig {
                addr: Some("127.0.0.1:4000".to_string()),
                timeout: Some("30s".to_string()),
                body_limit: None,
                max_connections: None,
            }),
            grpc: Some(GrpcConfig {
                addr: Some("127.0.0.1:4001".to_string()),
                server_addr: None,
                runtime_size: Some(8),
                max_recv_message_size: None,
                max_send_message_size: None,
            }),
            heartbeat: Some(HeartbeatConfig {
                interval: Some("18s".to_string()),
                retry_interval: Some("3s".to_string()),
            }),
            meta_client: Some(MetaClientConfig {
                metasrv_addrs: vec!["127.0.0.1:3002".to_string()],
                timeout: Some("3s".to_string()),
                heartbeat_timeout: Some("500ms".to_string()),
                ddl_timeout: Some("10s".to_string()),
                connect_timeout: Some("1s".to_string()),
                tcp_nodelay: Some(true),
            }),
            wal: None,
            storage: Some(DatanodeStorageConfig {
                data_home: Some("./greptimedb_data".to_string()),
                storage_type: Some("File".to_string()),
                cache_capacity: None,
                cache_path: None,
                bucket: None,
                root: None,
                access_key_id: None,
                secret_access_key: None,
                endpoint: None,
                region: None,
            }),
            query: None,
            logging: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_configs() {
        let metasrv_config = ConfigParser::default_metasrv_config();
        assert_eq!(metasrv_config.backend, "memory_store");

        let frontend_config = ConfigParser::default_frontend_config();
        assert!(frontend_config.meta_client.is_some());
        assert_eq!(frontend_config.meta_client.unwrap().metasrv_addrs, vec!["127.0.0.1:3002"]);

        let datanode_config = ConfigParser::default_datanode_config();
        assert!(datanode_config.meta_client.is_some());
        assert_eq!(datanode_config.meta_client.unwrap().metasrv_addrs, vec!["127.0.0.1:3002"]);
        assert!(datanode_config.storage.is_some());
        assert_eq!(datanode_config.storage.unwrap().storage_type, Some("File".to_string()));
    }

    #[test]
    fn test_s3_config_parsing() {
        let mut storage_config = HashMap::new();
        storage_config.insert("bucket".to_string(), toml::Value::String("test-bucket".to_string()));
        storage_config.insert("access_key_id".to_string(), toml::Value::String("test-key".to_string()));
        storage_config.insert("secret_access_key".to_string(), toml::Value::String("test-secret".to_string()));
        storage_config.insert("region".to_string(), toml::Value::String("us-east-1".to_string()));

        let storage = StorageConfig {
            storage_type: "S3".to_string(),
            config: storage_config,
        };

        let s3_config = storage.as_s3_config().unwrap();
        assert_eq!(s3_config.bucket, "test-bucket");
        assert_eq!(s3_config.access_key_id, "test-key");
        assert_eq!(s3_config.secret_access_key, "test-secret");
        assert_eq!(s3_config.region, Some("us-east-1".to_string()));
    }

    #[test]
    fn test_metasrv_config_parsing() {
        let toml_content = r#"
data_home = "./test_data"
store_addrs = ["127.0.0.1:2379"]
store_key_prefix = "/greptime"
backend = "etcd_store"

[grpc]
addr = "0.0.0.0:3002"
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(toml_content.as_bytes()).unwrap();

        let config = ConfigParser::parse_metasrv_config(temp_file.path()).unwrap();
        assert_eq!(config.backend, "etcd_store");
        assert_eq!(config.store_addrs, vec!["127.0.0.1:2379"]);
        assert_eq!(config.store_key_prefix, Some("/greptime".to_string()));
    }

    #[test]
    fn test_frontend_config_parsing() {
        let toml_content = r#"
default_timezone = "UTC"

[meta_client]
metasrv_addrs = ["127.0.0.1:3002", "127.0.0.1:3003"]
timeout = "3s"

[http]
addr = "0.0.0.0:4000"

[grpc]
addr = "0.0.0.0:4001"
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(toml_content.as_bytes()).unwrap();

        let config = ConfigParser::parse_frontend_config(temp_file.path()).unwrap();
        assert!(config.meta_client.is_some());
        let meta_client = config.meta_client.unwrap();
        assert_eq!(meta_client.metasrv_addrs, vec!["127.0.0.1:3002", "127.0.0.1:3003"]);

        assert!(config.http.is_some());
        let http = config.http.unwrap();
        assert_eq!(http.addr, Some("0.0.0.0:4000".to_string()));

        assert!(config.grpc.is_some());
        let grpc = config.grpc.unwrap();
        assert_eq!(grpc.addr, Some("0.0.0.0:4001".to_string()));
    }

    #[test]
    fn test_datanode_config_parsing() {
        let toml_content = r#"
node_id = 1

[meta_client]
metasrv_addrs = ["127.0.0.1:3002"]
timeout = "3s"

[storage]
type = "S3"
bucket = "my-bucket"
access_key_id = "my-key"
secret_access_key = "my-secret"
region = "us-west-2"
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(toml_content.as_bytes()).unwrap();

        let config = ConfigParser::parse_datanode_config(temp_file.path()).unwrap();
        assert!(config.meta_client.is_some());
        let meta_client = config.meta_client.unwrap();
        assert_eq!(meta_client.metasrv_addrs, vec!["127.0.0.1:3002"]);

        assert!(config.storage.is_some());
        let storage = config.storage.unwrap();
        assert_eq!(storage.storage_type, Some("S3".to_string()));
        assert_eq!(storage.bucket, Some("my-bucket".to_string()));
        assert_eq!(storage.access_key_id, Some("my-key".to_string()));
        assert_eq!(storage.secret_access_key, Some("my-secret".to_string()));
        assert_eq!(storage.region, Some("us-west-2".to_string()));
    }
}
