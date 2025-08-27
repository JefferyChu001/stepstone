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
use crate::config::{MetasrvConfig, StoreConfig};
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

    /// Check etcd store connectivity and operations
    async fn check_etcd(&self, store_config: &StoreConfig) -> CheckResult {
        let start = Instant::now();
        let mut details = Vec::new();

        // Connect to etcd
        match EtcdStore::with_endpoints(&store_config.store_addrs, store_config.max_txn_ops.unwrap_or(128)).await {
            Ok(store) => {
                details.push(CheckDetail::pass(
                    "Etcd Connection".to_string(),
                    format!("Successfully connected to etcd endpoints: {:?}", store_config.store_addrs),
                    Some(start.elapsed()),
                ));

                // Test basic operations
                let test_key = format!("{}__stepstone_test", store_config.store_key_prefix.as_deref().unwrap_or(""));
                let test_value = b"stepstone_test_value";

                // PUT operation
                match store.put(PutRequest {
                    key: test_key.as_bytes().to_vec(),
                    value: test_value.to_vec(),
                    prev_kv: false,
                }).await {
                    Ok(_) => {
                        details.push(CheckDetail::pass(
                            "Etcd PUT Operation".to_string(),
                            "PUT operation successful".to_string(),
                            None,
                        ));

                        // GET operation with retry
                        for attempt in 1..=3 {
                            // Small delay to ensure PUT operation is committed
                            if attempt > 1 {
                                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                            }

                            match store.get(test_key.as_bytes()).await {
                                Ok(Some(value)) => {
                                    if value.value == test_value {
                                        details.push(CheckDetail::pass(
                                            "Etcd GET Operation".to_string(),
                                            format!("GET operation successful and data matches (attempt {})", attempt),
                                            None,
                                        ));
                                        break;
                                    } else {
                                        if attempt == 3 {
                                            details.push(CheckDetail::fail(
                                                "Etcd GET Operation".to_string(),
                                                format!("GET operation returned incorrect data after {} attempts", attempt),
                                                None,
                                                Some("Check etcd data consistency".to_string()),
                                            ));
                                        }
                                    }
                                }
                                Ok(None) => {
                                    if attempt == 3 {
                                        details.push(CheckDetail::fail(
                                            "Etcd GET Operation".to_string(),
                                            format!("GET operation returned no data after {} attempts", attempt),
                                            None,
                                            Some("Check etcd connectivity and permissions".to_string()),
                                        ));
                                    }
                                }
                                Err(e) => {
                                    if attempt == 3 {
                                        details.push(CheckDetail::fail(
                                            "Etcd GET Operation".to_string(),
                                            format!("GET operation failed after {} attempts: {}", attempt, e),
                                            None,
                                            Some("Check etcd connectivity and permissions".to_string()),
                                        ));
                                    }
                                }
                            }
                        }

                        // DELETE operation (cleanup)
                        match store.delete(test_key.as_bytes(), false).await {
                            Ok(_) => {
                                details.push(CheckDetail::pass(
                                    "Etcd DELETE Operation".to_string(),
                                    "DELETE operation successful".to_string(),
                                    None,
                                ));
                            }
                            Err(e) => {
                                details.push(CheckDetail::warning(
                                    "Etcd DELETE Operation".to_string(),
                                    format!("DELETE operation failed: {}", e),
                                    None,
                                    Some("Test key may remain in etcd, but this doesn't affect functionality".to_string()),
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        details.push(CheckDetail::fail(
                            "Etcd PUT Operation".to_string(),
                            format!("PUT operation failed: {}", e),
                            None,
                            Some("Check etcd connectivity and write permissions".to_string()),
                        ));
                    }
                }
            }
            Err(e) => {
                details.push(CheckDetail::fail(
                    "Etcd Connection".to_string(),
                    format!("Failed to connect to etcd: {}", e),
                    Some(start.elapsed()),
                    Some("Check etcd endpoints and network connectivity".to_string()),
                ));
            }
        }

        CheckResult::from_details(details)
    }

    /// Check PostgreSQL store connectivity and operations
    async fn check_postgres(&self, store_config: &StoreConfig) -> CheckResult {
        let mut details = Vec::new();
        let start = Instant::now();

        if let Some(addr) = store_config.store_addrs.first() {
            match PgPool::connect(addr).await {
                Ok(pool) => {
                    details.push(CheckDetail::pass(
                        "PostgreSQL Connection".to_string(),
                        format!("Successfully connected to PostgreSQL: {}", addr),
                        Some(start.elapsed()),
                    ));

                    // Check metadata table
                    let table_name = store_config.meta_table_name.as_deref().unwrap_or("greptime_metasrv");
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
                                Some("Check database permissions and connectivity".to_string()),
                            ));
                        }
                    }

                    // Test write permissions
                    let test_table = format!("{}_stepstone_test", table_name);
                    let create_query = format!(
                        "CREATE TABLE IF NOT EXISTS {} (id SERIAL PRIMARY KEY, data TEXT)",
                        test_table
                    );

                    match sqlx::query(&create_query).execute(&pool).await {
                        Ok(_) => {
                            details.push(CheckDetail::pass(
                                "Write Permission".to_string(),
                                "Write permission verified".to_string(),
                                None,
                            ));

                            // Cleanup test table
                            let cleanup_query = format!("DROP TABLE IF EXISTS {}", test_table);
                            let _ = sqlx::query(&cleanup_query).execute(&pool).await;
                        }
                        Err(e) => {
                            details.push(CheckDetail::fail(
                                "Write Permission".to_string(),
                                format!("Write permission test failed: {}", e),
                                None,
                                Some("Check database user permissions".to_string()),
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

    /// Check MySQL store connectivity and operations
    async fn check_mysql(&self, store_config: &StoreConfig) -> CheckResult {
        let mut details = Vec::new();
        let start = Instant::now();

        if let Some(addr) = store_config.store_addrs.first() {
            match MySqlPool::connect(addr).await {
                Ok(pool) => {
                    details.push(CheckDetail::pass(
                        "MySQL Connection".to_string(),
                        format!("Successfully connected to MySQL: {}", addr),
                        Some(start.elapsed()),
                    ));

                    // Check metadata table
                    let table_name = store_config.meta_table_name.as_deref().unwrap_or("greptime_metasrv");
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
                                Some("Check database permissions and connectivity".to_string()),
                            ));
                        }
                    }

                    // Test write permissions
                    let test_table = format!("{}_stepstone_test", table_name);
                    let create_query = format!(
                        "CREATE TABLE IF NOT EXISTS {} (id INT AUTO_INCREMENT PRIMARY KEY, data TEXT)",
                        test_table
                    );

                    match sqlx::query(&create_query).execute(&pool).await {
                        Ok(_) => {
                            details.push(CheckDetail::pass(
                                "Write Permission".to_string(),
                                "Write permission verified".to_string(),
                                None,
                            ));

                            // Cleanup test table
                            let cleanup_query = format!("DROP TABLE IF EXISTS {}", test_table);
                            let _ = sqlx::query(&cleanup_query).execute(&pool).await;
                        }
                        Err(e) => {
                            details.push(CheckDetail::fail(
                                "Write Permission".to_string(),
                                format!("Write permission test failed: {}", e),
                                None,
                                Some("Check database user permissions".to_string()),
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
}

#[async_trait]
impl ComponentChecker for MetasrvChecker {
    async fn check(&self) -> CheckResult {
        match self.config.store.store_type.as_str() {
            "etcd_store" => self.check_etcd(&self.config.store).await,
            "postgres_store" => self.check_postgres(&self.config.store).await,
            "mysql_store" => self.check_mysql(&self.config.store).await,
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