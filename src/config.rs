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

/// Configuration for Metasrv component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetasrvConfig {
    /// Backend store configuration
    pub store: StoreConfig,
    /// Server configuration
    pub server: Option<ServerConfig>,
}

/// Configuration for Frontend component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendConfig {
    /// Metasrv addresses
    pub metasrv_addrs: Vec<String>,
    /// Server configuration
    pub server: Option<ServerConfig>,
}

/// Configuration for Datanode component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatanodeConfig {
    /// Metasrv addresses
    pub metasrv_addrs: Vec<String>,
    /// Storage configuration
    pub storage: StorageConfig,
    /// Server configuration
    pub server: Option<ServerConfig>,
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
            store: StoreConfig {
                store_type: "memory_store".to_string(),
                store_addrs: vec![],
                store_key_prefix: Some("/greptime".to_string()),
                max_txn_ops: Some(128),
                meta_table_name: Some("greptime_metasrv".to_string()),
                tls: None,
            },
            server: None,
        }
    }

    /// Create a default frontend config for testing
    pub fn default_frontend_config() -> FrontendConfig {
        FrontendConfig {
            metasrv_addrs: vec!["127.0.0.1:3002".to_string()],
            server: None,
        }
    }

    /// Create a default datanode config for testing
    pub fn default_datanode_config() -> DatanodeConfig {
        let mut storage_config = HashMap::new();
        storage_config.insert("root".to_string(), toml::Value::String("./data".to_string()));

        DatanodeConfig {
            metasrv_addrs: vec!["127.0.0.1:3002".to_string()],
            storage: StorageConfig {
                storage_type: "File".to_string(),
                config: storage_config,
            },
            server: None,
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
        assert_eq!(metasrv_config.store.store_type, "memory_store");

        let frontend_config = ConfigParser::default_frontend_config();
        assert_eq!(frontend_config.metasrv_addrs, vec!["127.0.0.1:3002"]);

        let datanode_config = ConfigParser::default_datanode_config();
        assert_eq!(datanode_config.storage.storage_type, "File");
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
[store]
store_type = "etcd_store"
store_addrs = ["127.0.0.1:2379"]
store_key_prefix = "/greptime"
max_txn_ops = 128

[server]
addr = "0.0.0.0:3002"
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(toml_content.as_bytes()).unwrap();

        let config = ConfigParser::parse_metasrv_config(temp_file.path()).unwrap();
        assert_eq!(config.store.store_type, "etcd_store");
        assert_eq!(config.store.store_addrs, vec!["127.0.0.1:2379"]);
        assert_eq!(config.store.store_key_prefix, Some("/greptime".to_string()));
        assert_eq!(config.store.max_txn_ops, Some(128));
    }

    #[test]
    fn test_frontend_config_parsing() {
        let toml_content = r#"
metasrv_addrs = ["127.0.0.1:3002", "127.0.0.1:3003"]

[server]
addr = "0.0.0.0:4000"
http_addr = "0.0.0.0:4001"
grpc_addr = "0.0.0.0:4002"
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(toml_content.as_bytes()).unwrap();

        let config = ConfigParser::parse_frontend_config(temp_file.path()).unwrap();
        assert_eq!(config.metasrv_addrs, vec!["127.0.0.1:3002", "127.0.0.1:3003"]);
        assert!(config.server.is_some());

        let server = config.server.unwrap();
        assert_eq!(server.addr, Some("0.0.0.0:4000".to_string()));
        assert_eq!(server.http_addr, Some("0.0.0.0:4001".to_string()));
        assert_eq!(server.grpc_addr, Some("0.0.0.0:4002".to_string()));
    }

    #[test]
    fn test_datanode_config_parsing() {
        let toml_content = r#"
metasrv_addrs = ["127.0.0.1:3002"]

[storage]
storage_type = "S3"
bucket = "my-bucket"
access_key_id = "my-key"
secret_access_key = "my-secret"
region = "us-west-2"
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(toml_content.as_bytes()).unwrap();

        let config = ConfigParser::parse_datanode_config(temp_file.path()).unwrap();
        assert_eq!(config.metasrv_addrs, vec!["127.0.0.1:3002"]);
        assert_eq!(config.storage.storage_type, "S3");

        let s3_config = config.storage.as_s3_config().unwrap();
        assert_eq!(s3_config.bucket, "my-bucket");
        assert_eq!(s3_config.access_key_id, "my-key");
        assert_eq!(s3_config.secret_access_key, "my-secret");
        assert_eq!(s3_config.region, Some("us-west-2".to_string()));
    }
}
