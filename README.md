# Stepstone - GreptimeDB Cluster Self-Test Tool

A command-line tool to validate GreptimeDB cluster component dependencies and configurations before deployment. Stepstone helps identify common configuration issues, connectivity problems, and performance bottlenecks that could prevent a GreptimeDB cluster from starting successfully.

## Features

- **Metasrv Validation**: Check etcd, PostgreSQL, or MySQL connectivity and permissions
- **Frontend Validation**: Verify metasrv connectivity and server configuration
- **Datanode Validation**: Test metasrv connectivity and object storage (S3, OSS, Azure Blob, GCS, File)
- **Performance Testing**: Optional performance benchmarks for object storage
- **Multiple Output Formats**: Human-readable and JSON output
- **Comprehensive Error Reporting**: Detailed error messages with suggestions

## Installation

### Build from Source

```bash
git clone https://github.com/GreptimeTeam/stepstone.git
cd stepstone
cargo build --release
```

The binary will be available at `target/release/stepstone`.

## Usage

### Basic Commands

```bash
# Check metasrv configuration
stepstone metasrv -c /path/to/metasrv.toml

# Check frontend configuration
stepstone frontend -c /path/to/frontend.toml

# Check datanode configuration
stepstone datanode -c /path/to/datanode.toml
```

### Advanced Options

```bash
# Enable verbose output
stepstone metasrv -c config.toml --verbose

# Include performance tests for datanode
stepstone datanode -c config.toml --include-performance

# Output results in JSON format
stepstone frontend -c config.toml --output json
```

## Configuration Examples

### Metasrv Configuration (TOML)

#### Etcd Store
```toml
[store]
store_type = "etcd_store"
store_addrs = ["127.0.0.1:2379", "127.0.0.1:2380"]
store_key_prefix = "/greptime"
max_txn_ops = 128
```

#### PostgreSQL Store
```toml
[store]
store_type = "postgres_store"
store_addrs = ["postgresql://user:password@localhost:5432/greptime"]
meta_table_name = "greptime_metasrv"
```

#### MySQL Store
```toml
[store]
store_type = "mysql_store"
store_addrs = ["mysql://user:password@localhost:3306/greptime"]
meta_table_name = "greptime_metasrv"
```

### Frontend Configuration (TOML)

```toml
metasrv_addrs = ["127.0.0.1:3002"]

[server]
addr = "0.0.0.0:4000"
http_addr = "0.0.0.0:4001"
grpc_addr = "0.0.0.0:4002"
```

### Datanode Configuration (TOML)

#### S3 Storage
```toml
metasrv_addrs = ["127.0.0.1:3002"]

[storage]
storage_type = "S3"
bucket = "my-greptime-bucket"
root = "data/"
access_key_id = "your-access-key"
secret_access_key = "your-secret-key"
endpoint = "https://s3.amazonaws.com"
region = "us-east-1"
```

#### File Storage
```toml
metasrv_addrs = ["127.0.0.1:3002"]

[storage]
storage_type = "File"
root = "/var/lib/greptimedb/data"
```

## Check Results

### Human-Readable Output

```
GreptimeDB Self-Test Report
===========================

Component: Metasrv
Configuration: /path/to/metasrv.toml
Total Duration: 125ms

✓ Etcd Connection                    [PASS] (125ms) - Successfully connected to etcd endpoints: ["127.0.0.1:2379"]
✓ Etcd PUT Operation                 [PASS] - PUT operation successful
✓ Etcd GET Operation                 [PASS] - GET operation successful and data matches
✓ Etcd DELETE Operation              [PASS] - DELETE operation successful

Overall Result: PASS
```

### JSON Output

```json
{
  "component": "metasrv",
  "config_file": "/path/to/metasrv.toml",
  "timestamp": "2025-08-27T10:30:00Z",
  "overall_result": "PASS",
  "total_checks": 4,
  "passed_checks": 4,
  "failed_checks": 0,
  "warning_checks": 0,
  "total_duration_ms": 125,
  "message": "All checks passed (4 passed)",
  "details": [
    {
      "item": "Etcd Connection",
      "status": "PASS",
      "message": "Successfully connected to etcd endpoints: [\"127.0.0.1:2379\"]",
      "duration_ms": 125,
      "suggestion": null
    }
  ]
}
```

## Supported Storage Types

### Object Storage
- **S3**: Amazon S3 and S3-compatible services
- **OSS**: Alibaba Cloud Object Storage Service (planned)
- **Azure Blob**: Microsoft Azure Blob Storage (planned)
- **GCS**: Google Cloud Storage (planned)
- **File**: Local file system storage

### Metadata Storage
- **Etcd**: Distributed key-value store
- **PostgreSQL**: Relational database
- **MySQL**: Relational database
- **Memory**: In-memory storage (for testing)

## Exit Codes

- `0`: All checks passed
- `1`: Some checks failed or error occurred

## Troubleshooting

### Common Issues

1. **Connection Timeout**
   - Check network connectivity
   - Verify service is running and accessible
   - Check firewall settings

2. **Permission Denied**
   - Verify credentials are correct
   - Check user permissions
   - Ensure proper access policies

3. **Configuration Errors**
   - Validate TOML syntax
   - Check required fields are present
   - Verify file paths exist

### Getting Help

For detailed error information, use the `--verbose` flag:

```bash
stepstone metasrv -c config.toml --verbose
```


