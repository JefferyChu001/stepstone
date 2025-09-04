# GreptimeDB 集群自检工具技术实现详解

## 目录
1. [项目架构概览](#项目架构概览)
2. [核心设计模式](#核心设计模式)
3. [文件结构详解](#文件结构详解)
4. [实现细节分析](#实现细节分析)
5. [性能测试实现](#性能测试实现)
6. [错误处理机制](#错误处理机制)
7. [扩展开发指南](#扩展开发指南)

## 项目架构概览

### 整体架构图

```
┌─────────────────────────────────────────────────────────────────┐
│                        Stepstone CLI Tool                       │
├─────────────────────────────────────────────────────────────────┤
│  main.rs (CLI 入口) → lib.rs (核心逻辑) → 各组件检查器          │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │
│  │ metasrv.rs  │  │frontend.rs  │  │datanode.rs  │              │
│  │             │  │             │  │             │              │
│  │ • etcd      │  │ • metasrv   │  │ • metasrv   │              │
│  │ • postgres  │  │   连接测试  │  │   连接测试  │              │
│  │   读写测试  │  │ • HTTP配置  │  │ • S3性能测试│              │
│  └─────────────┘  │   验证      │  │ • 文件权限  │              │
│                   └─────────────┘  │   测试      │              │
│                                    └─────────────┘              │
├─────────────────────────────────────────────────────────────────┤
│                    common.rs (共享组件)                         │
│  • CheckResult/CheckDetail 数据结构                            │
│  • JSON/文本输出格式化                                         │
│  • 错误处理和建议生成                                           │
└─────────────────────────────────────────────────────────────────┘
```

### 核心设计理念

1. **模块化设计**: 每个 GreptimeDB 组件对应一个独立的检查器模块
2. **异步架构**: 使用 Tokio 异步运行时处理网络 I/O 和并发操作
3. **统一接口**: 通过 `ComponentChecker` trait 提供一致的检查接口
4. **详细报告**: 提供人类可读和机器可读的检查结果
5. **性能基准**: 内置存储性能测试，提供实际的吞吐量和延迟数据

## 核心设计模式

### 1. Strategy Pattern (策略模式)

通过 `ComponentChecker` trait 实现不同组件的检查策略：

```rust
#[async_trait]
pub trait ComponentChecker {
    async fn check(&self) -> CheckResult;
    fn component_name(&self) -> &'static str;
}
```

### 2. Builder Pattern (构建者模式)

`CheckDetail` 使用构建者模式创建不同类型的检查结果：

```rust
impl CheckDetail {
    pub fn pass(item: String, message: String, duration: Option<Duration>) -> Self
    pub fn fail(item: String, message: String, duration: Option<Duration>, suggestion: Option<String>) -> Self
    pub fn warning(item: String, message: String, duration: Option<Duration>, suggestion: Option<String>) -> Self
}
```

### 3. Factory Pattern (工厂模式)

根据配置文件类型创建相应的检查器实例。

## 文件结构详解

### 1. `main.rs` - CLI 入口点

**职责**: 命令行参数解析和程序入口

**关键技术点**:
- 使用 `clap` crate 进行命令行参数解析
- 支持子命令模式 (`metasrv`, `frontend`, `datanode`)
- 异步 main 函数使用 `#[tokio::main]`

**核心代码结构**:
```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Metasrv { config, output } => {
            // 创建 MetasrvChecker 并执行检查
        }
        Commands::Frontend { config, output } => {
            // 创建 FrontendChecker 并执行检查
        }
        Commands::Datanode { config, output } => {
            // 创建 DatanodeChecker 并执行检查
        }
    }
}
```

### 2. `lib.rs` - 核心库入口

**职责**: 模块声明和公共接口导出

**技术细节**:
- 声明所有子模块 (`pub mod common`, `pub mod metasrv`, 等)
- 重新导出核心类型供外部使用
- 设置全局的编译器配置

### 3. `common.rs` - 共享数据结构和工具

**职责**: 定义通用的数据结构和输出格式化

**核心数据结构**:

#### `CheckDetail` - 单个检查项的结果
```rust
#[derive(Debug, Clone, Serialize)]
pub struct CheckDetail {
    pub item: String,           // 检查项名称
    pub status: CheckStatus,    // 状态: PASS/FAIL/WARNING
    pub message: String,        // 详细消息
    pub duration_ms: Option<u64>, // 执行时间(毫秒)
    pub suggestion: Option<String>, // 失败时的建议
}
```

#### `CheckResult` - 整体检查结果
```rust
#[derive(Debug, Serialize)]
pub struct CheckResult {
    pub component: String,      // 组件名称
    pub config_file: String,    // 配置文件路径
    pub timestamp: DateTime<Utc>, // 检查时间戳
    pub overall_result: CheckStatus, // 整体结果
    pub total_checks: usize,    // 总检查项数
    pub passed_checks: usize,   // 通过的检查项数
    pub failed_checks: usize,   // 失败的检查项数
    pub warning_checks: usize,  // 警告的检查项数
    pub total_duration_ms: u64, // 总执行时间
    pub message: String,        // 总结消息
    pub details: Vec<CheckDetail>, // 详细检查结果
}
```

**输出格式化**:
- `format_human_readable()`: 生成彩色的人类可读输出
- `format_json()`: 生成结构化的 JSON 输出
- 使用 `colored` crate 提供彩色终端输出

### 4. `metasrv.rs` - Metasrv 组件检查器

**职责**: 检查 Metasrv 的元数据存储后端连接和操作

#### 支持的后端类型:
1. **etcd**: 分布式键值存储
2. **PostgreSQL**: 关系数据库

#### 技术实现细节:

##### etcd 检查实现:
```rust
async fn check_etcd_store(&self, details: &mut Vec<CheckDetail>) {
    // 1. 连接测试
    let start = Instant::now();
    let client = match etcd_client::Client::connect(&self.config.store_addrs, None).await {
        Ok(client) => {
            let duration = start.elapsed();
            details.push(CheckDetail::pass(
                "Etcd Connection".to_string(),
                format!("Successfully connected to etcd endpoints: {:?}", self.config.store_addrs),
                Some(duration),
            ));
            client
        }
        Err(e) => {
            details.push(CheckDetail::fail(
                "Etcd Connection".to_string(),
                format!("Failed to connect to etcd: {}", e),
                Some(start.elapsed()),
                Some("Check etcd service status and network connectivity".to_string()),
            ));
            return;
        }
    };

    // 2. PUT 操作测试
    let test_key = "stepstone-test-key";
    let test_value = "stepstone-test-value";
    
    match client.put(test_key, test_value, None).await {
        Ok(_) => {
            details.push(CheckDetail::pass(
                "Etcd PUT Operation".to_string(),
                "PUT operation successful".to_string(),
                None,
            ));
        }
        Err(e) => {
            details.push(CheckDetail::fail(
                "Etcd PUT Operation".to_string(),
                format!("PUT operation failed: {}", e),
                None,
                Some("Check etcd write permissions and storage space".to_string()),
            ));
            return;
        }
    }

    // 3. GET 操作测试
    match client.get(test_key, None).await {
        Ok(resp) => {
            if let Some(kv) = resp.kvs().first() {
                let retrieved_value = std::str::from_utf8(kv.value()).unwrap_or("");
                if retrieved_value == test_value {
                    details.push(CheckDetail::pass(
                        "Etcd GET Operation".to_string(),
                        "GET operation successful and data matches".to_string(),
                        None,
                    ));
                } else {
                    details.push(CheckDetail::fail(
                        "Etcd GET Operation".to_string(),
                        "GET operation successful but data doesn't match".to_string(),
                        None,
                        Some("Check etcd data consistency".to_string()),
                    ));
                }
            } else {
                details.push(CheckDetail::fail(
                    "Etcd GET Operation".to_string(),
                    "GET operation returned empty result".to_string(),
                    None,
                    Some("Check etcd read permissions".to_string()),
                ));
            }
        }
        Err(e) => {
            details.push(CheckDetail::fail(
                "Etcd GET Operation".to_string(),
                format!("GET operation failed: {}", e),
                None,
                Some("Check etcd read permissions and connectivity".to_string()),
            ));
        }
    }

    // 4. DELETE 操作测试
    match client.delete(test_key, None).await {
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
                Some("Check etcd delete permissions".to_string()),
            ));
        }
    }
}
```

##### PostgreSQL 检查实现:
```rust
async fn check_postgres_store(&self, details: &mut Vec<CheckDetail>) {
    // 1. 连接池创建和连接测试
    let start = Instant::now();
    let pool = match PgPoolOptions::new()
        .max_connections(5)
        .connect(&self.config.store_addrs[0])
        .await
    {
        Ok(pool) => {
            details.push(CheckDetail::pass(
                "PostgreSQL Connection".to_string(),
                format!("Successfully connected to PostgreSQL: {}", self.config.store_addrs[0]),
                Some(start.elapsed()),
            ));
            pool
        }
        Err(e) => {
            details.push(CheckDetail::fail(
                "PostgreSQL Connection".to_string(),
                format!("Failed to connect to PostgreSQL: {}", e),
                Some(start.elapsed()),
                Some("Check PostgreSQL service status, credentials, and network connectivity".to_string()),
            ));
            return;
        }
    };

    // 2. 表存在性检查
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
                // 测试现有表的读写权限
                self.test_postgres_permissions(&pool, table_name, details).await;
            } else {
                details.push(CheckDetail::warning(
                    "Metadata Table Existence".to_string(),
                    format!("Table '{}' does not exist, will be created automatically", table_name),
                    None,
                    Some("This is normal for first-time setup".to_string()),
                ));
                // 测试表创建权限
                self.test_postgres_create_permissions(&pool, table_name, details).await;
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
```

#### 权限测试实现:
```rust
async fn test_postgres_permissions(&self, pool: &PgPool, table_name: &str, details: &mut Vec<CheckDetail>) {
    // 读权限测试
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
            return;
        }
    }

    // 写权限测试
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
            // 清理测试记录
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
```

### 5. `frontend.rs` - Frontend 组件检查器

**职责**: 检查 Frontend 的 Metasrv 连接和服务器配置

#### 技术实现:

##### Metasrv 连接测试:
```rust
async fn check_metasrv_connectivity(&self, details: &mut Vec<CheckDetail>) {
    for (index, addr) in self.config.meta_client.metasrv_addrs.iter().enumerate() {
        let start = Instant::now();
        
        match TcpStream::connect(addr).await {
            Ok(_) => {
                details.push(CheckDetail::pass(
                    format!("Metasrv Connectivity {}", index + 1),
                    format!("Successfully connected to metasrv at {}", addr),
                    Some(start.elapsed()),
                ));
            }
            Err(e) => {
                details.push(CheckDetail::fail(
                    format!("Metasrv Connectivity {}", index + 1),
                    format!("Failed to connect to metasrv at {}: {}", addr, e),
                    Some(start.elapsed()),
                    Some("Check metasrv service status and network connectivity".to_string()),
                ));
            }
        }
    }
}
```

##### HTTP 服务器配置验证:
```rust
fn check_http_server_config(&self, details: &mut Vec<CheckDetail>) {
    match self.config.http.addr.parse::<SocketAddr>() {
        Ok(_) => {
            details.push(CheckDetail::pass(
                "HTTP Server Address Configuration".to_string(),
                format!("HTTP server address '{}' is valid", self.config.http.addr),
                None,
            ));
        }
        Err(e) => {
            details.push(CheckDetail::fail(
                "HTTP Server Address Configuration".to_string(),
                format!("Invalid HTTP server address '{}': {}", self.config.http.addr, e),
                None,
                Some("Check the HTTP server address format in configuration".to_string()),
            ));
        }
    }
}
```

### 6. `datanode.rs` - Datanode 组件检查器

**职责**: 检查 Datanode 的存储后端和性能

#### 支持的存储类型:
1. **S3**: 对象存储 (Amazon S3, MinIO, 等)
2. **File**: 本地文件系统存储

#### S3 存储检查实现:

##### 客户端创建和权限验证:
```rust
async fn check_s3_storage(&self, details: &mut Vec<CheckDetail>) {
    // 1. S3 客户端创建
    let start = Instant::now();
    let mut builder = S3::default();
    
    // 配置 S3 参数
    builder
        .bucket(&self.config.storage.bucket.as_ref().unwrap())
        .region(&self.config.storage.region.as_ref().unwrap())
        .access_key_id(&self.config.storage.access_key_id.as_ref().unwrap())
        .secret_access_key(&self.config.storage.secret_access_key.as_ref().unwrap());

    if let Some(endpoint) = &self.config.storage.endpoint {
        builder.endpoint(endpoint);
    }

    let op = match Operator::new(builder) {
        Ok(op) => match op.finish() {
            Ok(op) => {
                details.push(CheckDetail::pass(
                    "S3 Client Creation".to_string(),
                    "S3 client created successfully".to_string(),
                    Some(start.elapsed()),
                ));
                op
            }
            Err(e) => {
                details.push(CheckDetail::fail(
                    "S3 Client Creation".to_string(),
                    format!("Failed to initialize S3 client: {}", e),
                    Some(start.elapsed()),
                    Some("Check S3 configuration parameters".to_string()),
                ));
                return;
            }
        }
        Err(e) => {
            details.push(CheckDetail::fail(
                "S3 Client Creation".to_string(),
                format!("Failed to create S3 client: {}", e),
                Some(start.elapsed()),
                Some("Check S3 configuration parameters".to_string()),
            ));
            return;
        }
    };

    // 2. 权限和基本操作测试
    self.test_s3_bucket_permissions(&op, details).await;
    
    // 3. 基本 CRUD 操作测试
    let test_key = format!("stepstone-test/{}", Uuid::new_v4());
    let test_data = b"stepstone-test-data";

    // PUT 测试
    match op.write(&test_key, test_data.as_slice()).await {
        Ok(_) => {
            details.push(CheckDetail::pass(
                "S3 PUT Operation".to_string(),
                "PUT operation successful".to_string(),
                None,
            ));

            // GET 测试
            match op.read(&test_key).await {
                Ok(data) => {
                    if data == test_data {
                        details.push(CheckDetail::pass(
                            "S3 GET Operation".to_string(),
                            "GET operation successful and data matches".to_string(),
                            None,
                        ));
                    } else {
                        details.push(CheckDetail::fail(
                            "S3 GET Operation".to_string(),
                            "GET operation successful but data doesn't match".to_string(),
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
                        Some("Check S3 read permissions and connectivity".to_string()),
                    ));
                }
            }

            // DELETE 测试
            match op.delete(&test_key).await {
                Ok(_) => {
                    details.push(CheckDetail::pass(
                        "S3 DELETE Operation".to_string(),
                        "DELETE operation successful".to_string(),
                        None,
                    ));

                    // 性能测试
                    self.test_s3_performance(&op, details).await;
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
        }
        Err(e) => {
            let error_msg = format!("{}", e);
            if error_msg.contains("InvalidAccessKeyId") {
                details.push(CheckDetail::fail(
                    "S3 PUT Operation".to_string(),
                    format!("Invalid access key: {}", e),
                    None,
                    Some("Check the access_key_id in configuration".to_string()),
                ));
            } else if error_msg.contains("NoSuchBucket") {
                details.push(CheckDetail::fail(
                    "S3 PUT Operation".to_string(),
                    format!("Bucket does not exist: {}", e),
                    None,
                    Some("Create the bucket or check the bucket name in configuration".to_string()),
                ));
            } else {
                details.push(CheckDetail::fail(
                    "S3 PUT Operation".to_string(),
                    format!("PUT operation failed: {}", e),
                    None,
                    Some("Check S3 permissions and connectivity".to_string()),
                ));
            }
        }
    }
}
```

## 性能测试实现

### S3 性能测试架构

性能测试分为三个层次，每个层次测试不同的使用场景：

#### 1. 中等文件性能测试 (64MB)
```rust
async fn test_s3_performance(&self, op: &opendal::Operator, details: &mut Vec<CheckDetail>) {
    use std::time::Instant;
    use tokio::time::{timeout, Duration};

    // 64MB 文件写入性能测试
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

            // 读取性能测试
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
                Err(e) => {
                    details.push(CheckDetail::warning(
                        "S3 64MB File Read Performance".to_string(),
                        format!("Read test failed: {}", e),
                        None,
                        Some("Performance test incomplete".to_string()),
                    ));
                }
            }

            // 清理测试文件
            let _ = op.delete(small_key).await;
        }
        Err(e) => {
            details.push(CheckDetail::warning(
                "S3 64MB File Write Performance".to_string(),
                format!("Write test failed or timed out: {}", e),
                None,
                Some("Check S3 performance and network bandwidth".to_string()),
            ));
        }
    }
}
```

#### 2. 大文件性能测试 (1GB)
```rust
// 1GB 文件写入性能测试 - 测试大文件处理能力
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

        // 清理大文件
        let _ = op.delete(large_key).await;
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
```

#### 3. 并发操作性能测试 (100 并发)
```rust
async fn test_s3_concurrent_performance(&self, op: &opendal::Operator, details: &mut Vec<CheckDetail>) {
    let concurrent_count = 100;
    let data = vec![0u8; 512]; // 每个操作 512 字节

    let start = Instant::now();
    let mut handles = Vec::new();

    // 创建 100 个并发写入任务
    for i in 0..concurrent_count {
        let op_clone = op.clone();
        let data_clone = data.clone();
        let key = format!("stepstone_concurrent_test_{}", i);
        let key_clone = key.clone();

        // 使用 tokio::spawn 创建并发任务
        let handle = tokio::spawn(async move {
            op_clone.write(&key_clone, data_clone).await
        });
        handles.push((handle, key));
    }

    // 等待所有任务完成并统计结果
    let mut successful_ops = 0;
    let mut keys_to_cleanup = Vec::new();

    for (handle, key) in handles {
        match timeout(Duration::from_secs(10), handle).await {
            Ok(Ok(Ok(_))) => {
                successful_ops += 1;
                keys_to_cleanup.push(key);
            }
            _ => {} // 失败或超时
        }
    }

    let total_duration = start.elapsed();
    let ops_per_second = successful_ops as f64 / total_duration.as_secs_f64();

    // 生成性能报告
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

    // 清理测试文件
    for key in keys_to_cleanup {
        let _ = op.delete(&key).await;
    }
}
```

## 错误处理机制

### 分层错误处理架构

工具采用分层的错误处理机制，每一层都有特定的职责：

#### 1. 网络层错误处理
```rust
// 连接超时处理
match timeout(Duration::from_secs(30), TcpStream::connect(addr)).await {
    Ok(Ok(_)) => {
        // 连接成功
    }
    Ok(Err(e)) => {
        // 连接失败 - 可能是服务未启动、端口被占用等
        let suggestion = match e.kind() {
            std::io::ErrorKind::ConnectionRefused => {
                "Service is not running or port is not accessible"
            }
            std::io::ErrorKind::TimedOut => {
                "Network timeout - check firewall and network connectivity"
            }
            std::io::ErrorKind::PermissionDenied => {
                "Permission denied - check user privileges"
            }
            _ => "Check network connectivity and service status"
        };

        details.push(CheckDetail::fail(
            "Network Connection".to_string(),
            format!("Connection failed: {}", e),
            None,
            Some(suggestion.to_string()),
        ));
    }
    Err(_) => {
        // 超时
        details.push(CheckDetail::fail(
            "Network Connection".to_string(),
            "Connection timed out".to_string(),
            None,
            Some("Check network connectivity and increase timeout if needed".to_string()),
        ));
    }
}
```

#### 2. 认证层错误处理
```rust
// S3 认证错误的详细分类
fn handle_s3_auth_error(error: &str) -> (String, String) {
    match error {
        e if e.contains("InvalidAccessKeyId") => (
            "S3 Access Key Validation".to_string(),
            "Check the access_key_id in configuration".to_string()
        ),
        e if e.contains("SignatureDoesNotMatch") => (
            "S3 Secret Key Validation".to_string(),
            "Check the secret_access_key in configuration".to_string()
        ),
        e if e.contains("AccessDenied") => (
            "S3 Permission Denied".to_string(),
            "Check IAM policies and bucket permissions".to_string()
        ),
        e if e.contains("NoSuchBucket") => (
            "S3 Bucket Existence".to_string(),
            "Create the bucket or check the bucket name in configuration".to_string()
        ),
        _ => (
            "S3 Authentication Error".to_string(),
            "Check S3 configuration and credentials".to_string()
        )
    }
}
```

#### 3. 应用层错误处理
```rust
// PostgreSQL 权限错误的详细分类
fn handle_postgres_error(error: &sqlx::Error) -> (String, String) {
    match error {
        sqlx::Error::Database(db_err) => {
            let code = db_err.code().unwrap_or_default();
            match code.as_ref() {
                "42501" => ( // insufficient_privilege
                    "PostgreSQL Permission Error".to_string(),
                    "Grant necessary permissions to the database user".to_string()
                ),
                "42P01" => ( // undefined_table
                    "PostgreSQL Table Missing".to_string(),
                    "Create the metadata table or grant CREATE permission".to_string()
                ),
                "28P01" => ( // invalid_password
                    "PostgreSQL Authentication Failed".to_string(),
                    "Check username and password in connection string".to_string()
                ),
                _ => (
                    "PostgreSQL Database Error".to_string(),
                    format!("Database error code: {}", code)
                )
            }
        }
        sqlx::Error::Io(io_err) => (
            "PostgreSQL Connection Error".to_string(),
            format!("Network error: {}", io_err)
        ),
        _ => (
            "PostgreSQL Unknown Error".to_string(),
            "Check PostgreSQL logs for more details".to_string()
        )
    }
}
```

## 配置解析和验证

### TOML 配置结构设计

#### 使用 serde 的高级特性:
```rust
#[derive(Debug, Deserialize)]
pub struct MetasrvConfig {
    pub data_home: Option<String>,

    #[serde(default)]
    pub store_addrs: Vec<String>,

    #[serde(default)]
    pub store_key_prefix: String,

    #[serde(rename = "backend")]
    pub backend_type: String,

    // 条件字段 - 只在特定后端时使用
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta_table_name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta_schema_name: Option<String>,

    // 嵌套配置结构
    pub grpc: Option<GrpcConfig>,
    pub http: Option<HttpConfig>,
}

#[derive(Debug, Deserialize)]
pub struct GrpcConfig {
    pub bind_addr: String,

    #[serde(default = "default_server_addr")]
    pub server_addr: String,

    #[serde(default = "default_runtime_size")]
    pub runtime_size: usize,
}

// 默认值函数
fn default_server_addr() -> String {
    "127.0.0.1:3002".to_string()
}

fn default_runtime_size() -> usize {
    8
}
```

#### 配置验证实现:
```rust
impl MetasrvConfig {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // 1. 验证必需字段
        if self.store_addrs.is_empty() {
            errors.push("store_addrs cannot be empty".to_string());
        }

        // 2. 验证后端类型
        match self.backend_type.as_str() {
            "etcd_store" => self.validate_etcd_config(&mut errors),
            "postgres_store" => self.validate_postgres_config(&mut errors),
            "mysql_store" => self.validate_mysql_config(&mut errors),
            "memory_store" => {}, // 内存存储无需额外验证
            _ => errors.push(format!("Unsupported backend: {}", self.backend_type)),
        }

        // 3. 验证网络地址格式
        if let Some(grpc) = &self.grpc {
            if let Err(e) = grpc.bind_addr.parse::<SocketAddr>() {
                errors.push(format!("Invalid gRPC bind address: {}", e));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn validate_etcd_config(&self, errors: &mut Vec<String>) {
        for addr in &self.store_addrs {
            if !addr.contains(':') {
                errors.push(format!("etcd address '{}' should include port", addr));
            }
        }
    }

    fn validate_postgres_config(&self, errors: &mut Vec<String>) {
        for addr in &self.store_addrs {
            if !addr.starts_with("postgresql://") {
                errors.push(format!("PostgreSQL address '{}' should start with 'postgresql://'", addr));
            }
        }

        if self.meta_table_name.is_none() {
            errors.push("meta_table_name is required for PostgreSQL backend".to_string());
        }
    }
}
```

## 异步编程模式深度解析

### 1. async/await 模式使用

#### 异步 trait 实现:
```rust
// 使用 async-trait crate 支持 trait 中的异步方法
#[async_trait]
pub trait ComponentChecker {
    async fn check(&self) -> CheckResult;
    fn component_name(&self) -> &'static str;
}

// 实现异步 trait
#[async_trait]
impl ComponentChecker for MetasrvChecker {
    async fn check(&self) -> CheckResult {
        let mut details = Vec::new();

        // 根据后端类型选择检查方法
        match self.config.backend_type.as_str() {
            "etcd_store" => self.check_etcd_store(&mut details).await,
            "postgres_store" => self.check_postgres_store(&mut details).await,
            _ => {
                details.push(CheckDetail::fail(
                    "Backend Type".to_string(),
                    format!("Unsupported backend: {}", self.config.backend_type),
                    None,
                    Some("Use 'etcd_store' or 'postgres_store'".to_string()),
                ));
            }
        }

        CheckResult::from_details(details)
    }

    fn component_name(&self) -> &'static str {
        "Metasrv"
    }
}
```

### 2. 并发控制模式

#### 使用 tokio::spawn 进行并发:
```rust
// 并发执行多个检查任务
let mut handles = Vec::new();

// 为每个 metasrv 地址创建并发连接测试
for addr in &self.config.meta_client.metasrv_addrs {
    let addr_clone = addr.clone();
    let handle = tokio::spawn(async move {
        TcpStream::connect(&addr_clone).await
    });
    handles.push((handle, addr));
}

// 收集所有结果
for (handle, addr) in handles {
    match handle.await {
        Ok(Ok(_)) => {
            details.push(CheckDetail::pass(
                format!("Metasrv Connectivity {}", addr),
                "Connection successful".to_string(),
                None,
            ));
        }
        Ok(Err(e)) => {
            details.push(CheckDetail::fail(
                format!("Metasrv Connectivity {}", addr),
                format!("Connection failed: {}", e),
                None,
                Some("Check service status".to_string()),
            ));
        }
        Err(e) => {
            details.push(CheckDetail::fail(
                format!("Metasrv Connectivity {}", addr),
                format!("Task failed: {}", e),
                None,
                Some("Internal error".to_string()),
            ));
        }
    }
}
```

### 3. 资源管理模式

#### 连接池管理:
```rust
// PostgreSQL 连接池配置
let pool = PgPoolOptions::new()
    .max_connections(5)                    // 最大连接数
    .acquire_timeout(Duration::from_secs(10)) // 获取连接超时
    .idle_timeout(Duration::from_secs(600))   // 空闲连接超时
    .max_lifetime(Duration::from_secs(1800))  // 连接最大生命周期
    .connect(&connection_string)
    .await?;

// 使用连接池执行查询
let result = sqlx::query("SELECT 1")
    .fetch_one(&pool)
    .await?;

// 连接池会自动管理连接的创建、复用和清理
```

#### 内存管理优化:
```rust
// 大文件测试的内存优化
async fn test_large_file_performance(&self, op: &opendal::Operator) {
    // 1. 预分配内存避免重复分配
    let mut large_data = Vec::with_capacity(1024 * 1024 * 1024);
    large_data.resize(1024 * 1024 * 1024, 0);

    // 2. 执行测试
    let result = op.write("test-key", &large_data).await;

    // 3. 立即释放大内存块
    drop(large_data);

    // 4. 处理结果
    match result {
        Ok(_) => { /* 处理成功 */ }
        Err(e) => { /* 处理错误 */ }
    }
}
```

## 输出格式化实现

### 人类可读格式

#### 彩色输出实现:
```rust
use colored::*;

impl CheckResult {
    pub fn format_human_readable(&self) -> String {
        let mut output = String::new();

        // 标题
        output.push_str(&format!("\n{}\n", "GreptimeDB Self-Test Report".bold().blue()));
        output.push_str(&format!("{}\n\n", "===========================".blue()));

        // 基本信息
        output.push_str(&format!("Component: {}\n", self.component.bold()));
        output.push_str(&format!("Configuration: {}\n", self.config_file));
        output.push_str(&format!("Total Duration: {:.2?}\n\n",
                                Duration::from_millis(self.total_duration_ms)));

        // 详细检查结果
        for detail in &self.details {
            let status_symbol = match detail.status {
                CheckStatus::Pass => "✓".green(),
                CheckStatus::Fail => "✗".red(),
                CheckStatus::Warning => "⚠".yellow(),
            };

            let status_text = match detail.status {
                CheckStatus::Pass => "[PASS]".green(),
                CheckStatus::Fail => "[FAIL]".red(),
                CheckStatus::Warning => "[WARN]".yellow(),
            };

            // 格式化持续时间
            let duration_str = if let Some(duration_ms) = detail.duration_ms {
                format!(" ({:.2?})", Duration::from_millis(duration_ms))
            } else {
                String::new()
            };

            output.push_str(&format!("{} {:<30} {}{} - {}\n",
                status_symbol,
                detail.item,
                status_text,
                duration_str,
                detail.message
            ));

            // 添加建议（如果有）
            if let Some(suggestion) = &detail.suggestion {
                output.push_str(&format!("    {} {}\n",
                    "💡 Suggestion:".yellow(),
                    suggestion.italic()
                ));
            }
        }

        // 总结
        output.push_str(&format!("\n{}: {}\n",
            "Overall Result".bold(),
            match self.overall_result {
                CheckStatus::Pass => "PASS".green().bold(),
                CheckStatus::Fail => "FAIL".red().bold(),
                CheckStatus::Warning => "WARNING".yellow().bold(),
            }
        ));

        output
    }
}
```

### JSON 格式实现

#### 结构化输出:
```rust
impl CheckResult {
    pub fn format_json(&self) -> Result<String, serde_json::Error> {
        // 使用 serde_json 进行序列化
        serde_json::to_string_pretty(self)
    }
}

// 自定义序列化格式
#[derive(Serialize)]
struct JsonOutput {
    component: String,
    config_file: String,
    timestamp: DateTime<Utc>,
    overall_result: CheckStatus,
    summary: CheckSummary,
    details: Vec<CheckDetail>,
}

#[derive(Serialize)]
struct CheckSummary {
    total_checks: usize,
    passed_checks: usize,
    failed_checks: usize,
    warning_checks: usize,
    total_duration_ms: u64,
    success_rate: f64,
}
```

## 依赖管理和 Crate 选择策略

### 核心依赖分析

#### 1. 异步运行时选择
```toml
# Cargo.toml
[dependencies]
tokio = { version = "1.0", features = ["full"] }
async-trait = "0.1"
```

**选择理由**:
- **tokio**: Rust 生态系统中最成熟的异步运行时
- **features = ["full"]**: 包含所有功能，简化开发
- **async-trait**: 支持 trait 中的异步方法，是当前的标准解决方案

#### 2. 网络客户端选择
```toml
etcd-client = "0.12"
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "postgres"] }
opendal = { version = "0.47", features = ["services-s3"] }
```

**选择理由**:
- **etcd-client**: etcd 官方维护的 Rust 客户端，API 稳定
- **sqlx**: 异步 SQL 客户端，支持编译时 SQL 检查
- **opendal**: 统一的对象存储抽象层，支持多种存储后端

#### 3. 序列化框架选择
```toml
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"
serde_json = "1.0"
```

**选择理由**:
- **serde**: Rust 生态系统的标准序列化框架
- **derive 特性**: 自动生成序列化/反序列化代码
- **toml**: 专门的 TOML 解析器，错误信息友好

### 版本管理策略

#### 语义化版本控制:
```toml
[dependencies]
# 主要版本锁定，允许补丁更新
tokio = "1.0"           # 允许 1.x.x
serde = "1.0"           # 允许 1.x.x

# 次要版本锁定，用于快速变化的 crate
opendal = "0.47"        # 允许 0.47.x
etcd-client = "0.12"    # 允许 0.12.x

# 精确版本锁定，用于关键依赖
clap = "=4.4.0"         # 精确版本
```

## 测试策略和实现

### 单元测试结构

#### 模块化测试:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;
    use std::sync::Arc;

    // 测试辅助函数
    async fn setup_test_environment() -> TestEnvironment {
        TestEnvironment {
            etcd_client: setup_etcd_client().await,
            postgres_pool: setup_postgres_pool().await,
            s3_operator: setup_s3_operator().await,
        }
    }

    #[tokio::test]
    async fn test_etcd_connection_success() {
        let config = MetasrvConfig {
            store_addrs: vec!["127.0.0.1:2379".to_string()],
            backend_type: "etcd_store".to_string(),
            ..Default::default()
        };

        let checker = MetasrvChecker::new(config);
        let result = checker.check().await;

        assert_eq!(result.overall_result, CheckStatus::Pass);
        assert!(result.details.iter().any(|d| d.item.contains("Etcd Connection")));
    }

    #[tokio::test]
    async fn test_etcd_connection_failure() {
        let config = MetasrvConfig {
            store_addrs: vec!["127.0.0.1:9999".to_string()], // 无效端口
            backend_type: "etcd_store".to_string(),
            ..Default::default()
        };

        let checker = MetasrvChecker::new(config);
        let result = checker.check().await;

        assert_eq!(result.overall_result, CheckStatus::Fail);
        assert!(result.details.iter().any(|d|
            d.item.contains("Etcd Connection") && d.status == CheckStatus::Fail
        ));
    }

    #[tokio::test]
    async fn test_s3_performance_benchmarks() {
        let env = setup_test_environment().await;

        let config = DatanodeConfig {
            storage: StorageConfig {
                r#type: "S3".to_string(),
                bucket: Some("test-bucket".to_string()),
                // ... 其他 S3 配置
            },
            // ... 其他配置
        };

        let checker = DatanodeChecker::new(config);
        let result = checker.check().await;

        // 验证性能测试项存在
        assert!(result.details.iter().any(|d| d.item.contains("64MB File Write Performance")));
        assert!(result.details.iter().any(|d| d.item.contains("1GB File Write Performance")));
        assert!(result.details.iter().any(|d| d.item.contains("Concurrent Operations")));

        // 验证性能指标在合理范围内
        for detail in &result.details {
            if detail.item.contains("Performance") {
                assert!(detail.duration_ms.is_some());
                if let Some(duration) = detail.duration_ms {
                    assert!(duration < 60000); // 不应超过 60 秒
                }
            }
        }
    }
}
```

### 集成测试实现

#### Docker 环境测试:
```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::process::Command;

    #[tokio::test]
    #[ignore] // 需要 Docker 环境
    async fn test_real_cluster_validation() {
        // 1. 启动测试集群
        let output = Command::new("docker")
            .args(&["compose", "-f", "cluster-with-etcd.yaml", "up", "-d"])
            .output()
            .expect("Failed to start cluster");

        assert!(output.status.success());

        // 2. 等待服务启动
        tokio::time::sleep(Duration::from_secs(30)).await;

        // 3. 测试各组件
        let metasrv_config = load_config("test-metasrv.toml").unwrap();
        let metasrv_checker = MetasrvChecker::new(metasrv_config);
        let metasrv_result = metasrv_checker.check().await;
        assert_eq!(metasrv_result.overall_result, CheckStatus::Pass);

        let frontend_config = load_config("test-frontend.toml").unwrap();
        let frontend_checker = FrontendChecker::new(frontend_config);
        let frontend_result = frontend_checker.check().await;
        assert_eq!(frontend_result.overall_result, CheckStatus::Pass);

        let datanode_config = load_config("test-datanode.toml").unwrap();
        let datanode_checker = DatanodeChecker::new(datanode_config);
        let datanode_result = datanode_checker.check().await;
        assert_eq!(datanode_result.overall_result, CheckStatus::Pass);

        // 4. 清理测试环境
        let _ = Command::new("docker")
            .args(&["compose", "-f", "cluster-with-etcd.yaml", "down"])
            .output();
    }
}
```

## 扩展开发指南

### 添加新组件检查器

#### 1. 定义配置结构:
```rust
// 在新的模块文件中定义配置
#[derive(Debug, Deserialize)]
pub struct FlownodeConfig {
    pub node_id: Option<u64>,
    pub meta_client: MetaClientConfig,
    pub grpc: Option<GrpcConfig>,
    pub http: Option<HttpConfig>,
    // Flownode 特有的配置
    pub flow_engine: Option<FlowEngineConfig>,
}

#[derive(Debug, Deserialize)]
pub struct FlowEngineConfig {
    pub max_concurrent_flows: Option<usize>,
    pub flow_timeout: Option<String>,
}
```

#### 2. 实现检查器:
```rust
pub struct FlownodeChecker {
    config: FlownodeConfig,
}

impl FlownodeChecker {
    pub fn new(config: FlownodeConfig) -> Self {
        Self { config }
    }

    async fn check_flow_engine(&self, details: &mut Vec<CheckDetail>) {
        // 实现 Flow Engine 特有的检查逻辑
    }
}

#[async_trait]
impl ComponentChecker for FlownodeChecker {
    async fn check(&self) -> CheckResult {
        let mut details = Vec::new();

        // 通用检查
        self.check_metasrv_connectivity(&mut details).await;

        // Flownode 特有检查
        self.check_flow_engine(&mut details).await;

        CheckResult::from_details(details)
    }

    fn component_name(&self) -> &'static str {
        "Flownode"
    }
}
```

#### 3. 集成到 CLI:
```rust
// 在 main.rs 中添加新的子命令
#[derive(Parser)]
enum Commands {
    Metasrv { config: String, output: Option<String> },
    Frontend { config: String, output: Option<String> },
    Datanode { config: String, output: Option<String> },
    Flownode { config: String, output: Option<String> }, // 新增
}

// 在 match 语句中处理新命令
match cli.command {
    Commands::Flownode { config, output } => {
        let config_content = std::fs::read_to_string(&config)?;
        let flownode_config: FlownodeConfig = toml::from_str(&config_content)?;
        let checker = FlownodeChecker::new(flownode_config);
        let result = checker.check().await;

        match output.as_deref() {
            Some("json") => println!("{}", result.format_json()?),
            _ => println!("{}", result.format_human_readable()),
        }
    }
    // ... 其他命令
}
```

### 性能优化最佳实践

#### 1. 异步操作优化:
```rust
// 使用 join! 并行执行独立的检查
use tokio::join;

async fn parallel_checks(&self) -> CheckResult {
    let mut details = Vec::new();

    // 并行执行多个独立检查
    let (connectivity_result, config_result, permission_result) = join!(
        self.check_connectivity(),
        self.check_configuration(),
        self.check_permissions()
    );

    details.extend(connectivity_result);
    details.extend(config_result);
    details.extend(permission_result);

    CheckResult::from_details(details)
}
```

#### 2. 内存使用优化:
```rust
// 使用迭代器避免中间集合
let results: Vec<CheckDetail> = self.config.store_addrs
    .iter()
    .enumerate()
    .map(|(i, addr)| async move {
        self.test_connection(i, addr).await
    })
    .collect::<FuturesUnordered<_>>()
    .collect()
    .await;
```

#### 3. 错误传播优化:
```rust
// 使用 ? 操作符简化错误处理
async fn check_with_error_propagation(&self) -> Result<CheckDetail, Box<dyn std::error::Error>> {
    let client = etcd_client::Client::connect(&self.config.store_addrs, None).await?;
    let response = client.put("test", "value", None).await?;

    Ok(CheckDetail::pass(
        "Operation".to_string(),
        "Success".to_string(),
        None,
    ))
}
```

## 调试和故障排除

### 日志记录策略

#### 结构化日志:
```rust
use tracing::{info, warn, error, debug, instrument};

#[instrument(skip(self))]
async fn check_etcd_store(&self, details: &mut Vec<CheckDetail>) {
    info!("Starting etcd store check");
    debug!("etcd endpoints: {:?}", self.config.store_addrs);

    match etcd_client::Client::connect(&self.config.store_addrs, None).await {
        Ok(client) => {
            info!("etcd connection successful");
            // 继续检查...
        }
        Err(e) => {
            error!("etcd connection failed: {}", e);
            // 错误处理...
        }
    }
}
```

### 性能分析工具

#### 内置性能监控:
```rust
use std::time::Instant;

struct PerformanceMonitor {
    start_time: Instant,
    checkpoints: Vec<(String, Instant)>,
}

impl PerformanceMonitor {
    fn new() -> Self {
        Self {
            start_time: Instant::now(),
            checkpoints: Vec::new(),
        }
    }

    fn checkpoint(&mut self, name: &str) {
        self.checkpoints.push((name.to_string(), Instant::now()));
    }

    fn report(&self) -> String {
        let mut report = String::new();
        let mut last_time = self.start_time;

        for (name, time) in &self.checkpoints {
            let duration = time.duration_since(last_time);
            report.push_str(&format!("{}: {:.2?}\n", name, duration));
            last_time = *time;
        }

        report
    }
}

// 使用示例
async fn check_with_monitoring(&self) -> CheckResult {
    let mut monitor = PerformanceMonitor::new();

    self.check_connectivity().await;
    monitor.checkpoint("Connectivity Check");

    self.check_permissions().await;
    monitor.checkpoint("Permission Check");

    self.check_performance().await;
    monitor.checkpoint("Performance Check");

    debug!("Performance report:\n{}", monitor.report());

    // 返回结果...
}
```

## 实际源代码文件详解

### 1. `src/main.rs` - 程序入口点

这是整个程序的入口点，负责 CLI 参数解析和程序流程控制。

#### 关键代码分析:

<augment_code_snippet path="src/main.rs" mode="EXCERPT">
````rust
#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match &cli.command {
        Commands::Frontend { config, verbose, output } => {
            run_frontend_check(config, *verbose, output).await
        }
        Commands::Datanode { config, verbose, include_performance, output } => {
            run_datanode_check(config, *verbose, *include_performance, output).await
        }
        Commands::Metasrv { config, verbose, output } => {
            run_metasrv_check(config, *verbose, output).await
        }
    };
````
</augment_code_snippet>

**技术要点**:
- 使用 `#[tokio::main]` 宏创建异步 main 函数
- 通过 `clap::Parser` 自动生成 CLI 参数解析
- 模式匹配分发到不同的组件检查函数
- 统一的错误处理和退出码管理

#### CLI 结构定义:
```rust
#[derive(Parser)]
#[command(name = "stepstone")]
#[command(about = "GreptimeDB cluster self-test tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check metasrv configuration and connectivity
    Metasrv {
        /// Path to metasrv configuration file
        #[arg(short, long)]
        config: String,

        /// Enable verbose output
        #[arg(short, long)]
        verbose: bool,

        /// Output format (json or human-readable)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Check frontend configuration and connectivity
    Frontend {
        #[arg(short, long)]
        config: String,
        #[arg(short, long)]
        verbose: bool,
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Check datanode configuration and storage
    Datanode {
        #[arg(short, long)]
        config: String,
        #[arg(short, long)]
        verbose: bool,
        /// Include performance tests for storage
        #[arg(long)]
        include_performance: bool,
        #[arg(short, long)]
        output: Option<String>,
    },
}
```

### 2. `src/common.rs` - 核心数据结构

这个文件定义了整个工具的核心数据结构和接口。

#### ComponentChecker Trait:

<augment_code_snippet path="src/common.rs" mode="EXCERPT">
````rust
#[async_trait]
pub trait ComponentChecker {
    /// Perform the check and return the result
    async fn check(&self) -> CheckResult;

    /// Get the name of the component being checked
    fn component_name(&self) -> &'static str;
}
````
</augment_code_snippet>

**设计理念**:
- 使用 trait 定义统一的检查接口
- `async fn check()` 支持异步检查操作
- 返回标准化的 `CheckResult` 结构

#### CheckResult 数据结构:

<augment_code_snippet path="src/common.rs" mode="EXCERPT">
````rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub success: bool,
    pub message: String,
    pub details: Vec<CheckDetail>,
    pub total_duration: Option<Duration>,
}
````
</augment_code_snippet>

**技术特点**:
- 使用 `serde` 支持 JSON 序列化
- `Clone` trait 支持结果复制
- 包含总体成功状态和详细检查项

#### 输出格式化实现:

<augment_code_snippet path="src/common.rs" mode="EXCERPT">
````rust
pub fn print_human_readable(&self, component_name: &str, config_file: Option<&str>) {
    println!("\n{}", "GreptimeDB Self-Test Report".bold().blue());
    println!("{}", "===========================".blue());

    for detail in &self.details {
        let status_symbol = match detail.status {
            CheckStatus::Pass => "✓".green(),
            CheckStatus::Fail => "✗".red(),
            CheckStatus::Warning => "⚠".yellow(),
        };

        println!("{} {:<30} {} {} - {}",
            status_symbol, detail.item, status_text, duration_text, detail.message);
    }
}
````
</augment_code_snippet>

**技术特点**:
- 使用 `colored` crate 提供彩色终端输出
- 格式化对齐确保输出整齐
- 支持 Unicode 符号 (✓, ✗, ⚠, 💡)

### 3. `src/metasrv.rs` - Metasrv 检查器实现

这是最复杂的检查器，支持多种元数据存储后端。

#### etcd 检查实现:

<augment_code_snippet path="src/metasrv.rs" mode="EXCERPT">
````rust
async fn check_etcd(&self, store_config: &StoreConfig) -> CheckResult {
    let start = Instant::now();
    let mut details = Vec::new();

    // 使用 GreptimeDB 内部的 EtcdStore
    match EtcdStore::with_endpoints(&store_config.store_addrs, store_config.max_txn_ops.unwrap_or(128)).await {
        Ok(store) => {
            details.push(CheckDetail::pass(
                "Etcd Connection".to_string(),
                format!("Successfully connected to etcd endpoints: {:?}", store_config.store_addrs),
                Some(start.elapsed()),
            ));

            // 测试 PUT 操作
            let test_key = format!("{}__stepstone_test", store_config.store_key_prefix.as_deref().unwrap_or(""));
            match store.put(PutRequest {
                key: test_key.as_bytes().to_vec(),
                value: b"stepstone_test_value".to_vec(),
                prev_kv: false,
            }).await {
                Ok(_) => {
                    details.push(CheckDetail::pass(
                        "Etcd PUT Operation".to_string(),
                        "PUT operation successful".to_string(),
                        None,
                    ));
                }
                Err(e) => {
                    details.push(CheckDetail::fail(
                        "Etcd PUT Operation".to_string(),
                        format!("PUT operation failed: {}", e),
                        None,
                        Some("Check etcd write permissions and storage space".to_string()),
                    ));
                }
            }
        }
    }
}
````
</augment_code_snippet>

**技术要点**:
- 直接使用 GreptimeDB 内部的 `EtcdStore` 类型，确保兼容性
- 实现完整的 CRUD 操作测试 (PUT/GET/DELETE)
- 使用重试机制处理 etcd 的最终一致性
- 详细的错误分类和建议生成

#### PostgreSQL 检查实现:

<augment_code_snippet path="src/metasrv.rs" mode="EXCERPT">
````rust
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

                // 检查元数据表
                let table_name = self.config.meta_table_name.as_deref().unwrap_or("greptime_metasrv");

                // 测试表存在性
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
                            // 测试现有表的读写权限
                            self.test_postgres_permissions(&pool, table_name, &mut details).await;
                        } else {
                            // 测试表创建权限
                            self.test_postgres_create_permissions(&pool, table_name, &mut details).await;
                        }
                    }
                }
            }
        }
    }
}
````
</augment_code_snippet>

**技术要点**:
- 使用 `sqlx::PgPool` 进行连接池管理
- 通过 `information_schema.tables` 检查表存在性
- 分离表创建权限和读写权限的测试
- 使用事务确保测试数据的一致性

#### PostgreSQL 权限测试详解:

<augment_code_snippet path="src/metasrv.rs" mode="EXCERPT">
````rust
async fn test_postgres_permissions(&self, pool: &PgPool, table_name: &str, details: &mut Vec<CheckDetail>) {
    // 读权限测试
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
            return;
        }
    }

    // 写权限测试 - 使用 UPSERT 模式
    let test_key = "stepstone_test_key";
    let test_value = "stepstone_test_value";
    let upsert_query = format!(
        "INSERT INTO {} (key, value) VALUES ($1, $2) ON CONFLICT (key) DO UPDATE SET value = $2",
        table_name
    );

    match sqlx::query(&upsert_query)
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

            // 清理测试记录
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
````
</augment_code_snippet>

### 4. `src/datanode.rs` - Datanode 检查器实现

这是最复杂的检查器，包含完整的 S3 性能测试套件。

#### S3 性能测试架构:

<augment_code_snippet path="src/datanode.rs" mode="EXCERPT">
````rust
async fn test_s3_performance(&self, op: &opendal::Operator, details: &mut Vec<CheckDetail>) {
    // 64MB 文件性能测试
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
        }
    }

    // 1GB 大文件性能测试
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
        }
    }
}
````
</augment_code_snippet>

**性能测试设计理念**:
- **64MB 测试**: 模拟 GreptimeDB 典型的时间序列数据块大小
- **1GB 测试**: 测试大文件处理能力和网络稳定性
- **超时控制**: 避免测试无限等待
- **吞吐量计算**: 提供 MB/s 指标用于性能评估

#### 并发操作测试实现:

<augment_code_snippet path="src/datanode.rs" mode="EXCERPT">
````rust
async fn test_s3_concurrent_performance(&self, op: &opendal::Operator, details: &mut Vec<CheckDetail>) {
    let concurrent_count = 100;
    let data = vec![0u8; 512]; // 每个操作 512 字节

    let start = Instant::now();
    let mut handles = Vec::new();

    // 创建 100 个并发写入任务
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

    // 等待所有任务完成
    let mut successful_ops = 0;
    for (handle, key) in handles {
        match timeout(Duration::from_secs(10), handle).await {
            Ok(Ok(Ok(_))) => {
                successful_ops += 1;
                keys_to_cleanup.push(key);
            }
            _ => {} // 失败或超时
        }
    }

    let total_duration = start.elapsed();
    let ops_per_second = successful_ops as f64 / total_duration.as_secs_f64();

    details.push(CheckDetail::pass(
        "S3 Concurrent Operations".to_string(),
        format!("{} concurrent writes: {:.2}ms ({:.1} ops/s)",
               concurrent_count, total_duration.as_millis(), ops_per_second),
        Some(total_duration),
    ));
}
````
</augment_code_snippet>

**并发测试技术要点**:
- 使用 `tokio::spawn` 创建真正的并发任务
- 每个任务独立的 S3 客户端克隆
- 超时控制避免单个任务阻塞整体测试
- 统计成功率和吞吐量指标
- 自动清理测试数据

### 5. `src/frontend.rs` - Frontend 检查器实现

Frontend 检查器相对简单，主要测试网络连接和配置验证。

#### Metasrv 连接测试:

<augment_code_snippet path="src/frontend.rs" mode="EXCERPT">
````rust
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

    for (index, addr) in metasrv_addrs.iter().enumerate() {
        let start = Instant::now();

        match timeout(Duration::from_secs(10), TcpStream::connect(addr)).await {
            Ok(Ok(_)) => {
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
                    Some("Check metasrv service status and network connectivity".to_string()),
                ));
            }
            Err(_) => {
                details.push(CheckDetail::fail(
                    format!("Metasrv Connectivity {}", index + 1),
                    format!("Connection to metasrv at {} timed out", addr),
                    Some(start.elapsed()),
                    Some("Check network connectivity and firewall settings".to_string()),
                ));
            }
        }
    }

    CheckResult::from_details(details)
}
````
</augment_code_snippet>

**技术特点**:
- 使用 `TcpStream::connect` 进行简单的连接测试
- 支持多个 metasrv 地址的并行测试
- 精确的超时控制和错误分类
- 详细的网络错误诊断建议

## 关键技术实现总结

### 1. 异步编程模式的深度应用

工具大量使用 Rust 的异步编程特性：

#### async/await 模式:
- 所有网络 I/O 操作都是异步的
- 使用 `#[async_trait]` 支持 trait 中的异步方法
- `tokio::spawn` 实现真正的并发操作

#### 超时控制模式:
```rust
// 统一的超时控制模式
match timeout(Duration::from_secs(30), operation()).await {
    Ok(Ok(result)) => { /* 操作成功 */ }
    Ok(Err(e)) => { /* 操作失败 */ }
    Err(_) => { /* 超时 */ }
}
```

### 2. 错误处理的分层设计

#### 三层错误处理架构:
1. **网络层**: 连接超时、DNS 解析失败
2. **认证层**: 凭据错误、权限不足
3. **应用层**: 配置错误、业务逻辑错误

#### 智能错误分类:
```rust
// 根据错误内容自动分类和生成建议
match error_message {
    e if e.contains("InvalidAccessKeyId") => "Check access key configuration",
    e if e.contains("NoSuchBucket") => "Create bucket or check bucket name",
    e if e.contains("AccessDenied") => "Check IAM permissions",
    _ => "Check general configuration"
}
```

### 3. 性能测试的科学设计

#### 多维度性能测试:
- **文件大小维度**: 64MB (典型块大小) → 1GB (大文件处理)
- **并发维度**: 100 个并发操作测试高负载场景
- **指标维度**: 吞吐量 (MB/s) + 延迟 (ms) + 成功率 (%)

#### 内存管理优化:
```rust
// 大文件测试的内存优化
let large_data = vec![0u8; 1024 * 1024 * 1024]; // 预分配 1GB
// 使用后立即释放
drop(large_data);
```

### 4. 配置系统的灵活设计

#### 多后端支持:
- **etcd**: 分布式键值存储，支持 CRUD 操作测试
- **PostgreSQL**: 关系数据库，支持权限和表创建测试
- **S3**: 对象存储，支持完整的性能基准测试
- **File**: 本地文件系统，支持权限验证

#### 配置验证机制:
```rust
// 分层配置验证
impl Config {
    fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // 必需字段验证
        if self.required_field.is_empty() {
            errors.push("Required field missing".to_string());
        }

        // 格式验证
        if !self.address.contains(':') {
            errors.push("Address must include port".to_string());
        }

        // 后端特定验证
        match self.backend_type {
            "postgres_store" => self.validate_postgres(&mut errors),
            "etcd_store" => self.validate_etcd(&mut errors),
            _ => {}
        }

        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }
}
```

### 5. 输出格式化的用户体验设计

#### 双格式支持:
- **人类可读**: 彩色输出、Unicode 符号、对齐格式
- **机器可读**: 结构化 JSON、时间戳、详细指标

#### 渐进式信息披露:
```
✓ S3 Connection                [PASS] (125ms) - Successfully connected
✓ S3 64MB Write Performance    [PASS] (156ms) - 409.20 MB/s
⚠ S3 1GB Write Performance     [WARN] (2069ms) - May be slow for production
    💡 Suggestion: Consider using faster storage or optimizing network
```

### 6. 依赖管理的最佳实践

#### 核心依赖选择策略:
- **tokio**: 异步运行时的事实标准
- **sqlx**: 编译时 SQL 检查，类型安全
- **opendal**: 统一对象存储抽象，多后端支持
- **serde**: 序列化生态系统的核心
- **clap**: 现代 CLI 参数解析

#### 版本管理策略:
```toml
# 主要版本锁定 - 稳定 API
tokio = "1.0"
serde = "1.0"

# 次要版本锁定 - 快速迭代的 crate
opendal = "0.47"
etcd-client = "0.12"
```

## 学习要点和扩展建议

### 1. 深入理解异步编程
- 掌握 `async/await` 的工作原理
- 理解 `Future` trait 和执行器模型
- 学会使用 `tokio::spawn` 进行并发控制

### 2. 错误处理模式
- 使用 `Result<T, E>` 进行可恢复错误处理
- 掌握 `?` 操作符的错误传播机制
- 学会设计分层的错误处理架构

### 3. 性能测试方法论
- 设计多维度的性能测试矩阵
- 理解吞吐量、延迟、并发度的关系
- 掌握内存管理和资源清理的最佳实践

### 4. 配置系统设计
- 使用 `serde` 进行灵活的配置解析
- 实现配置验证和错误报告机制
- 支持多种配置格式和环境变量

### 5. 用户体验设计
- 提供清晰的错误信息和解决建议
- 支持多种输出格式满足不同需求
- 使用进度指示和彩色输出提升体验

这个技术文档详细解析了 GreptimeDB 集群自检工具的每个技术层面，从架构设计到具体实现，从错误处理到性能优化，为你提供了完整的技术理解和扩展开发指南。通过学习这个项目，你可以掌握现代 Rust 应用开发的最佳实践，包括异步编程、错误处理、性能测试、配置管理等核心技能。
```
