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
}
