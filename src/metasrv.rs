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
use crate::config::MetasrvConfig;
use crate::error;
use async_trait::async_trait;
use common_meta::kv_backend::etcd::EtcdStore;
use common_meta::kv_backend::KvBackendRef;
use common_meta::rpc::store::PutRequest;
use itertools::Itertools;
use snafu::{ensure, OptionExt, ResultExt};
use sqlx::{MySqlPool, PgPool};
use std::fmt::{Debug, Formatter};
use std::time::Instant;

const TEST_KEY_VALUE: &str = "/__stepstone_test";

/// Metasrv component checker
pub struct MetasrvChecker {
    config: MetasrvConfig,
}

impl Debug for MetasrvChecker {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "MetasrvChecker")
    }
}

/// Legacy EtcdChecker for backward compatibility
pub struct EtcdChecker {
    endpoints: String,
    etcd_kv_backend: KvBackendRef,
}

impl Debug for EtcdChecker {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "EtcdChecker")
    }
}

impl MetasrvChecker {
    /// Create a new MetasrvChecker with the given configuration
    pub fn new(config: MetasrvConfig) -> Self {
        Self { config }
    }

    /// Check etcd store using new config format
    async fn check_etcd_new(&self) -> CheckResult {
        let mut details = Vec::new();
        let start = Instant::now();

        // Connect to etcd and test basic operations
        match EtcdStore::with_endpoints(&self.config.store_addrs, 128).await {
            Ok(store) => {
                // Test basic operations immediately to verify real connectivity
                let test_key = format!("{}__stepstone_test", self.config.store_key_prefix.as_deref().unwrap_or(""));
                let test_value = b"stepstone_test_value";

                // PUT operation (this will test real connectivity)
                match store.put(PutRequest {
                    key: test_key.as_bytes().to_vec(),
                    value: test_value.to_vec(),
                    prev_kv: false,
                }).await {
                    Ok(_) => {
                        details.push(CheckDetail::pass(
                            "Etcd Connection".to_string(),
                            format!("Successfully connected to etcd endpoints: {:?}", self.config.store_addrs),
                            Some(start.elapsed()),
                        ));
                        details.push(CheckDetail::pass(
                            "Etcd PUT Operation".to_string(),
                            "PUT operation successful".to_string(),
                            None,
                        ));

                        // GET operation
                        match store.get(test_key.as_bytes()).await {
                            Ok(Some(value)) => {
                                if value.value == test_value {
                                    details.push(CheckDetail::pass(
                                        "Etcd GET Operation".to_string(),
                                        "GET operation successful and data matches".to_string(),
                                        None,
                                    ));
                                } else {
                                    details.push(CheckDetail::fail(
                                        "Etcd GET Operation".to_string(),
                                        "GET operation returned incorrect data".to_string(),
                                        None,
                                        Some("Check etcd data consistency".to_string()),
                                    ));
                                }
                            }
                            Ok(None) => {
                                details.push(CheckDetail::fail(
                                    "Etcd GET Operation".to_string(),
                                    "GET operation returned no data".to_string(),
                                    None,
                                    Some("Check etcd connectivity and data persistence".to_string()),
                                ));
                            }
                            Err(e) => {
                                details.push(CheckDetail::fail(
                                    "Etcd GET Operation".to_string(),
                                    format!("GET operation failed: {}", e),
                                    None,
                                    Some("Check etcd connectivity and permissions".to_string()),
                                ));
                            }
                        }

                        // DELETE operation
                        match store.delete(test_key.as_bytes(), false).await {
                            Ok(_) => {
                                details.push(CheckDetail::pass(
                                    "Etcd DELETE Operation".to_string(),
                                    "DELETE operation successful".to_string(),
                                    None,
                                ));
                            }
                            Err(e) => {
                                details.push(CheckDetail::fail(
                                    "Etcd DELETE Operation".to_string(),
                                    format!("DELETE operation failed: {}", e),
                                    None,
                                    Some("Check etcd permissions".to_string()),
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        details.push(CheckDetail::fail(
                            "Etcd Connection".to_string(),
                            format!("Failed to connect to etcd: {}", e),
                            Some(start.elapsed()),
                            Some("Check etcd service status and network connectivity".to_string()),
                        ));
                    }
                }
            }
            Err(e) => {
                details.push(CheckDetail::fail(
                    "Etcd Connection".to_string(),
                    format!("Failed to connect to etcd: {}", e),
                    Some(start.elapsed()),
                    Some("Check etcd service status and network connectivity".to_string()),
                ));
            }
        }

        CheckResult::from_details(details)
    }

    /// Check PostgreSQL store using new config format
    async fn check_postgres_new(&self) -> CheckResult {
        let mut details = Vec::new();
        let start = Instant::now();

        if let Some(addr) = self.config.store_addrs.first() {
            match PgPool::connect(addr).await {
                Ok(pool) => {
                    details.push(CheckDetail::pass(
                        "PostgreSQL Connection".to_string(),
                        format!("Successfully connected to PostgreSQL: {}", addr),
                        Some(start.elapsed()),
                    ));

                    // Check metadata table
                    let table_name = self.config.meta_table_name.as_deref().unwrap_or("greptime_metasrv");
                    let query = format!(
                        "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = '{}')",
                        table_name
                    );

                    match sqlx::query_scalar::<_, bool>(&query).fetch_one(&pool).await {
                        Ok(exists) => {
                            if exists {
                                details.push(CheckDetail::pass(
                                    "Metadata Table Existence".to_string(),
                                    format!("Table '{}' exists", table_name),
                                    None,
                                ));

                                // Test read/write permissions on existing table
                                self.test_postgres_permissions(&pool, table_name, &mut details).await;
                            } else {
                                details.push(CheckDetail::warning(
                                    "Metadata Table Existence".to_string(),
                                    format!("Table '{}' does not exist, will be created automatically", table_name),
                                    None,
                                    Some("This is normal for first-time setup".to_string()),
                                ));

                                // Test table creation permissions
                                self.test_postgres_create_permissions(&pool, table_name, &mut details).await;
                            }
                        }
                        Err(e) => {
                            details.push(CheckDetail::fail(
                                "Metadata Table Check".to_string(),
                                format!("Failed to check table existence: {}", e),
                                None,
                                Some("Check database permissions and schema access".to_string()),
                            ));
                        }
                    }
                }
                Err(e) => {
                    details.push(CheckDetail::fail(
                        "PostgreSQL Connection".to_string(),
                        format!("Failed to connect to PostgreSQL: {}", e),
                        Some(start.elapsed()),
                        Some("Check connection string, network connectivity, and database availability".to_string()),
                    ));
                }
            }
        } else {
            details.push(CheckDetail::fail(
                "PostgreSQL Configuration".to_string(),
                "No PostgreSQL address configured".to_string(),
                None,
                Some("Add PostgreSQL connection string to store_addrs".to_string()),
            ));
        }

        CheckResult::from_details(details)
    }

    /// Check MySQL store using new config format
    async fn check_mysql_new(&self) -> CheckResult {
        let mut details = Vec::new();
        let start = Instant::now();

        if let Some(addr) = self.config.store_addrs.first() {
            match MySqlPool::connect(addr).await {
                Ok(pool) => {
                    details.push(CheckDetail::pass(
                        "MySQL Connection".to_string(),
                        format!("Successfully connected to MySQL: {}", addr),
                        Some(start.elapsed()),
                    ));

                    // Check metadata table
                    let table_name = self.config.meta_table_name.as_deref().unwrap_or("greptime_metasrv");
                    let query = format!(
                        "SELECT EXISTS (SELECT * FROM information_schema.tables WHERE table_name = '{}')",
                        table_name
                    );

                    match sqlx::query_scalar::<_, bool>(&query).fetch_one(&pool).await {
                        Ok(exists) => {
                            if exists {
                                details.push(CheckDetail::pass(
                                    "Metadata Table Existence".to_string(),
                                    format!("Table '{}' exists", table_name),
                                    None,
                                ));
                            } else {
                                details.push(CheckDetail::warning(
                                    "Metadata Table Existence".to_string(),
                                    format!("Table '{}' does not exist, will be created automatically", table_name),
                                    None,
                                    Some("This is normal for first-time setup".to_string()),
                                ));
                            }
                        }
                        Err(e) => {
                            details.push(CheckDetail::fail(
                                "Metadata Table Check".to_string(),
                                format!("Failed to check table existence: {}", e),
                                None,
                                Some("Check database permissions".to_string()),
                            ));
                        }
                    }
                }
                Err(e) => {
                    details.push(CheckDetail::fail(
                        "MySQL Connection".to_string(),
                        format!("Failed to connect to MySQL: {}", e),
                        Some(start.elapsed()),
                        Some("Check connection string, network connectivity, and database availability".to_string()),
                    ));
                }
            }
        } else {
            details.push(CheckDetail::fail(
                "MySQL Configuration".to_string(),
                "No MySQL address configured".to_string(),
                None,
                Some("Add MySQL connection string to store_addrs".to_string()),
            ));
        }

        CheckResult::from_details(details)
    }

    /// Test PostgreSQL read/write permissions on existing table
    async fn test_postgres_permissions(&self, pool: &PgPool, table_name: &str, details: &mut Vec<CheckDetail>) {
        // Test SELECT permission
        let select_query = format!("SELECT COUNT(*) FROM {}", table_name);
        match sqlx::query_scalar::<_, i64>(&select_query).fetch_one(pool).await {
            Ok(_) => {
                details.push(CheckDetail::pass(
                    "PostgreSQL Read Permission".to_string(),
                    format!("Successfully read from table '{}'", table_name),
                    None,
                ));
            }
            Err(e) => {
                details.push(CheckDetail::fail(
                    "PostgreSQL Read Permission".to_string(),
                    format!("Failed to read from table '{}': {}", table_name, e),
                    None,
                    Some("Grant SELECT permission on the metadata table".to_string()),
                ));
                return; // If we can't read, we probably can't write either
            }
        }

        // Test INSERT permission with a test record
        let test_key = "stepstone_test_key";
        let test_value = "stepstone_test_value";
        let insert_query = format!(
            "INSERT INTO {} (key, value) VALUES ($1, $2) ON CONFLICT (key) DO UPDATE SET value = $2",
            table_name
        );

        match sqlx::query(&insert_query)
            .bind(test_key)
            .bind(test_value)
            .execute(pool)
            .await
        {
            Ok(_) => {
                details.push(CheckDetail::pass(
                    "PostgreSQL Write Permission".to_string(),
                    format!("Successfully wrote to table '{}'", table_name),
                    None,
                ));

                // Clean up test record
                let delete_query = format!("DELETE FROM {} WHERE key = $1", table_name);
                let _ = sqlx::query(&delete_query).bind(test_key).execute(pool).await;
            }
            Err(e) => {
                details.push(CheckDetail::fail(
                    "PostgreSQL Write Permission".to_string(),
                    format!("Failed to write to table '{}': {}", table_name, e),
                    None,
                    Some("Grant INSERT/UPDATE permission on the metadata table".to_string()),
                ));
            }
        }
    }

    /// Test PostgreSQL table creation permissions
    async fn test_postgres_create_permissions(&self, pool: &PgPool, table_name: &str, details: &mut Vec<CheckDetail>) {
        let create_query = format!(
            "CREATE TABLE IF NOT EXISTS {} (
                key VARCHAR(255) PRIMARY KEY,
                value TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            table_name
        );

        match sqlx::query(&create_query).execute(pool).await {
            Ok(_) => {
                details.push(CheckDetail::pass(
                    "PostgreSQL Create Permission".to_string(),
                    format!("Successfully created/verified table '{}'", table_name),
                    None,
                ));

                // Now test read/write on the newly created table
                self.test_postgres_permissions(pool, table_name, details).await;
            }
            Err(e) => {
                details.push(CheckDetail::fail(
                    "PostgreSQL Create Permission".to_string(),
                    format!("Failed to create table '{}': {}", table_name, e),
                    None,
                    Some("Grant CREATE permission on the database/schema".to_string()),
                ));
            }
        }
    }
}

#[async_trait]
impl ComponentChecker for MetasrvChecker {
    async fn check(&self) -> CheckResult {
        match self.config.backend.as_str() {
            "etcd_store" => self.check_etcd_new().await,
            "postgres_store" => self.check_postgres_new().await,
            "mysql_store" => self.check_mysql_new().await,
            "memory_store" => CheckResult::success(
                "Memory store requires no external dependencies".to_string(),
                vec![CheckDetail::pass(
                    "Memory Store".to_string(),
                    "Memory store is always available".to_string(),
                    None,
                )],
            ),
            unknown => CheckResult::failure(
                format!("Unknown store type: {}", unknown),
                vec![CheckDetail::fail(
                    "Store Type".to_string(),
                    format!("Unsupported store type: {}", unknown),
                    None,
                    Some("Use one of: etcd_store, postgres_store, mysql_store, memory_store".to_string()),
                )],
            ),
        }
    }

    fn component_name(&self) -> &'static str {
        "Metasrv"
    }
}

impl EtcdChecker {
    pub async fn try_new<E, S>(endpoints: S) -> error::Result<Self>
    where
        E: AsRef<str>,
        S: AsRef<[E]>,
    {
        let endpoints_str = endpoints
            .as_ref()
            .iter()
            .map(|e| e.as_ref().to_string())
            .join(",");
        let etcd_kv_backend = EtcdStore::with_endpoints(endpoints, usize::MAX).await?;
        Ok(Self {
            endpoints: endpoints_str,
            etcd_kv_backend,
        })
    }

    pub async fn check_put_get(&self) -> error::Result<()> {
        self.etcd_kv_backend
            .put(PutRequest {
                key: TEST_KEY_VALUE.as_bytes().to_vec(),
                value: TEST_KEY_VALUE.as_bytes().to_vec(),
                prev_kv: false,
            })
            .await
            .context(error::EtcdOperationSnafu {
                endpoints: &self.endpoints,
            })?;
        let value = self
            .etcd_kv_backend
            .get(TEST_KEY_VALUE.as_bytes())
            .await
            .context(error::EtcdOperationSnafu {
                endpoints: &self.endpoints,
            })?
            .context(error::EtcdValueMismatchSnafu {
                endpoints: &self.endpoints,
                expect: TEST_KEY_VALUE,
                actual: "None",
            })?;

        ensure!(
            value.value.as_slice() == TEST_KEY_VALUE.as_bytes(),
            error::EtcdValueMismatchSnafu {
                endpoints: &self.endpoints,
                expect: TEST_KEY_VALUE,
                actual: format!("{:?}", value.value.as_slice()),
            }
        );
        self.etcd_kv_backend
            .delete(TEST_KEY_VALUE.as_bytes(), false)
            .await
            .context(error::EtcdOperationSnafu {
                endpoints: &self.endpoints,
            })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::metasrv::EtcdChecker;

    #[tokio::test]
    async fn test_connect_to_etcd_failed() {
        let checker = EtcdChecker::try_new(&["127.0.0.1:2379"]).await.unwrap();
        let result = checker.etcd_kv_backend.get(b"abcd").await;
        assert!(result.is_err());
    }
}