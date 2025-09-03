# Stepstone - GreptimeDB Cluster Self-Test Tool

A command-line tool to validate GreptimeDB cluster component dependencies and configurations before deployment. Stepstone helps identify common configuration issues, connectivity problems, and performance bottlenecks that could prevent a GreptimeDB cluster from starting successfully.

## Features

- **Metasrv Validation**: Check etcd and PostgreSQL connectivity, permissions, and operations
- **Frontend Validation**: Verify metasrv connectivity and server configuration
- **Datanode Validation**: Test metasrv connectivity and storage (S3, File system) with performance benchmarks
- **Advanced S3 Testing**: Comprehensive S3 permissions, performance, and concurrent operations testing
- **Multiple Output Formats**: Human-readable and JSON output
- **Comprehensive Error Reporting**: Detailed error messages with actionable suggestions
- **Real Cluster Testing**: Validate running GreptimeDB clusters deployed via Docker Compose

## Installation

### Build from Source

```bash
git clone <repository-url>
cd stepstone
cargo build --release
```

The binary will be available at `target/release/stepstone`.

### Prerequisites

- **Rust**: 1.70+ (see `rust-toolchain.toml`)
- **Dependencies**: etcd, PostgreSQL, or MinIO/S3 (depending on your configuration)

## Usage

### Basic Commands

```bash
# Check metasrv configuration
stepstone metasrv -c /path/to/metasrv.toml

# Check frontend configuration
stepstone frontend -c /path/to/frontend.toml

# Check datanode configuration
stepstone datanode -c /path/to/datanode.toml

stepstone metasrv -c test-metasrv.toml

stepstone metasrv -c test-metasrv-postgres.toml

stepstone frontend -c test-frontend.toml

stepstone datanode -c test-datanode.toml
```

### Advanced Options

```bash
# Output results in JSON format
stepstone metasrv -c config.toml --output json
stepstone frontend -c config.toml --output json
stepstone datanode -c config.toml --output json
```

### Example Configurations

The repository includes three example configuration files:
- `metasrv.example.toml` - Metasrv configuration with etcd backend
- `frontend.example.toml` - Frontend configuration
- `datanode.example.toml` - Datanode configuration with file storage

## Configuration Examples

### Metasrv Configuration

#### Etcd Backend
```toml
data_home = "./data/metasrv"
store_addrs = ["127.0.0.1:2379"]
backend = "etcd_store"

[grpc]
bind_addr = "0.0.0.0:3002"
server_addr = "127.0.0.1:3002"

[http]
addr = "0.0.0.0:3000"
```

#### PostgreSQL Backend
```toml
data_home = "./data/metasrv"
store_addrs = ["postgresql://user:password@localhost:5432/greptime_meta"]
backend = "postgres_store"
meta_table_name = "greptime_metasrv"

[grpc]
bind_addr = "0.0.0.0:3002"
server_addr = "127.0.0.1:3002"
```

### Frontend Configuration

```toml
[http]
addr = "0.0.0.0:4000"

[grpc]
bind_addr = "0.0.0.0:4001"
server_addr = "127.0.0.1:4001"

[mysql]
enable = true
addr = "0.0.0.0:4002"

[postgres]
enable = true
addr = "0.0.0.0:4003"

[meta_client]
metasrv_addrs = ["127.0.0.1:3002"]
timeout = "3s"
```

### Datanode Configuration

#### S3 Storage
```toml
node_id = 0

[grpc]
bind_addr = "0.0.0.0:3001"
server_addr = "127.0.0.1:3001"

[http]
addr = "0.0.0.0:5000"

[meta_client]
metasrv_addrs = ["127.0.0.1:3002"]
timeout = "3s"

[storage]
type = "S3"
bucket = "my-greptime-bucket"
root = "data/"
access_key_id = "your-access-key"
secret_access_key = "your-secret-key"
endpoint = "https://s3.amazonaws.com"
region = "us-east-1"
```

#### File Storage
```toml
node_id = 0

[meta_client]
metasrv_addrs = ["127.0.0.1:3002"]

[storage]
type = "File"
data_home = "/var/lib/greptimedb/data"
```

## Check Results

### Human-Readable Output

```
GreptimeDB Self-Test Report
===========================

Component: Datanode
Configuration: datanode.example.toml
Total Duration: 2.5s

✓ Metasrv Connectivity               [PASS] (2ms) - Successfully connected to metasrv at 127.0.0.1:3002
✓ S3 Client Creation                 [PASS] (330ms) - S3 client created successfully
✓ S3 Bucket List Permission          [PASS] (15ms) - Successfully listed bucket contents
✓ S3 PUT Operation                   [PASS] - PUT operation successful
✓ S3 GET Operation                   [PASS] - GET operation successful and data matches
✓ S3 DELETE Operation                [PASS] - DELETE operation successful
✓ S3 64MB File Write Performance     [PASS] (156ms) - 64MB write: 156ms (409.20 MB/s)
✓ S3 64MB File Read Performance      [PASS] (35ms) - 64MB read: 35ms (1809.75 MB/s)
✓ S3 1GB File Write Performance      [PASS] (2069ms) - 1GB write: 2069ms (494.74 MB/s)
✓ S3 Concurrent Operations           [PASS] (84ms) - 100 concurrent writes: 84ms (1189.7 ops/s)

Overall Result: PASS
```

### JSON Output

```json
{
  "component": "Datanode",
  "config_file": "datanode.example.toml",
  "timestamp": "2025-09-03T02:13:10.523305+00:00",
  "overall_result": "PASS",
  "total_checks": 10,
  "passed_checks": 10,
  "failed_checks": 0,
  "warning_checks": 0,
  "total_duration_ms": 2500,
  "message": "All checks passed (10 passed)",
  "details": [
    {
      "item": "S3 64MB File Write Performance",
      "status": "PASS",
      "message": "64MB write: 156ms (409.20 MB/s)",
      "duration_ms": 156,
      "suggestion": null
    },
    {
      "item": "S3 Concurrent Operations",
      "status": "PASS",
      "message": "100 concurrent writes: 84ms (1189.7 ops/s)",
      "duration_ms": 84,
      "suggestion": null
    }
  ]
}
```

## Supported Storage Types

### Object Storage
- **S3**: Amazon S3 and S3-compatible services (MinIO, etc.)
  - Comprehensive permission testing (ListBucket, GetObject, PutObject, DeleteObject)
  - Performance benchmarks (64MB, 1GB files, 100 concurrent operations)
  - Error detection (invalid credentials, missing buckets, access denied)
- **File**: Local file system storage
  - Directory existence and write permission validation

### Metadata Storage
- **Etcd**: Distributed key-value store
  - Connection testing and CRUD operations validation
- **PostgreSQL**: Relational database
  - Connection, table existence, and read/write permission testing
  - Automatic table creation permission validation

## Exit Codes

- `0`: All checks passed
- `1`: Some checks failed or error occurred

## Testing Real Clusters

Stepstone can validate running GreptimeDB clusters deployed via Docker Compose:

```bash
# Start the official GreptimeDB cluster
GREPTIMEDB_VERSION=v0.16.0 docker compose -f cluster-with-etcd.yaml up -d

# Test the running cluster components
stepstone metasrv -c cluster-metasrv-config.toml
stepstone frontend -c cluster-frontend-config.toml
stepstone datanode -c cluster-datanode-config.toml
```

## Troubleshooting

### Common Issues

1. **Connection Timeout**
   - Check network connectivity to etcd/PostgreSQL/S3 endpoints
   - Verify services are running and accessible on specified ports
   - Check firewall settings and security groups

2. **S3 Permission Denied**
   - Verify access_key_id and secret_access_key are correct
   - Check bucket exists and is accessible
   - Ensure IAM policies grant required permissions (ListBucket, GetObject, PutObject, DeleteObject)

3. **PostgreSQL Permission Issues**
   - Verify database user has sufficient privileges
   - Check if metadata table exists or user can create tables
   - Ensure connection string format is correct

4. **Configuration Errors**
   - Validate TOML syntax using `taplo check *.toml`
   - Check required fields are present and correctly named
   - Verify file paths and network addresses are accessible

## Performance Benchmarks

Stepstone includes comprehensive performance testing for S3 storage:

- **64MB File Performance**: Tests typical time-series data chunk sizes
- **1GB File Performance**: Tests large file handling capabilities
- **Concurrent Operations**: Tests 100 simultaneous operations for high-throughput scenarios
- **Latency Measurement**: Precise timing for all operations
- **Throughput Calculation**: MB/s and ops/s metrics

Example performance results:
- 64MB write: 409 MB/s, read: 1809 MB/s
- 1GB write: 494 MB/s
- 100 concurrent operations: 1189 ops/s



## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.
