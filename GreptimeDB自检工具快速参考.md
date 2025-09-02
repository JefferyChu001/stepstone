# GreptimeDB 自检工具快速参考

## 快速开始

### 安装

```bash
# 从源码编译
git clone https://github.com/GreptimeTeam/stepstone.git
cd stepstone
cargo build --release
sudo cp target/release/stepstone /usr/local/bin/
```

### 基本用法

```bash
# 检查 Metasrv
stepstone metasrv -c /path/to/metasrv.toml

# 检查 Frontend  
stepstone frontend -c /path/to/frontend.toml

# 检查 Datanode
stepstone datanode -c /path/to/datanode.toml

# 包含性能测试
stepstone datanode -c /path/to/datanode.toml --include-performance

# JSON 输出
stepstone metasrv -c /path/to/metasrv.toml --output json
```

## 配置文件模板

### Metasrv 配置

#### Etcd 存储
```toml
[store]
store_type = "etcd_store"
store_addrs = ["127.0.0.1:2379", "127.0.0.1:2380"]
store_key_prefix = "/greptime"
max_txn_ops = 128
```

#### PostgreSQL 存储
```toml
[store]
store_type = "postgres_store"
store_addrs = ["postgresql://user:pass@localhost:5432/greptime"]
meta_table_name = "greptime_metasrv"
```

#### MySQL 存储
```toml
[store]
store_type = "mysql_store"
store_addrs = ["mysql://user:pass@localhost:3306/greptime"]
meta_table_name = "greptime_metasrv"
```

### Frontend 配置

```toml
metasrv_addrs = ["127.0.0.1:3002", "127.0.0.1:3003"]

[server]
addr = "0.0.0.0:4000"
timeout = "30s"
```

### Datanode 配置

#### S3 存储
```toml
metasrv_addrs = ["127.0.0.1:3002"]

[storage]
storage_type = "S3"

[storage.config]
bucket = "my-bucket"
root = "/greptime-data"
access_key_id = "your-key"
secret_access_key = "your-secret"
endpoint = "https://s3.amazonaws.com"
region = "us-east-1"
```

#### 文件存储
```toml
metasrv_addrs = ["127.0.0.1:3002"]

[storage]
storage_type = "File"

[storage.config]
root = "/var/lib/greptime/data"
```

## 检查项目说明

### Metasrv 检查项目

| 检查项目 | 说明 | 失败原因 |
|---------|------|---------|
| 存储后端连接 | 验证到 Etcd/PostgreSQL/MySQL 的连通性 | 服务未启动、网络问题、认证失败 |
| PUT 操作 | 测试写入操作 | 权限不足、存储空间不足 |
| GET 操作 | 测试读取操作 | 数据不一致、网络问题 |
| DELETE 操作 | 测试删除操作 | 权限不足 |
| 元数据表检查 | 验证必要表结构（数据库存储） | 表不存在、权限不足 |

### Frontend 检查项目

| 检查项目 | 说明 | 失败原因 |
|---------|------|---------|
| Metasrv 连通性 | TCP 连接测试 | 网络不通、服务未启动、端口错误 |
| 地址解析 | 验证地址格式 | 地址格式错误、缺少端口 |
| 服务器配置 | 检查监听配置 | 地址冲突、权限不足 |

### Datanode 检查项目

| 检查项目 | 说明 | 失败原因 |
|---------|------|---------|
| Metasrv 连通性 | 与 Frontend 相同 | 同 Frontend |
| 存储配置解析 | 验证存储配置 | 配置格式错误、必要参数缺失 |
| 存储连通性 | 测试存储访问 | 认证失败、网络问题、权限不足 |
| 基本操作 | PUT/GET/LIST/DELETE | 权限不足、存储空间不足 |
| 性能测试 | 延迟和吞吐量测试 | 性能不达标、网络延迟高 |

## 常见错误和解决方案

### 连接错误

```
错误: ConnectionFailed { message: "Connection refused" }
解决: 
1. 检查服务是否启动
2. 验证网络连通性
3. 确认端口配置正确
```

### 认证错误

```
错误: PermissionDenied { message: "Authentication failed" }
解决:
1. 检查用户名密码
2. 验证访问密钥
3. 确认用户权限
```

### 配置错误

```
错误: ConfigLoad { message: "Failed to parse TOML" }
解决:
1. 检查配置文件语法
2. 验证必要参数
3. 确认文件路径正确
```

### 地址解析错误

```
错误: InvalidPort { address: "localhost:abc" }
解决:
1. 使用正确的端口号
2. 检查地址格式 (host:port)
3. 移除多余的路径部分
```

## 输出格式

### 人类可读格式

```
=== GreptimeDB Metasrv Configuration Check ===
Config File: /path/to/metasrv.toml
Timestamp: 2024-01-15T10:30:00Z

✓ Etcd Connection: Successfully connected (Duration: 45ms)
✓ Etcd PUT Operation: Test data written (Duration: 12ms)
✓ Etcd GET Operation: Data verified (Duration: 8ms)
✓ Etcd DELETE Operation: Test data cleaned (Duration: 10ms)

Overall Result: PASS
Total Checks: 4 | Passed: 4 | Failed: 0 | Warnings: 0
Total Duration: 75ms
```

### JSON 格式

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
      "message": "Successfully connected to etcd",
      "duration_ms": 45,
      "suggestion": null
    }
  ]
}
```

## 自动化脚本示例

### 部署验证脚本

```bash
#!/bin/bash
# 验证新部署的集群
components=("metasrv" "frontend" "datanode")
configs=("/etc/greptime/metasrv.toml" "/etc/greptime/frontend.toml" "/etc/greptime/datanode.toml")

for i in "${!components[@]}"; do
    echo "检查 ${components[$i]}..."
    if stepstone ${components[$i]} -c ${configs[$i]}; then
        echo "✓ ${components[$i]} 检查通过"
    else
        echo "✗ ${components[$i]} 检查失败"
        exit 1
    fi
done
echo "集群部署验证完成"
```

### 健康检查脚本

```bash
#!/bin/bash
# 定期健康检查
LOG_DIR="/var/log/greptime/health"
mkdir -p $LOG_DIR

for component in metasrv frontend datanode; do
    stepstone $component -c /etc/greptime/$component.toml --output json > $LOG_DIR/$component.json
    
    if [ $? -ne 0 ]; then
        echo "GreptimeDB $component 健康检查失败" | mail -s "Health Check Alert" admin@company.com
    fi
done
```

## 性能基准参考

### S3 存储性能基准

| 数据大小 | 写入延迟 | 读取延迟 | 写入吞吐量 | 读取吞吐量 |
|---------|---------|---------|-----------|-----------|
| 1KB | < 50ms | < 30ms | > 20 KB/s | > 30 KB/s |
| 1MB | < 200ms | < 150ms | > 5 MB/s | > 7 MB/s |
| 10MB | < 2s | < 1.5s | > 5 MB/s | > 7 MB/s |

### 文件存储性能基准

| 数据大小 | 写入延迟 | 读取延迟 | 写入吞吐量 | 读取吞吐量 |
|---------|---------|---------|-----------|-----------|
| 1KB | < 5ms | < 2ms | > 200 KB/s | > 500 KB/s |
| 1MB | < 50ms | < 20ms | > 20 MB/s | > 50 MB/s |
| 10MB | < 500ms | < 200ms | > 20 MB/s | > 50 MB/s |

## 故障排除清单

### 部署前检查

- [ ] 所有服务已启动
- [ ] 网络连通性正常
- [ ] 防火墙规则配置正确
- [ ] DNS 解析正常
- [ ] 时间同步正确

### 配置检查

- [ ] 配置文件语法正确
- [ ] 必要参数已配置
- [ ] 地址格式正确
- [ ] 认证信息有效
- [ ] 权限配置适当

### 性能检查

- [ ] 存储性能满足要求
- [ ] 网络延迟在可接受范围
- [ ] 并发能力足够
- [ ] 资源使用合理

## 环境变量

```bash
# 启用详细日志
export RUST_LOG=debug

# 设置超时时间
export STEPSTONE_TIMEOUT=60

# 设置重试次数
export STEPSTONE_RETRY_COUNT=3
```

## 集成示例

### Docker Compose

```yaml
version: '3.8'
services:
  health-check:
    image: stepstone:latest
    volumes:
      - ./configs:/configs
    command: |
      sh -c "
        stepstone metasrv -c /configs/metasrv.toml &&
        stepstone frontend -c /configs/frontend.toml &&
        stepstone datanode -c /configs/datanode.toml
      "
```

### Kubernetes Job

```yaml
apiVersion: batch/v1
kind: Job
metadata:
  name: greptime-health-check
spec:
  template:
    spec:
      containers:
      - name: stepstone
        image: stepstone:latest
        command: ["stepstone"]
        args: ["metasrv", "-c", "/configs/metasrv.toml"]
        volumeMounts:
        - name: config
          mountPath: /configs
      volumes:
      - name: config
        configMap:
          name: greptime-config
      restartPolicy: Never
```

这个快速参考指南提供了使用 GreptimeDB 自检工具的所有必要信息，帮助用户快速上手并有效地进行集群健康检查。
