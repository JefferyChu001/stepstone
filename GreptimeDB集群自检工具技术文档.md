# GreptimeDB 集群自检工具技术文档

## 项目概述

GreptimeDB 集群自检工具（stepstone）是一个用 Rust 编写的命令行工具，用于验证 GreptimeDB 集群各组件的配置和连通性。该工具支持对 Metasrv、Frontend 和 Datanode 三个核心组件进行全面的健康检查。

## 架构设计

### 核心模块结构

```
src/
├── main.rs          # 主程序入口和命令行接口
├── error.rs         # 统一错误处理系统
├── config.rs        # 配置文件解析
├── common.rs        # 通用检查结果和接口定义
├── metasrv.rs       # Metasrv 组件检查器
├── frontend.rs      # Frontend 组件检查器
└── datanode.rs      # Datanode 组件检查器
```

### 错误处理系统

项目采用集中式错误管理，所有错误类型都定义在 `src/error.rs` 中：

#### 核心错误类型

```rust
#[derive(Snafu)]
#[snafu(visibility(pub))]
#[stack_trace_debug]
pub enum Error {
    // 基础错误类型
    ConfigLoad { message: String },
    ConnectionFailed { message: String, location: Location },
    PermissionDenied { message: String, location: Location },
    
    // 网络和地址解析错误
    InvalidAddress { address: String, location: Location },
    MissingPort { address: String, location: Location },
    InvalidPort { address: String, port_str: String, error: ParseIntError, location: Location },
    TcpConnection { address: String, message: String, error: std::io::Error, location: Location },
    
    // 存储系统错误
    S3Config { message: String, location: Location },
    S3Operation { message: String, error: opendal::Error, location: Location },
    FileStorageOperation { message: String, error: std::io::Error, location: Location },
    
    // 数据库错误
    PostgresConnection { message: String, error: sqlx::Error, location: Location },
    PostgresQuery { message: String, error: sqlx::Error, location: Location },
    MySqlConnection { message: String, error: sqlx::Error, location: Location },
    MySqlQuery { message: String, error: sqlx::Error, location: Location },
    
    // Etcd 相关错误
    EtcdOperation { endpoints: String, error: common_meta::error::Error, location: Location },
    EtcdValueMismatch { endpoints: String, expect: String, actual: String, location: Location },
}
```

#### 错误处理特性

1. **位置跟踪**：每个错误都包含发生位置信息
2. **堆栈跟踪**：通过 `#[stack_trace_debug]` 提供详细的调用栈
3. **错误链**：支持原始错误的传播和包装
4. **类型安全**：编译时确保错误处理的完整性

## 组件检查器详解

### 1. Metasrv 检查器 (`src/metasrv.rs`)

Metasrv 是 GreptimeDB 的元数据服务，支持多种存储后端：

#### 支持的存储后端

- **Etcd Store**: 分布式键值存储
- **PostgreSQL Store**: 关系型数据库存储
- **MySQL Store**: 关系型数据库存储
- **Memory Store**: 内存存储（开发/测试用）

#### Etcd 检查实现

```rust
async fn check_etcd(&self, store_config: &StoreConfig) -> CheckResult {
    // 1. 建立 Etcd 连接
    match EtcdStore::with_endpoints(&store_config.store_addrs, store_config.max_txn_ops.unwrap_or(128)).await {
        Ok(store) => {
            // 2. 执行基本操作测试
            let test_key = format!("{}__stepstone_test", store_config.store_key_prefix.as_deref().unwrap_or(""));
            let test_value = b"stepstone_test_value";
            
            // PUT 操作测试
            match store.put(PutRequest {
                key: test_key.as_bytes().to_vec(),
                value: test_value.to_vec(),
                prev_kv: false,
            }).await {
                Ok(_) => {
                    // GET 操作测试
                    match store.get(test_key.as_bytes()).await {
                        Ok(Some(value)) => {
                            // 验证数据一致性
                            if value.value == test_value {
                                // DELETE 操作测试
                                store.delete(test_key.as_bytes(), false).await;
                                // 记录成功结果
                            }
                        }
                    }
                }
            }
        }
    }
}
```

#### PostgreSQL/MySQL 检查实现

```rust
async fn check_postgres(&self, store_config: &StoreConfig) -> CheckResult {
    if let Some(addr) = store_config.store_addrs.first() {
        match PgPool::connect(addr).await {
            Ok(pool) => {
                // 检查元数据表是否存在
                let table_name = store_config.meta_table_name.as_deref().unwrap_or("greptime_metasrv");
                let query = format!(
                    "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = '{}')",
                    table_name
                );
                
                match sqlx::query(&query).fetch_one(&pool).await {
                    Ok(row) => {
                        let exists: bool = row.get(0);
                        if exists {
                            // 测试基本的 CRUD 操作
                            // INSERT, SELECT, UPDATE, DELETE 测试
                        }
                    }
                }
            }
        }
    }
}
```

### 2. Frontend 检查器 (`src/frontend.rs`)

Frontend 是 GreptimeDB 的查询前端，主要检查与 Metasrv 的连通性：

#### 连通性检查实现

```rust
async fn check_metasrv_connectivity(&self) -> CheckResult {
    for (index, addr) in self.config.metasrv_addrs.iter().enumerate() {
        // 1. 解析地址
        let (host, port) = match self.parse_address(addr) {
            Ok((h, p)) => (h, p),
            Err(e) => {
                // 记录地址解析错误
                continue;
            }
        };
        
        // 2. TCP 连接测试
        match timeout(Duration::from_secs(10), TcpStream::connect((host.as_str(), port))).await {
            Ok(Ok(_stream)) => {
                // 连接成功
            }
            Ok(Err(e)) => {
                // 连接失败
            }
            Err(_) => {
                // 连接超时
            }
        }
    }
}
```

#### 地址解析实现

```rust
fn parse_address(&self, addr: &str) -> error::Result<(String, u16)> {
    // 处理不同的地址格式
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

fn parse_host_port(&self, addr: &str) -> error::Result<(String, u16)> {
    if let Some(colon_pos) = addr.rfind(':') {
        let host = addr[..colon_pos].to_string();
        let port_str = &addr[colon_pos + 1..];
        
        // 移除路径部分
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
```

### 3. Datanode 检查器 (`src/datanode.rs`)

Datanode 是 GreptimeDB 的数据存储节点，需要检查与 Metasrv 的连通性和对象存储的访问：

#### 支持的对象存储类型

- **Amazon S3**: AWS 对象存储
- **阿里云 OSS**: 阿里云对象存储
- **Azure Blob**: 微软云对象存储
- **Google Cloud Storage**: 谷歌云对象存储
- **File Storage**: 本地文件存储

#### S3 存储检查实现

```rust
async fn check_s3_storage(&self) -> CheckResult {
    // 1. 解析 S3 配置
    let s3_config = match self.config.storage.as_s3_config() {
        Ok(config) => config,
        Err(e) => {
            // 配置解析失败
            return CheckResult::from_details(details);
        }
    };
    
    // 2. 构建 S3 操作器
    let mut builder = S3::default()
        .root(s3_config.root.as_deref().unwrap_or(""))
        .bucket(&s3_config.bucket)
        .access_key_id(&s3_config.access_key_id)
        .secret_access_key(&s3_config.secret_access_key);
    
    if let Some(endpoint) = &s3_config.endpoint {
        builder = builder.endpoint(endpoint);
    }
    
    let op = Operator::new(builder)?.finish();
    
    // 3. 执行基本操作测试
    let test_key = format!("stepstone_test_{}", chrono::Utc::now().timestamp());
    let test_data = b"stepstone test data";
    
    // PUT 测试
    match op.write(&test_key, test_data).await {
        Ok(_) => {
            // GET 测试
            match op.read(&test_key).await {
                Ok(data) => {
                    if data.len() == test_data.len() {
                        // LIST 测试
                        match op.list("/").await {
                            Ok(_) => {
                                // DELETE 测试
                                let _ = op.delete(&test_key).await;
                                
                                // 性能测试（可选）
                                if self.include_performance {
                                    let perf_result = self.performance_test_s3(&op).await;
                                    details.extend(perf_result.details);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
```

#### 性能测试实现

```rust
async fn performance_test_s3(&self, op: &Operator) -> CheckResult {
    let test_sizes = vec![
        (1024, "1KB"),
        (1024 * 1024, "1MB"),
        (10 * 1024 * 1024, "10MB"),
    ];
    
    for (size, size_name) in test_sizes {
        let test_data = vec![0u8; size];
        let test_key = format!("perf_test_{}_{}", size_name, chrono::Utc::now().timestamp());
        
        // 写入延迟测试
        let start = Instant::now();
        match op.write(&test_key, &test_data).await {
            Ok(_) => {
                let write_latency = start.elapsed();
                let write_throughput = (size as f64) / write_latency.as_secs_f64() / (1024.0 * 1024.0);
                
                // 读取延迟测试
                let start = Instant::now();
                match op.read(&test_key).await {
                    Ok(read_data) => {
                        let read_latency = start.elapsed();
                        let read_throughput = (read_data.len() as f64) / read_latency.as_secs_f64() / (1024.0 * 1024.0);
                        
                        // 记录性能指标
                        details.push(CheckDetail::pass(
                            format!("S3 Write Performance ({})", size_name),
                            format!("Latency: {:?}, Throughput: {:.2} MB/s", write_latency, write_throughput),
                            Some(write_latency),
                        ));
                        
                        details.push(CheckDetail::pass(
                            format!("S3 Read Performance ({})", size_name),
                            format!("Latency: {:?}, Throughput: {:.2} MB/s", read_latency, read_throughput),
                            Some(read_latency),
                        ));
                    }
                }
                
                // 清理测试文件
                let _ = op.delete(&test_key).await;
            }
        }
    }
    
    // 并发操作测试
    let concurrent_result = self.performance_test_concurrent_s3(op).await;
    details.extend(concurrent_result.details);
    
    CheckResult::from_details(details)
}
```

## 配置文件格式

### Metasrv 配置示例

#### Etcd 存储配置
```toml
[store]
store_type = "etcd_store"
store_addrs = ["127.0.0.1:2379", "127.0.0.1:2380", "127.0.0.1:2381"]
store_key_prefix = "/greptime"
max_txn_ops = 128
```

#### PostgreSQL 存储配置
```toml
[store]
store_type = "postgres_store"
store_addrs = ["postgresql://username:password@localhost:5432/greptime"]
meta_table_name = "greptime_metasrv"
```

#### MySQL 存储配置
```toml
[store]
store_type = "mysql_store"
store_addrs = ["mysql://username:password@localhost:3306/greptime"]
meta_table_name = "greptime_metasrv"
```

### Frontend 配置示例

```toml
metasrv_addrs = ["127.0.0.1:3002", "127.0.0.1:3003"]

[server]
addr = "0.0.0.0:4000"
timeout = "30s"
```

### Datanode 配置示例

#### S3 存储配置
```toml
metasrv_addrs = ["127.0.0.1:3002"]

[storage]
storage_type = "S3"

[storage.config]
bucket = "my-greptime-bucket"
root = "/greptime-data"
access_key_id = "your-access-key"
secret_access_key = "your-secret-key"
endpoint = "https://s3.amazonaws.com"
region = "us-east-1"
```

#### 文件存储配置
```toml
metasrv_addrs = ["127.0.0.1:3002"]

[storage]
storage_type = "File"

[storage.config]
root = "/var/lib/greptime/data"
```

## 使用方法

### 基本命令

```bash
# 检查 Metasrv 配置
./stepstone metasrv -c /path/to/metasrv.toml

# 检查 Frontend 配置
./stepstone frontend -c /path/to/frontend.toml

# 检查 Datanode 配置
./stepstone datanode -c /path/to/datanode.toml

# 包含性能测试的 Datanode 检查
./stepstone datanode -c /path/to/datanode.toml --include-performance

# JSON 格式输出
./stepstone metasrv -c /path/to/metasrv.toml --output json
```

### 输出格式

#### 人类可读格式
```
=== GreptimeDB Metasrv Configuration Check ===
Config File: /path/to/metasrv.toml
Timestamp: 2024-01-15T10:30:00Z

✓ Etcd Connection: Successfully connected to etcd endpoints: ["127.0.0.1:2379"] (Duration: 45ms)
✓ Etcd PUT Operation: Successfully wrote test data to etcd (Duration: 12ms)
✓ Etcd GET Operation: Successfully read and verified test data (Duration: 8ms)
✓ Etcd DELETE Operation: Successfully deleted test data (Duration: 10ms)

Overall Result: PASS
Total Checks: 4 | Passed: 4 | Failed: 0 | Warnings: 0
Total Duration: 75ms
```

#### JSON 格式
```json
{
  "component": "Metasrv",
  "config_file": "/path/to/metasrv.toml",
  "timestamp": "2024-01-15T10:30:00Z",
  "overall_result": "PASS",
  "total_checks": 4,
  "passed_checks": 4,
  "failed_checks": 0,
  "warning_checks": 0,
  "total_duration_ms": 75,
  "message": "All checks passed successfully",
  "details": [
    {
      "name": "Etcd Connection",
      "status": "PASS",
      "message": "Successfully connected to etcd endpoints: [\"127.0.0.1:2379\"]",
      "duration_ms": 45,
      "suggestion": null
    }
  ]
}
```

## 检查项目详解

### Metasrv 检查项目

1. **存储后端连接测试**
   - 验证到 Etcd/PostgreSQL/MySQL 的网络连通性
   - 测试认证和权限

2. **基本操作测试**
   - PUT/GET/DELETE 操作验证
   - 数据一致性检查

3. **元数据表验证**（数据库存储）
   - 检查必要的表结构是否存在
   - 验证表的读写权限

### Frontend 检查项目

1. **Metasrv 连通性测试**
   - TCP 连接测试
   - 超时和重试机制验证

2. **服务器配置验证**
   - 监听地址和端口检查
   - 配置参数合理性验证

### Datanode 检查项目

1. **Metasrv 连通性测试**
   - 与 Frontend 相同的连通性检查

2. **对象存储访问测试**
   - 存储桶/容器访问权限验证
   - 基本的 CRUD 操作测试

3. **性能基准测试**（可选）
   - 不同数据大小的读写延迟测试
   - 吞吐量测试
   - 并发操作测试

## 错误诊断和建议

工具会为每种错误类型提供具体的诊断信息和修复建议：

### 常见错误类型

1. **连接错误**
   - 网络不可达
   - 端口未开放
   - 服务未启动

2. **认证错误**
   - 用户名/密码错误
   - 访问密钥无效
   - 权限不足

3. **配置错误**
   - 配置文件格式错误
   - 必要参数缺失
   - 参数值无效

4. **性能问题**
   - 延迟过高
   - 吞吐量不足
   - 并发能力有限

每个错误都会包含：
- 详细的错误描述
- 发生的具体位置
- 建议的修复方案
- 相关的配置参数说明

## 扩展和定制

### 添加新的检查项目

1. 在相应的检查器中添加新的检查函数
2. 在 `Error` 枚举中添加相应的错误类型
3. 更新配置结构体以支持新的参数
4. 添加相应的测试用例

### 支持新的存储后端

1. 在 `config.rs` 中添加新的配置结构体
2. 在 `datanode.rs` 中实现相应的检查逻辑
3. 在 `error.rs` 中添加特定的错误类型
4. 更新文档和示例配置

## 部署和集成

### 编译和安装

```bash
# 克隆项目
git clone https://github.com/GreptimeTeam/stepstone.git
cd stepstone

# 编译发布版本
cargo build --release

# 安装到系统路径
sudo cp target/release/stepstone /usr/local/bin/

# 验证安装
stepstone --version
```

### Docker 集成

```dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/stepstone /usr/local/bin/stepstone
ENTRYPOINT ["stepstone"]
```

### CI/CD 集成

#### GitHub Actions 示例

```yaml
name: GreptimeDB Health Check
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  health-check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Setup GreptimeDB
        run: |
          # 启动 GreptimeDB 集群
          docker-compose up -d

      - name: Build stepstone
        run: cargo build --release

      - name: Run health checks
        run: |
          ./target/release/stepstone metasrv -c configs/metasrv.toml --output json > metasrv-check.json
          ./target/release/stepstone frontend -c configs/frontend.toml --output json > frontend-check.json
          ./target/release/stepstone datanode -c configs/datanode.toml --output json > datanode-check.json

      - name: Upload results
        uses: actions/upload-artifact@v3
        with:
          name: health-check-results
          path: "*-check.json"
```

## 监控和告警集成

### Prometheus 指标导出

可以扩展工具以导出 Prometheus 格式的指标：

```rust
// 在 main.rs 中添加指标导出功能
use prometheus::{Counter, Histogram, Registry};

pub struct HealthCheckMetrics {
    pub checks_total: Counter,
    pub check_duration: Histogram,
    pub failures_total: Counter,
}

impl HealthCheckMetrics {
    pub fn new() -> Self {
        Self {
            checks_total: Counter::new("stepstone_checks_total", "Total number of checks performed").unwrap(),
            check_duration: Histogram::new("stepstone_check_duration_seconds", "Duration of health checks").unwrap(),
            failures_total: Counter::new("stepstone_failures_total", "Total number of failed checks").unwrap(),
        }
    }

    pub fn register(&self, registry: &Registry) {
        registry.register(Box::new(self.checks_total.clone())).unwrap();
        registry.register(Box::new(self.check_duration.clone())).unwrap();
        registry.register(Box::new(self.failures_total.clone())).unwrap();
    }
}
```

### Grafana 仪表板配置

```json
{
  "dashboard": {
    "title": "GreptimeDB Health Check Dashboard",
    "panels": [
      {
        "title": "Check Success Rate",
        "type": "stat",
        "targets": [
          {
            "expr": "rate(stepstone_checks_total[5m]) - rate(stepstone_failures_total[5m])"
          }
        ]
      },
      {
        "title": "Check Duration",
        "type": "graph",
        "targets": [
          {
            "expr": "histogram_quantile(0.95, rate(stepstone_check_duration_seconds_bucket[5m]))"
          }
        ]
      }
    ]
  }
}
```

## 高级配置和调优

### 超时和重试配置

```toml
# 在配置文件中添加超时设置
[timeouts]
connection_timeout = "10s"
operation_timeout = "30s"
total_timeout = "300s"

[retry]
max_attempts = 3
backoff_multiplier = 2.0
initial_delay = "1s"
max_delay = "30s"
```

### 并发控制

```rust
// 在检查器中实现并发控制
use tokio::sync::Semaphore;
use std::sync::Arc;

pub struct ConcurrencyConfig {
    pub max_concurrent_checks: usize,
    pub max_concurrent_connections: usize,
}

impl DatanodeChecker {
    async fn check_with_concurrency_control(&self) -> CheckResult {
        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent_checks));

        let mut tasks = Vec::new();
        for addr in &self.config.metasrv_addrs {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let addr = addr.clone();

            tasks.push(tokio::spawn(async move {
                let _permit = permit; // 持有许可证
                // 执行检查逻辑
                self.check_single_metasrv(&addr).await
            }));
        }

        // 等待所有任务完成
        let results = futures::future::join_all(tasks).await;
        // 合并结果
        self.merge_check_results(results)
    }
}
```

### 缓存和优化

```rust
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub struct CheckCache {
    cache: HashMap<String, (CheckResult, Instant)>,
    ttl: Duration,
}

impl CheckCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            cache: HashMap::new(),
            ttl,
        }
    }

    pub fn get(&self, key: &str) -> Option<&CheckResult> {
        if let Some((result, timestamp)) = self.cache.get(key) {
            if timestamp.elapsed() < self.ttl {
                return Some(result);
            }
        }
        None
    }

    pub fn insert(&mut self, key: String, result: CheckResult) {
        self.cache.insert(key, (result, Instant::now()));
    }
}
```

## 故障排除指南

### 常见问题和解决方案

#### 1. Etcd 连接问题

**问题**: `EtcdOperation` 错误，无法连接到 etcd

**可能原因**:
- etcd 服务未启动
- 网络连接问题
- 认证配置错误
- TLS 配置问题

**解决步骤**:
```bash
# 1. 检查 etcd 服务状态
systemctl status etcd

# 2. 测试网络连通性
telnet 127.0.0.1 2379

# 3. 检查 etcd 健康状态
etcdctl endpoint health

# 4. 验证认证配置
etcdctl --user=root:password get /test
```

**配置调整**:
```toml
[store]
store_type = "etcd_store"
store_addrs = ["127.0.0.1:2379"]
# 添加认证信息
username = "root"
password = "your-password"
# TLS 配置
tls_cert_path = "/path/to/cert.pem"
tls_key_path = "/path/to/key.pem"
tls_ca_path = "/path/to/ca.pem"
```

#### 2. S3 存储访问问题

**问题**: `S3Operation` 错误，无法访问 S3 存储

**可能原因**:
- 访问密钥错误
- 存储桶不存在或无权限
- 网络连接问题
- 区域配置错误

**解决步骤**:
```bash
# 1. 验证 AWS 凭证
aws sts get-caller-identity

# 2. 测试存储桶访问
aws s3 ls s3://your-bucket-name

# 3. 检查存储桶权限
aws s3api get-bucket-policy --bucket your-bucket-name
```

**配置调整**:
```toml
[storage.config]
bucket = "your-bucket-name"
region = "us-east-1"
access_key_id = "your-access-key"
secret_access_key = "your-secret-key"
# 对于非 AWS S3 兼容存储
endpoint = "https://your-s3-endpoint.com"
# 强制路径样式（某些 S3 兼容存储需要）
force_path_style = true
```

#### 3. 数据库连接问题

**问题**: `PostgresConnection` 或 `MySqlConnection` 错误

**可能原因**:
- 数据库服务未启动
- 连接字符串格式错误
- 用户名密码错误
- 数据库不存在

**解决步骤**:
```bash
# PostgreSQL
psql -h localhost -U username -d greptime -c "SELECT 1;"

# MySQL
mysql -h localhost -u username -p greptime -e "SELECT 1;"
```

**配置调整**:
```toml
[store]
store_type = "postgres_store"
# 完整的连接字符串
store_addrs = ["postgresql://username:password@localhost:5432/greptime?sslmode=disable"]
# 或者使用环境变量
store_addrs = ["postgresql://${DB_USER}:${DB_PASSWORD}@${DB_HOST}:${DB_PORT}/${DB_NAME}"]
```

### 调试模式

启用详细日志输出：

```bash
# 设置日志级别
export RUST_LOG=debug
./stepstone metasrv -c config.toml --verbose

# 或者使用环境变量
RUST_LOG=stepstone=debug ./stepstone metasrv -c config.toml
```

### 性能调优建议

#### 1. 连接池配置

```toml
[database]
max_connections = 10
min_connections = 1
connection_timeout = "30s"
idle_timeout = "600s"
```

#### 2. 批量操作优化

```rust
// 批量检查多个端点
async fn batch_check_endpoints(&self, endpoints: &[String]) -> Vec<CheckResult> {
    let batch_size = 10;
    let mut results = Vec::new();

    for chunk in endpoints.chunks(batch_size) {
        let batch_tasks: Vec<_> = chunk.iter()
            .map(|endpoint| self.check_single_endpoint(endpoint))
            .collect();

        let batch_results = futures::future::join_all(batch_tasks).await;
        results.extend(batch_results);
    }

    results
}
```

#### 3. 内存使用优化

```rust
// 使用流式处理大文件
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn stream_large_file_test(&self, op: &Operator) -> CheckResult {
    let mut reader = tokio::io::stdin();
    let mut buffer = [0u8; 8192]; // 8KB 缓冲区

    let mut total_size = 0;
    let start = Instant::now();

    loop {
        match reader.read(&mut buffer).await {
            Ok(0) => break, // EOF
            Ok(n) => {
                op.write_with(&format!("stream_test_{}", total_size), &buffer[..n]).await?;
                total_size += n;
            }
            Err(e) => return Err(e.into()),
        }
    }

    let duration = start.elapsed();
    let throughput = (total_size as f64) / duration.as_secs_f64() / (1024.0 * 1024.0);

    CheckResult::success(
        format!("Streamed {} bytes in {:?} ({:.2} MB/s)", total_size, duration, throughput),
        vec![]
    )
}
```

## 最佳实践

### 1. 配置管理

- 使用环境变量管理敏感信息
- 为不同环境维护不同的配置文件
- 使用配置模板和变量替换

### 2. 自动化集成

- 在部署流水线中集成健康检查
- 设置定期的健康检查任务
- 配置告警和通知机制

### 3. 监控和日志

- 收集和分析检查结果
- 建立性能基线和趋势分析
- 设置合适的告警阈值

### 4. 安全考虑

- 限制工具的网络访问权限
- 使用最小权限原则配置数据库用户
- 定期轮换访问密钥和密码

## 实际使用场景

### 场景 1: 新集群部署验证

在部署新的 GreptimeDB 集群后，使用自检工具验证配置：

```bash
#!/bin/bash
# deploy-validation.sh

echo "=== GreptimeDB 集群部署验证 ==="

# 1. 检查 Metasrv 配置
echo "检查 Metasrv 配置..."
./stepstone metasrv -c /etc/greptime/metasrv.toml --output json > metasrv-check.json
if [ $? -eq 0 ]; then
    echo "✓ Metasrv 配置检查通过"
else
    echo "✗ Metasrv 配置检查失败"
    cat metasrv-check.json | jq '.details[] | select(.status == "FAIL")'
    exit 1
fi

# 2. 检查 Frontend 配置
echo "检查 Frontend 配置..."
./stepstone frontend -c /etc/greptime/frontend.toml --output json > frontend-check.json
if [ $? -eq 0 ]; then
    echo "✓ Frontend 配置检查通过"
else
    echo "✗ Frontend 配置检查失败"
    exit 1
fi

# 3. 检查 Datanode 配置（包含性能测试）
echo "检查 Datanode 配置..."
./stepstone datanode -c /etc/greptime/datanode.toml --include-performance --output json > datanode-check.json
if [ $? -eq 0 ]; then
    echo "✓ Datanode 配置检查通过"
    # 提取性能指标
    echo "性能测试结果:"
    cat datanode-check.json | jq '.details[] | select(.name | contains("Performance")) | {name, message, duration_ms}'
else
    echo "✗ Datanode 配置检查失败"
    exit 1
fi

echo "=== 集群部署验证完成 ==="
```

### 场景 2: 定期健康检查

设置 cron 任务进行定期检查：

```bash
# /etc/cron.d/greptime-health-check
# 每小时执行一次健康检查
0 * * * * greptime /usr/local/bin/greptime-health-check.sh

# greptime-health-check.sh
#!/bin/bash
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
LOG_DIR="/var/log/greptime/health-checks"
mkdir -p $LOG_DIR

# 执行检查
/usr/local/bin/stepstone metasrv -c /etc/greptime/metasrv.toml --output json > $LOG_DIR/metasrv_$TIMESTAMP.json
/usr/local/bin/stepstone frontend -c /etc/greptime/frontend.toml --output json > $LOG_DIR/frontend_$TIMESTAMP.json
/usr/local/bin/stepstone datanode -c /etc/greptime/datanode.toml --output json > $LOG_DIR/datanode_$TIMESTAMP.json

# 检查结果并发送告警
for component in metasrv frontend datanode; do
    result_file="$LOG_DIR/${component}_$TIMESTAMP.json"
    overall_result=$(cat $result_file | jq -r '.overall_result')

    if [ "$overall_result" != "PASS" ]; then
        # 发送告警邮件
        failed_checks=$(cat $result_file | jq '.details[] | select(.status == "FAIL") | .name' | tr '\n' ', ')
        echo "GreptimeDB $component 健康检查失败: $failed_checks" | \
            mail -s "GreptimeDB Health Check Alert" admin@company.com
    fi
done

# 清理旧日志（保留7天）
find $LOG_DIR -name "*.json" -mtime +7 -delete
```

### 场景 3: 故障诊断

当集群出现问题时，使用自检工具快速定位问题：

```bash
#!/bin/bash
# troubleshoot.sh

echo "=== GreptimeDB 故障诊断 ==="

# 启用详细日志
export RUST_LOG=debug

# 检查各组件状态
components=("metasrv" "frontend" "datanode")
configs=("/etc/greptime/metasrv.toml" "/etc/greptime/frontend.toml" "/etc/greptime/datanode.toml")

for i in "${!components[@]}"; do
    component="${components[$i]}"
    config="${configs[$i]}"

    echo "--- 检查 $component ---"

    # 执行检查并保存详细日志
    ./stepstone $component -c $config --verbose 2>&1 | tee ${component}_debug.log

    # 分析失败的检查项
    ./stepstone $component -c $config --output json > ${component}_result.json
    failed_count=$(cat ${component}_result.json | jq '.failed_checks')

    if [ "$failed_count" -gt 0 ]; then
        echo "发现 $failed_count 个失败的检查项:"
        cat ${component}_result.json | jq '.details[] | select(.status == "FAIL") | {name, message, suggestion}'

        # 根据错误类型提供具体建议
        if grep -q "ConnectionFailed" ${component}_debug.log; then
            echo "建议: 检查网络连接和服务状态"
        fi

        if grep -q "PermissionDenied" ${component}_debug.log; then
            echo "建议: 检查用户权限和认证配置"
        fi

        if grep -q "ConfigLoad" ${component}_debug.log; then
            echo "建议: 检查配置文件格式和必要参数"
        fi
    else
        echo "$component 检查通过"
    fi

    echo ""
done

echo "=== 诊断完成 ==="
```

### 场景 4: 性能基准测试

在不同环境中建立性能基线：

```bash
#!/bin/bash
# performance-benchmark.sh

echo "=== GreptimeDB 性能基准测试 ==="

# 测试不同的存储配置
storage_configs=(
    "/etc/greptime/datanode-s3.toml"
    "/etc/greptime/datanode-file.toml"
    "/etc/greptime/datanode-oss.toml"
)

for config in "${storage_configs[@]}"; do
    if [ -f "$config" ]; then
        storage_type=$(basename $config .toml | cut -d'-' -f2)
        echo "--- 测试 $storage_type 存储性能 ---"

        # 执行性能测试
        ./stepstone datanode -c $config --include-performance --output json > perf_${storage_type}.json

        # 提取关键性能指标
        echo "写入性能:"
        cat perf_${storage_type}.json | jq '.details[] | select(.name | contains("Write Performance")) | {size: (.name | split("(")[1] | split(")")[0]), message}'

        echo "读取性能:"
        cat perf_${storage_type}.json | jq '.details[] | select(.name | contains("Read Performance")) | {size: (.name | split("(")[1] | split(")")[0]), message}'

        echo "并发性能:"
        cat perf_${storage_type}.json | jq '.details[] | select(.name | contains("Concurrent")) | {name, message}'

        echo ""
    fi
done

# 生成性能报告
echo "=== 性能对比报告 ==="
for config in "${storage_configs[@]}"; do
    if [ -f "$config" ]; then
        storage_type=$(basename $config .toml | cut -d'-' -f2)
        echo "$storage_type 存储:"

        # 计算平均延迟
        avg_write_latency=$(cat perf_${storage_type}.json | jq '.details[] | select(.name | contains("Write Performance")) | .duration_ms' | awk '{sum+=$1; count++} END {print sum/count}')
        avg_read_latency=$(cat perf_${storage_type}.json | jq '.details[] | select(.name | contains("Read Performance")) | .duration_ms' | awk '{sum+=$1; count++} END {print sum/count}')

        echo "  平均写入延迟: ${avg_write_latency}ms"
        echo "  平均读取延迟: ${avg_read_latency}ms"
        echo ""
    fi
done
```

### 场景 5: 配置变更验证

在修改配置后验证变更的影响：

```bash
#!/bin/bash
# config-change-validation.sh

CONFIG_FILE="$1"
COMPONENT="$2"

if [ -z "$CONFIG_FILE" ] || [ -z "$COMPONENT" ]; then
    echo "用法: $0 <配置文件> <组件类型>"
    echo "示例: $0 /etc/greptime/metasrv.toml metasrv"
    exit 1
fi

echo "=== 配置变更验证 ==="

# 备份当前配置
cp $CONFIG_FILE ${CONFIG_FILE}.backup.$(date +%Y%m%d_%H%M%S)

# 执行变更前检查
echo "变更前检查..."
./stepstone $COMPONENT -c $CONFIG_FILE --output json > before_change.json
before_result=$(cat before_change.json | jq -r '.overall_result')
echo "变更前状态: $before_result"

if [ "$before_result" != "PASS" ]; then
    echo "警告: 配置变更前已存在问题"
    cat before_change.json | jq '.details[] | select(.status == "FAIL") | {name, message}'
fi

# 等待用户确认配置变更
echo "请修改配置文件，完成后按回车继续..."
read

# 执行变更后检查
echo "变更后检查..."
./stepstone $COMPONENT -c $CONFIG_FILE --output json > after_change.json
after_result=$(cat after_change.json | jq -r '.overall_result')
echo "变更后状态: $after_result"

# 对比变更前后的结果
echo "=== 变更影响分析 ==="

# 检查新增的失败项
new_failures=$(comm -23 <(cat after_change.json | jq -r '.details[] | select(.status == "FAIL") | .name' | sort) <(cat before_change.json | jq -r '.details[] | select(.status == "FAIL") | .name' | sort))

if [ -n "$new_failures" ]; then
    echo "新增失败项:"
    echo "$new_failures"
    echo ""
    echo "建议回滚配置变更"
    exit 1
fi

# 检查修复的问题
fixed_issues=$(comm -23 <(cat before_change.json | jq -r '.details[] | select(.status == "FAIL") | .name' | sort) <(cat after_change.json | jq -r '.details[] | select(.status == "FAIL") | .name' | sort))

if [ -n "$fixed_issues" ]; then
    echo "已修复问题:"
    echo "$fixed_issues"
fi

# 性能对比（如果是 datanode）
if [ "$COMPONENT" = "datanode" ]; then
    echo "=== 性能对比 ==="

    # 提取性能数据进行对比
    before_perf=$(cat before_change.json | jq '.details[] | select(.name | contains("Performance")) | {name, duration_ms}')
    after_perf=$(cat after_change.json | jq '.details[] | select(.name | contains("Performance")) | {name, duration_ms}')

    echo "变更前性能:"
    echo "$before_perf"
    echo ""
    echo "变更后性能:"
    echo "$after_perf"
fi

echo "=== 配置变更验证完成 ==="
```

## 扩展开发指南

### 添加新的检查项

1. **定义错误类型**（在 `src/error.rs` 中）:

```rust
#[snafu(display("Custom check failed: {}", message))]
CustomCheckFailed {
    message: String,
    #[snafu(implicit)]
    location: Location,
},
```

2. **实现检查逻辑**（在相应的检查器中）:

```rust
async fn check_custom_feature(&self) -> CheckResult {
    let mut details = Vec::new();
    let start = Instant::now();

    // 执行自定义检查逻辑
    match self.perform_custom_check().await {
        Ok(result) => {
            details.push(CheckDetail::pass(
                "Custom Feature Check".to_string(),
                format!("Custom check passed: {}", result),
                Some(start.elapsed()),
            ));
        }
        Err(e) => {
            details.push(CheckDetail::fail(
                "Custom Feature Check".to_string(),
                format!("Custom check failed: {}", e),
                Some(start.elapsed()),
                Some("Check custom feature configuration".to_string()),
            ));
        }
    }

    CheckResult::from_details(details)
}
```

3. **集成到主检查流程**:

```rust
async fn check(&self) -> CheckResult {
    let mut all_details = Vec::new();

    // 现有检查...

    // 添加新的检查
    let custom_result = self.check_custom_feature().await;
    all_details.extend(custom_result.details);

    CheckResult::from_details(all_details)
}
```

### 添加新的存储后端支持

1. **定义配置结构**（在 `src/config.rs` 中）:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomStorageConfig {
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub region: Option<String>,
}

impl StorageConfig {
    pub fn as_custom_storage_config(&self) -> crate::error::Result<CustomStorageConfig> {
        Ok(CustomStorageConfig {
            endpoint: self.config.get("endpoint")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| crate::error::Error::InvalidConfig {
                    message: "Missing endpoint for custom storage".to_string(),
                })?,
            // ... 其他字段
        })
    }
}
```

2. **实现检查逻辑**（在 `src/datanode.rs` 中）:

```rust
async fn check_custom_storage(&self) -> CheckResult {
    let mut details = Vec::new();

    // 解析配置
    let config = match self.config.storage.as_custom_storage_config() {
        Ok(config) => config,
        Err(e) => {
            details.push(CheckDetail::fail(
                "Custom Storage Configuration".to_string(),
                format!("Failed to parse configuration: {}", e),
                None,
                Some("Check custom storage configuration parameters".to_string()),
            ));
            return CheckResult::from_details(details);
        }
    };

    // 实现具体的检查逻辑
    // ...

    CheckResult::from_details(details)
}
```

3. **更新主检查逻辑**:

```rust
async fn check_object_storage(&self) -> CheckResult {
    match self.config.storage.storage_type.as_str() {
        "S3" => self.check_s3_storage().await,
        "CustomStorage" => self.check_custom_storage().await,
        // ... 其他存储类型
        unknown => CheckResult::failure(
            format!("Unknown storage type: {}", unknown),
            vec![CheckDetail::fail(
                "Storage Type".to_string(),
                format!("Unsupported storage type: {}", unknown),
                None,
                Some("Supported types: S3, CustomStorage, File".to_string()),
            )],
        ),
    }
}
```

这个自检工具为 GreptimeDB 集群的部署和维护提供了强有力的支持，能够快速发现和诊断配置问题，确保集群的稳定运行。通过合理的配置和集成，可以大大提高系统的可靠性和可维护性。
