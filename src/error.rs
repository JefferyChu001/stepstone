// Copyright 2023 Greptime Team
//
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

use common_macro::stack_trace_debug;
use snafu::{Location, Snafu};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Snafu)]
#[snafu(visibility(pub))]
#[stack_trace_debug]
pub enum Error {
    #[snafu(transparent)]
    CommonMeta {
        #[snafu(source)]
        error: common_meta::error::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Cannot operate etcd, provided endpoints: {}", endpoints))]
    EtcdOperation {
        endpoints: String,
        #[snafu(source)]
        error: common_meta::error::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display(
        "Inconsistent etcd value from {}, expect `{}`, actual: `{}`",
        endpoints,
        expect,
        actual
    ))]
    EtcdValueMismatch {
        endpoints: String,
        expect: String,
        actual: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Failed to load configuration: {}", message))]
    ConfigLoad {
        message: String,
    },

    #[snafu(display("Connection failed: {}", message))]
    ConnectionFailed {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Permission denied: {}", message))]
    PermissionDenied {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Performance test failed: {}", message))]
    PerformanceTestFailed {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Database operation failed: {}", message))]
    DatabaseOperation {
        message: String,
        #[snafu(source)]
        error: sqlx::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Object storage operation failed: {}", message))]
    ObjectStoreOperation {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Network operation failed: {}", message))]
    NetworkOperation {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Timeout occurred: {}", message))]
    Timeout {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Invalid configuration: {}", message))]
    InvalidConfig {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("File system operation failed: {}", message))]
    FileSystem {
        message: String,
        #[snafu(source)]
        error: std::io::Error,
        #[snafu(implicit)]
        location: Location,
    },

    // Address parsing errors
    #[snafu(display("Invalid address format: {}", address))]
    InvalidAddress {
        address: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Address must contain port number: {}", address))]
    MissingPort {
        address: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Invalid port number in address {}: {}", address, port_str))]
    InvalidPort {
        address: String,
        port_str: String,
        #[snafu(source)]
        error: std::num::ParseIntError,
        #[snafu(implicit)]
        location: Location,
    },

    // Storage-specific errors
    #[snafu(display("S3 configuration error: {}", message))]
    S3Config {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("S3 operation failed: {}", message))]
    S3Operation {
        message: String,
        #[snafu(source)]
        error: opendal::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("OSS configuration error: {}", message))]
    OssConfig {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("OSS operation failed: {}", message))]
    OssOperation {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Azure Blob configuration error: {}", message))]
    AzureBlobConfig {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Azure Blob operation failed: {}", message))]
    AzureBlobOperation {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Google Cloud Storage configuration error: {}", message))]
    GcsConfig {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Google Cloud Storage operation failed: {}", message))]
    GcsOperation {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("File storage configuration error: {}", message))]
    FileStorageConfig {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("File storage operation failed: {}", message))]
    FileStorageOperation {
        message: String,
        #[snafu(source)]
        error: std::io::Error,
        #[snafu(implicit)]
        location: Location,
    },

    // Metasrv-specific errors
    #[snafu(display("PostgreSQL connection failed: {}", message))]
    PostgresConnection {
        message: String,
        #[snafu(source)]
        error: sqlx::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("PostgreSQL query failed: {}", message))]
    PostgresQuery {
        message: String,
        #[snafu(source)]
        error: sqlx::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("MySQL connection failed: {}", message))]
    MySqlConnection {
        message: String,
        #[snafu(source)]
        error: sqlx::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("MySQL query failed: {}", message))]
    MySqlQuery {
        message: String,
        #[snafu(source)]
        error: sqlx::Error,
        #[snafu(implicit)]
        location: Location,
    },

    // TCP connection errors
    #[snafu(display("TCP connection failed to {}: {}", address, message))]
    TcpConnection {
        address: String,
        message: String,
        #[snafu(source)]
        error: std::io::Error,
        #[snafu(implicit)]
        location: Location,
    },

    // JSON serialization errors
    #[snafu(display("JSON serialization failed: {}", message))]
    JsonSerialization {
        message: String,
        #[snafu(source)]
        error: serde_json::Error,
        #[snafu(implicit)]
        location: Location,
    },

    // TOML parsing errors
    #[snafu(display("TOML parsing failed: {}", message))]
    TomlParsing {
        message: String,
        #[snafu(source)]
        error: toml::de::Error,
        #[snafu(implicit)]
        location: Location,
    },

    // Performance test errors
    #[snafu(display("Performance test setup failed: {}", message))]
    PerformanceTestSetup {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Performance test execution failed: {}", message))]
    PerformanceTestExecution {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    // Unsupported operations
    #[snafu(display("Unsupported storage type: {}", storage_type))]
    UnsupportedStorageType {
        storage_type: String,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Unsupported store type: {}", store_type))]
    UnsupportedStoreType {
        store_type: String,
        #[snafu(implicit)]
        location: Location,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use snafu::ResultExt;

    #[test]
    fn test_missing_port_error() {
        // Test missing port error
        let missing_port_error = MissingPortSnafu {
            address: "localhost".to_string(),
        }.build();

        assert!(missing_port_error.to_string().contains("Address must contain port number"));
        assert!(missing_port_error.to_string().contains("localhost"));
    }

    #[test]
    fn test_config_load_error() {
        let config_error = ConfigLoadSnafu {
            message: "Failed to parse TOML".to_string(),
        }.build();

        assert!(config_error.to_string().contains("Failed to load configuration"));
        assert!(config_error.to_string().contains("Failed to parse TOML"));
    }

    #[test]
    fn test_error_context() {
        // Test that we can use context with our error types
        let result: std::result::Result<(), std::io::Error> = Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found"
        ));

        let error = result.context(FileSystemSnafu {
            message: "Failed to read config file".to_string(),
        }).unwrap_err();

        assert!(error.to_string().contains("File system operation failed"));
        assert!(error.to_string().contains("Failed to read config file"));
    }
}
