# GreptimeDB 集群自检工具实现方案

## 1. 需求分析

根据需求文档，我们需要开发一个命令行工具 `greptime-self-test`，用于检查 GreptimeDB 集群配置的正确性。该工具需要支持：

### 1.1 基本功能
- **Metasrv 检查**：验证 etcd 或 RDS（PostgreSQL/MySQL）的连接和权限
- **Frontend 检查**：验证与 metasrv 的连接
- **Datanode 检查**：验证与 metasrv 和对象存储的连接及性能

### 1.2 命令行接口
```bash
# Check metasrv config
greptime-self-test metasrv -c metasrv_config.conf

# Check frontend config  
greptime-self-test frontend -c frontend_config.toml

# Check datanode config
greptime-self-test datanode -c datanode_config.toml
```

## 2. 技术架构设计

### 2.1 项目结构
基于现有的 GreptimeDB 代码结构，我们将在 `src/cli` 下新增自检功能：

```
src/cli/src/
├── lib.rs                 # 现有文件，需要添加 self_test 模块
├── self_test/             # 新增自检模块
│   ├── mod.rs            # 模块入口
│   ├── metasrv.rs        # Metasrv 检查逻辑
│   ├── frontend.rs       # Frontend 检查逻辑
│   ├── datanode.rs       # Datanode 检查逻辑
│   ├── common.rs         # 通用检查逻辑
│   └── config.rs         # 配置解析
```

### 2.2 核心组件设计

#### 2.2.1 配置解析器
利用现有的配置解析框架：
- 复用 `common-config` 中的 `Configurable` trait
- 使用现有的 `MetasrvOptions`、`FrontendOptions`、`DatanodeOptions`
- 支持 TOML 格式配置文件解析

#### 2.2.2 检查器接口
```rust
#[async_trait]
pub trait ComponentChecker {
    async fn check(&self) -> CheckResult;
    fn component_name(&self) -> &'static str;
}

pub struct CheckResult {
    pub success: bool,
    pub message: String,
    pub details: Vec<CheckDetail>,
}

pub struct CheckDetail {
    pub item: String,
    pub status: CheckStatus,
    pub message: String,
    pub duration: Option<Duration>,
}

pub enum CheckStatus {
    Pass,
    Fail,
    Warning,
}
```

## 3. 具体实现方案

### 3.1 Metasrv 检查器实现

#### 3.1.1 Etcd 检查
基于现有的 `src/common/meta/src/kv_backend/etcd.rs`：

```rust
pub struct EtcdChecker {
    endpoints: Vec<String>,
    max_txn_ops: usize,
}

impl EtcdChecker {
    async fn check_connection(&self) -> CheckResult {
        // 1. 尝试连接 etcd
        // 2. 执行基本的 put/get/delete 操作
        // 3. 测试事务操作
        // 4. 检查权限
    }
}
```

**检查项目**：
- 连接性测试：尝试连接所有 etcd 端点
- 基本操作测试：put、get、delete 操作
- 事务操作测试：验证 max_txn_ops 限制
- 权限验证：确保有读写权限

#### 3.1.2 PostgreSQL/MySQL 检查
基于现有的数据库连接代码：

```rust
pub struct RdsChecker {
    store_addrs: Vec<String>,
    backend: BackendImpl,
    meta_table_name: String,
    tls_config: Option<TlsOption>,
}

impl RdsChecker {
    async fn check_postgres(&self) -> CheckResult {
        // 1. 解析连接字符串
        // 2. 建立数据库连接
        // 3. 检查表是否存在
        // 4. 测试读写权限
        // 5. 验证 TLS 配置
    }
    
    async fn check_mysql(&self) -> CheckResult {
        // 类似 PostgreSQL 检查逻辑
    }
}
```

**检查项目**：
- 数据库连接测试
- 元数据表存在性检查
- 读写权限验证
- TLS 连接验证（如果配置）

### 3.2 Frontend 检查器实现

```rust
pub struct FrontendChecker {
    meta_client_options: MetaClientOptions,
}

impl FrontendChecker {
    async fn check_metasrv_connection(&self) -> CheckResult {
        // 1. 连接到 metasrv
        // 2. 执行心跳测试
        // 3. 验证基本的元数据操作
    }
}
```

**检查项目**：
- Metasrv 连接性测试
- 心跳机制验证
- 基本元数据查询测试

### 3.3 Datanode 检查器实现

#### 3.3.1 Metasrv 连接检查
复用 Frontend 的检查逻辑

#### 3.3.2 对象存储检查
基于现有的 `src/object-store/src/factory.rs`：

```rust
pub struct ObjectStoreChecker {
    storage_config: StorageConfig,
}

impl ObjectStoreChecker {
    async fn check_s3(&self, config: &S3Config) -> CheckResult {
        // 1. 验证 bucket 存在性
        // 2. 测试读写权限
        // 3. 性能测试（可选）
    }
    
    async fn check_oss(&self, config: &OssConfig) -> CheckResult {
        // 类似 S3 检查
    }
    
    async fn check_azblob(&self, config: &AzblobConfig) -> CheckResult {
        // 类似 S3 检查
    }
    
    async fn check_gcs(&self, config: &GcsConfig) -> CheckResult {
        // 类似 S3 检查
    }
}
```

**检查项目**：
- 对象存储连接测试
- Bucket/Container 存在性验证
- 访问权限测试（读/写/删除）
- 基本性能测试（延迟、吞吐量）

## 4. 命令行接口实现

### 4.1 集成到现有 CLI 系统

修改 `src/plugins/src/cli.rs`：

```rust
#[derive(Parser)]
pub enum SubCommand {
    Bench(BenchTableMetadataCommand),
    #[clap(subcommand)]
    Data(DataCommand),
    #[clap(subcommand)]
    Meta(MetadataCommand),
    #[clap(subcommand)]
    SelfTest(SelfTestCommand),  // 新增
}
```

### 4.2 自检命令定义

```rust
#[derive(Parser)]
pub enum SelfTestCommand {
    /// Check metasrv configuration
    Metasrv {
        /// Configuration file path
        #[clap(short, long)]
        config: String,
        /// Verbose output
        #[clap(short, long)]
        verbose: bool,
    },
    /// Check frontend configuration  
    Frontend {
        #[clap(short, long)]
        config: String,
        #[clap(short, long)]
        verbose: bool,
    },
    /// Check datanode configuration
    Datanode {
        #[clap(short, long)]
        config: String,
        #[clap(short, long)]
        verbose: bool,
        /// Include performance tests
        #[clap(long)]
        include_performance: bool,
    },
}
```

## 5. 错误处理和日志

### 5.1 错误类型定义
```rust
#[derive(Debug, Snafu)]
pub enum SelfTestError {
    #[snafu(display("Failed to load config file: {}", source))]
    ConfigLoad { source: common_config::Error },
    
    #[snafu(display("Connection failed: {}", message))]
    ConnectionFailed { message: String },
    
    #[snafu(display("Permission denied: {}", message))]
    PermissionDenied { message: String },
    
    #[snafu(display("Performance test failed: {}", message))]
    PerformanceTestFailed { message: String },
}
```

### 5.2 日志和输出
- 使用现有的 `common-telemetry` 进行日志记录
- 提供详细的检查报告
- 支持 JSON 格式输出（便于自动化处理）

## 6. 测试策略

### 6.1 单元测试
- 每个检查器的独立测试
- Mock 外部依赖（etcd、数据库、对象存储）
- 配置解析测试

### 6.2 集成测试
- 使用 Docker 容器搭建测试环境
- 测试真实的 etcd、PostgreSQL、MinIO 等服务
- 端到端的配置检查测试

## 7. 实现步骤

### 阶段一：基础框架
1. 创建自检模块结构
2. 实现基本的命令行接口
3. 添加配置解析功能

### 阶段二：核心检查器
1. 实现 Metasrv 检查器（etcd + RDS）
2. 实现 Frontend 检查器
3. 实现 Datanode 检查器（基础功能）

### 阶段三：对象存储支持
1. 实现 S3 检查器
2. 实现 OSS、Azure Blob、GCS 检查器
3. 添加性能测试功能

### 阶段四：完善和优化
1. 添加详细的错误信息和建议
2. 优化输出格式
3. 添加更多检查项目

## 8. 依赖关系

### 8.1 现有依赖
- `clap`: 命令行解析
- `tokio`: 异步运行时
- `serde`: 序列化
- `toml`: 配置文件解析
- `etcd-client`: etcd 客户端
- `sqlx`: 数据库客户端
- `object_store_opendal`: 对象存储客户端

### 8.2 可能需要的新依赖
- `indicatif`: 进度条显示
- `colored`: 彩色输出
- `serde_json`: JSON 输出格式

## 9. 详细代码实现示例

### 9.1 主要文件修改

#### 修改 `src/cli/src/lib.rs`
```rust
// 添加新模块
pub mod self_test;
pub use self_test::SelfTestCommand;
```

#### 修改 `src/plugins/src/cli.rs`
```rust
use cli::{BenchTableMetadataCommand, DataCommand, MetadataCommand, SelfTestCommand, Tool};

#[derive(Parser)]
pub enum SubCommand {
    Bench(BenchTableMetadataCommand),
    #[clap(subcommand)]
    Data(DataCommand),
    #[clap(subcommand)]
    Meta(MetadataCommand),
    #[clap(subcommand)]
    SelfTest(SelfTestCommand),
}

impl SubCommand {
    pub async fn build(&self) -> std::result::Result<Box<dyn Tool>, BoxedError> {
        match self {
            SubCommand::Bench(cmd) => cmd.build().await,
            SubCommand::Data(cmd) => cmd.build().await,
            SubCommand::Meta(cmd) => cmd.build().await,
            SubCommand::SelfTest(cmd) => cmd.build().await,
        }
    }
}
```

### 9.2 核心检查逻辑实现

#### `src/cli/src/self_test/metasrv.rs`
```rust
use async_trait::async_trait;
use common_meta::kv_backend::etcd::EtcdStore;
use common_meta::kv_backend::{KvBackend, RangeRequest};
use meta_srv::metasrv::MetasrvOptions;
use sqlx::{MySqlPool, PgPool};
use std::time::{Duration, Instant};

pub struct MetasrvChecker {
    options: MetasrvOptions,
}

impl MetasrvChecker {
    pub fn new(options: MetasrvOptions) -> Self {
        Self { options }
    }

    async fn check_etcd(&self) -> CheckResult {
        let start = Instant::now();
        let mut details = Vec::new();

        // 连接测试
        match EtcdStore::with_endpoints(&self.options.store_addrs, 128).await {
            Ok(store) => {
                details.push(CheckDetail {
                    item: "Etcd Connection".to_string(),
                    status: CheckStatus::Pass,
                    message: "Successfully connected to etcd".to_string(),
                    duration: Some(start.elapsed()),
                });

                // 基本操作测试
                let test_key = format!("{}__self_test_key", self.options.store_key_prefix);
                let test_value = b"self_test_value";

                // PUT 操作
                if let Err(e) = store.put(test_key.as_bytes(), test_value).await {
                    details.push(CheckDetail {
                        item: "Etcd PUT Operation".to_string(),
                        status: CheckStatus::Fail,
                        message: format!("PUT operation failed: {}", e),
                        duration: None,
                    });
                } else {
                    details.push(CheckDetail {
                        item: "Etcd PUT Operation".to_string(),
                        status: CheckStatus::Pass,
                        message: "PUT operation successful".to_string(),
                        duration: None,
                    });

                    // GET 操作
                    let range_req = RangeRequest::new().with_key(test_key.as_bytes());
                    match store.range(range_req).await {
                        Ok(response) if !response.kvs.is_empty() => {
                            details.push(CheckDetail {
                                item: "Etcd GET Operation".to_string(),
                                status: CheckStatus::Pass,
                                message: "GET operation successful".to_string(),
                                duration: None,
                            });
                        }
                        Ok(_) => {
                            details.push(CheckDetail {
                                item: "Etcd GET Operation".to_string(),
                                status: CheckStatus::Fail,
                                message: "GET operation returned empty result".to_string(),
                                duration: None,
                            });
                        }
                        Err(e) => {
                            details.push(CheckDetail {
                                item: "Etcd GET Operation".to_string(),
                                status: CheckStatus::Fail,
                                message: format!("GET operation failed: {}", e),
                                duration: None,
                            });
                        }
                    }

                    // DELETE 操作
                    if let Err(e) = store.delete(test_key.as_bytes(), false).await {
                        details.push(CheckDetail {
                            item: "Etcd DELETE Operation".to_string(),
                            status: CheckStatus::Fail,
                            message: format!("DELETE operation failed: {}", e),
                            duration: None,
                        });
                    } else {
                        details.push(CheckDetail {
                            item: "Etcd DELETE Operation".to_string(),
                            status: CheckStatus::Pass,
                            message: "DELETE operation successful".to_string(),
                            duration: None,
                        });
                    }
                }
            }
            Err(e) => {
                details.push(CheckDetail {
                    item: "Etcd Connection".to_string(),
                    status: CheckStatus::Fail,
                    message: format!("Failed to connect to etcd: {}", e),
                    duration: Some(start.elapsed()),
                });
            }
        }

        let success = details.iter().all(|d| d.status == CheckStatus::Pass);
        CheckResult {
            success,
            message: if success {
                "All etcd checks passed".to_string()
            } else {
                "Some etcd checks failed".to_string()
            },
            details,
        }
    }

    async fn check_postgres(&self) -> CheckResult {
        let mut details = Vec::new();
        let start = Instant::now();

        if let Some(addr) = self.options.store_addrs.first() {
            match PgPool::connect(addr).await {
                Ok(pool) => {
                    details.push(CheckDetail {
                        item: "PostgreSQL Connection".to_string(),
                        status: CheckStatus::Pass,
                        message: "Successfully connected to PostgreSQL".to_string(),
                        duration: Some(start.elapsed()),
                    });

                    // 检查元数据表
                    let table_name = &self.options.meta_table_name;
                    let query = format!(
                        "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = '{}')",
                        table_name
                    );

                    match sqlx::query_scalar::<_, bool>(&query).fetch_one(&pool).await {
                        Ok(exists) => {
                            if exists {
                                details.push(CheckDetail {
                                    item: "Metadata Table Existence".to_string(),
                                    status: CheckStatus::Pass,
                                    message: format!("Table '{}' exists", table_name),
                                    duration: None,
                                });
                            } else {
                                details.push(CheckDetail {
                                    item: "Metadata Table Existence".to_string(),
                                    status: CheckStatus::Warning,
                                    message: format!("Table '{}' does not exist, will be created automatically", table_name),
                                    duration: None,
                                });
                            }
                        }
                        Err(e) => {
                            details.push(CheckDetail {
                                item: "Metadata Table Check".to_string(),
                                status: CheckStatus::Fail,
                                message: format!("Failed to check table existence: {}", e),
                                duration: None,
                            });
                        }
                    }

                    // 权限测试
                    let test_query = format!(
                        "CREATE TABLE IF NOT EXISTS {}_test (id SERIAL PRIMARY KEY, data TEXT)",
                        table_name
                    );
                    match sqlx::query(&test_query).execute(&pool).await {
                        Ok(_) => {
                            details.push(CheckDetail {
                                item: "Write Permission".to_string(),
                                status: CheckStatus::Pass,
                                message: "Write permission verified".to_string(),
                                duration: None,
                            });

                            // 清理测试表
                            let cleanup_query = format!("DROP TABLE IF EXISTS {}_test", table_name);
                            let _ = sqlx::query(&cleanup_query).execute(&pool).await;
                        }
                        Err(e) => {
                            details.push(CheckDetail {
                                item: "Write Permission".to_string(),
                                status: CheckStatus::Fail,
                                message: format!("Write permission test failed: {}", e),
                                duration: None,
                            });
                        }
                    }
                }
                Err(e) => {
                    details.push(CheckDetail {
                        item: "PostgreSQL Connection".to_string(),
                        status: CheckStatus::Fail,
                        message: format!("Failed to connect to PostgreSQL: {}", e),
                        duration: Some(start.elapsed()),
                    });
                }
            }
        }

        let success = details.iter().all(|d| matches!(d.status, CheckStatus::Pass | CheckStatus::Warning));
        CheckResult {
            success,
            message: if success {
                "PostgreSQL checks completed".to_string()
            } else {
                "PostgreSQL checks failed".to_string()
            },
            details,
        }
    }
}

#[async_trait]
impl ComponentChecker for MetasrvChecker {
    async fn check(&self) -> CheckResult {
        match self.options.backend {
            BackendImpl::EtcdStore => self.check_etcd().await,
            BackendImpl::PostgresStore => self.check_postgres().await,
            BackendImpl::MysqlStore => self.check_mysql().await,
            BackendImpl::MemoryStore => CheckResult {
                success: true,
                message: "Memory store requires no external dependencies".to_string(),
                details: vec![CheckDetail {
                    item: "Memory Store".to_string(),
                    status: CheckStatus::Pass,
                    message: "Memory store is always available".to_string(),
                    duration: None,
                }],
            },
        }
    }

    fn component_name(&self) -> &'static str {
        "Metasrv"
    }
}
```

### 9.3 对象存储检查实现

#### `src/cli/src/self_test/object_store.rs`
```rust
use object_store::{ObjectStore, PutPayload};
use object_store_opendal::OpendalStore;
use opendal::services::{S3, Oss, Azblob, Gcs};
use opendal::Operator;
use std::time::{Duration, Instant};
use uuid::Uuid;

pub struct ObjectStoreChecker {
    config: ObjectStoreConfig,
}

impl ObjectStoreChecker {
    pub fn new(config: ObjectStoreConfig) -> Self {
        Self { config }
    }

    async fn check_s3(&self, s3_config: &S3Config) -> CheckResult {
        let mut details = Vec::new();
        let start = Instant::now();

        // 构建 S3 客户端
        let mut builder = S3::default()
            .root(&s3_config.root)
            .bucket(&s3_config.bucket)
            .access_key_id(s3_config.access_key_id.expose_secret())
            .secret_access_key(s3_config.secret_access_key.expose_secret());

        if let Some(endpoint) = &s3_config.endpoint {
            builder = builder.endpoint(endpoint);
        }
        if let Some(region) = &s3_config.region {
            builder = builder.region(region);
        }

        match Operator::new(builder) {
            Ok(op) => {
                let op = op.finish();
                details.push(CheckDetail {
                    item: "S3 Client Creation".to_string(),
                    status: CheckStatus::Pass,
                    message: "S3 client created successfully".to_string(),
                    duration: Some(start.elapsed()),
                });

                // 测试基本操作
                let test_key = format!("self-test/{}", Uuid::new_v4());
                let test_data = b"self-test-data";

                // PUT 测试
                match op.write(&test_key, test_data).await {
                    Ok(_) => {
                        details.push(CheckDetail {
                            item: "S3 PUT Operation".to_string(),
                            status: CheckStatus::Pass,
                            message: "PUT operation successful".to_string(),
                            duration: None,
                        });

                        // GET 测试
                        match op.read(&test_key).await {
                            Ok(data) => {
                                if data == test_data {
                                    details.push(CheckDetail {
                                        item: "S3 GET Operation".to_string(),
                                        status: CheckStatus::Pass,
                                        message: "GET operation successful".to_string(),
                                        duration: None,
                                    });
                                } else {
                                    details.push(CheckDetail {
                                        item: "S3 GET Operation".to_string(),
                                        status: CheckStatus::Fail,
                                        message: "GET operation returned incorrect data".to_string(),
                                        duration: None,
                                    });
                                }
                            }
                            Err(e) => {
                                details.push(CheckDetail {
                                    item: "S3 GET Operation".to_string(),
                                    status: CheckStatus::Fail,
                                    message: format!("GET operation failed: {}", e),
                                    duration: None,
                                });
                            }
                        }

                        // DELETE 测试
                        match op.delete(&test_key).await {
                            Ok(_) => {
                                details.push(CheckDetail {
                                    item: "S3 DELETE Operation".to_string(),
                                    status: CheckStatus::Pass,
                                    message: "DELETE operation successful".to_string(),
                                    duration: None,
                                });
                            }
                            Err(e) => {
                                details.push(CheckDetail {
                                    item: "S3 DELETE Operation".to_string(),
                                    status: CheckStatus::Fail,
                                    message: format!("DELETE operation failed: {}", e),
                                    duration: None,
                                });
                            }
                        }
                    }
                    Err(e) => {
                        details.push(CheckDetail {
                            item: "S3 PUT Operation".to_string(),
                            status: CheckStatus::Fail,
                            message: format!("PUT operation failed: {}", e),
                            duration: None,
                        });
                    }
                }
            }
            Err(e) => {
                details.push(CheckDetail {
                    item: "S3 Client Creation".to_string(),
                    status: CheckStatus::Fail,
                    message: format!("Failed to create S3 client: {}", e),
                    duration: Some(start.elapsed()),
                });
            }
        }

        let success = details.iter().all(|d| d.status == CheckStatus::Pass);
        CheckResult {
            success,
            message: if success {
                "All S3 checks passed".to_string()
            } else {
                "Some S3 checks failed".to_string()
            },
            details,
        }
    }

    async fn performance_test(&self) -> CheckResult {
        // 性能测试实现
        // 测试延迟和吞吐量
        todo!("Implement performance testing")
    }
}
```

## 10. 输出格式设计

### 10.1 标准输出格式
```
GreptimeDB Self-Test Report
===========================

Component: Metasrv
Configuration: /path/to/metasrv.toml
Backend: etcd_store

✓ Etcd Connection                    [PASS] (125ms) - Successfully connected to etcd
✓ Etcd PUT Operation                 [PASS] - PUT operation successful
✓ Etcd GET Operation                 [PASS] - GET operation successful
✓ Etcd DELETE Operation              [PASS] - DELETE operation successful

Overall Result: PASS (4/4 checks passed)
```

### 10.2 JSON 输出格式
```json
{
  "component": "metasrv",
  "config_file": "/path/to/metasrv.toml",
  "timestamp": "2025-08-22T10:30:00Z",
  "overall_result": "PASS",
  "total_checks": 4,
  "passed_checks": 4,
  "failed_checks": 0,
  "warning_checks": 0,
  "total_duration_ms": 125,
  "details": [
    {
      "item": "Etcd Connection",
      "status": "PASS",
      "message": "Successfully connected to etcd",
      "duration_ms": 125
    }
  ]
}
```

## 11. 部署和使用

### 11.1 编译
```bash
# 编译包含自检功能的 greptime 二进制
cargo build --release --features self-test

# 或者单独编译 CLI 工具
cargo build --release -p cli
```

### 11.2 使用示例
```bash
# 基本检查
greptime cli self-test metasrv -c /etc/greptimedb/metasrv.toml

# 详细输出
greptime cli self-test datanode -c /etc/greptimedb/datanode.toml --verbose

# 包含性能测试
greptime cli self-test datanode -c /etc/greptimedb/datanode.toml --include-performance

# JSON 输出
greptime cli self-test frontend -c /etc/greptimedb/frontend.toml --output json
```

这个实现方案充分利用了 GreptimeDB 现有的代码基础，通过复用配置解析、数据库连接、对象存储等模块，可以快速实现一个功能完整的集群自检工具。整个方案具有良好的可扩展性，可以根据实际需求逐步添加更多的检查项目和功能。
