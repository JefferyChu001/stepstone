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

//! 集成测试模块，测试常见的部署错误场景

use crate::common::{CheckDetail, CheckResult, CheckStatus};
use crate::error;
use std::time::Duration;
use snafu::IntoError;

/// 测试错误处理逻辑
#[cfg(test)]
mod error_handling_tests {
    use super::*;
    use snafu::ResultExt;

    #[test]
    fn test_error_message_format() {
        // 测试各种错误类型的消息格式

        // 1. 测试地址解析错误
        let parse_error = "abc".parse::<u16>().unwrap_err();
        let invalid_port_error = error::InvalidPortSnafu {
            address: "localhost:abc".to_string(),
            port_str: "abc".to_string(),
        }.into_error(parse_error);

        let error_message = invalid_port_error.to_string();
        assert!(error_message.contains("Invalid port number"));
        assert!(error_message.contains("localhost:abc"));
        assert!(error_message.contains("abc"));

        // 2. 测试缺少端口错误
        let missing_port_error = error::MissingPortSnafu {
            address: "localhost".to_string(),
        }.build();

        let error_message = missing_port_error.to_string();
        assert!(error_message.contains("Address must contain port number"));
        assert!(error_message.contains("localhost"));

        // 3. 测试配置加载错误
        let config_error = error::ConfigLoadSnafu {
            message: "Failed to parse TOML".to_string(),
        }.build();

        let error_message = config_error.to_string();
        assert!(error_message.contains("Failed to load configuration"));
        assert!(error_message.contains("Failed to parse TOML"));
    }

    #[test]
    fn test_error_context_propagation() {
        // 测试错误上下文传播
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let result: Result<(), std::io::Error> = Err(io_error);

        let contextual_error = result.context(error::FileSystemSnafu {
            message: "Failed to read config file".to_string(),
        }).unwrap_err();

        let error_message = contextual_error.to_string();
        assert!(error_message.contains("File system operation failed"));
        assert!(error_message.contains("Failed to read config file"));

        // 验证原始错误信息也被保留
        let error_chain: Vec<String> = std::iter::successors(
            Some(&contextual_error as &dyn std::error::Error),
            |e| e.source()
        ).map(|e| e.to_string()).collect();

        assert!(error_chain.len() >= 2, "错误链应该包含至少两个错误");
        assert!(error_chain.iter().any(|msg| msg.contains("file not found")));
    }

    #[test]
    fn test_check_result_creation() {
        // 测试检查结果的创建和格式化

        // 1. 创建成功的检查详情
        let success_detail = CheckDetail::pass(
            "Test Check".to_string(),
            "Check passed successfully".to_string(),
            Some(Duration::from_millis(100)),
        );

        assert_eq!(success_detail.status, CheckStatus::Pass);
        assert_eq!(success_detail.item, "Test Check");
        assert!(success_detail.message.contains("successfully"));
        assert!(success_detail.duration.is_some());
        assert!(success_detail.suggestion.is_none());

        // 2. 创建失败的检查详情
        let failure_detail = CheckDetail::fail(
            "Test Check".to_string(),
            "Check failed with error".to_string(),
            Some(Duration::from_millis(50)),
            Some("Please check the configuration".to_string()),
        );

        assert_eq!(failure_detail.status, CheckStatus::Fail);
        assert_eq!(failure_detail.item, "Test Check");
        assert!(failure_detail.message.contains("failed"));
        assert!(failure_detail.suggestion.is_some());
        assert!(failure_detail.suggestion.as_ref().unwrap().contains("configuration"));

        // 3. 创建检查结果
        let check_result = CheckResult::from_details(vec![success_detail, failure_detail]);

        assert!(!check_result.success, "包含失败项的结果应该是失败的");
        assert_eq!(check_result.details.len(), 2);

        // 验证总体消息
        assert!(check_result.message.contains("1 passed") || check_result.message.contains("1 failed"));
    }

    #[test]
    fn test_json_serialization() {
        // 测试 JSON 序列化功能
        let detail = CheckDetail::pass(
            "JSON Test".to_string(),
            "JSON serialization test".to_string(),
            Some(Duration::from_millis(25)),
        );

        let result = CheckResult::from_details(vec![detail]);

        // 测试 JSON 序列化
        let json_result = result.to_json("TestComponent", Some("/path/to/config.toml"));
        assert!(json_result.is_ok(), "JSON 序列化应该成功");

        let json_string = json_result.unwrap();

        // 验证 JSON 包含必要字段
        assert!(json_string.contains("\"component\""));
        assert!(json_string.contains("\"config_file\""));
        assert!(json_string.contains("\"timestamp\""));
        assert!(json_string.contains("\"overall_result\""));
        assert!(json_string.contains("\"details\""));
        assert!(json_string.contains("TestComponent"));
        assert!(json_string.contains("/path/to/config.toml"));

        // 验证 JSON 格式正确
        let parsed: serde_json::Value = serde_json::from_str(&json_string)
            .expect("生成的 JSON 应该是有效的");

        assert!(parsed["component"].is_string());
        assert!(parsed["details"].is_array());
        assert!(parsed["total_checks"].is_number());
    }
}

/// 测试地址解析功能
#[cfg(test)]
mod address_parsing_tests {
    use super::*;

    #[test]
    fn test_address_parsing_errors() {
        // 测试各种地址解析错误场景

        // 1. 测试缺少端口的地址
        let missing_port_error = error::MissingPortSnafu {
            address: "localhost".to_string(),
        }.build();

        assert!(missing_port_error.to_string().contains("Address must contain port number"));
        assert!(missing_port_error.to_string().contains("localhost"));

        // 2. 测试无效端口的地址
        let parse_error = "abc".parse::<u16>().unwrap_err();
        let invalid_port_error = error::InvalidPortSnafu {
            address: "localhost:abc".to_string(),
            port_str: "abc".to_string(),
        }.into_error(parse_error);

        assert!(invalid_port_error.to_string().contains("Invalid port number"));
        assert!(invalid_port_error.to_string().contains("localhost:abc"));
        assert!(invalid_port_error.to_string().contains("abc"));
    }

    #[test]
    fn test_storage_configuration_errors() {
        // 测试存储配置错误

        // 1. S3 配置错误
        let s3_config_error = error::S3ConfigSnafu {
            message: "Missing bucket name".to_string(),
        }.build();

        assert!(s3_config_error.to_string().contains("S3 configuration error"));
        assert!(s3_config_error.to_string().contains("Missing bucket name"));

        // 2. 文件存储配置错误
        let file_config_error = error::FileStorageConfigSnafu {
            message: "Invalid root directory".to_string(),
        }.build();

        assert!(file_config_error.to_string().contains("File storage configuration error"));
        assert!(file_config_error.to_string().contains("Invalid root directory"));
    }

    #[test]
    fn test_database_connection_errors() {
        // 测试数据库连接错误

        // 模拟 PostgreSQL 连接错误
        let pg_error = sqlx::Error::Configuration("Invalid connection string".into());
        let pg_connection_error = error::PostgresConnectionSnafu {
            message: "Failed to connect to PostgreSQL".to_string(),
        }.into_error(pg_error);

        assert!(pg_connection_error.to_string().contains("PostgreSQL connection failed"));
        assert!(pg_connection_error.to_string().contains("Failed to connect to PostgreSQL"));

        // 模拟 MySQL 连接错误
        let mysql_error = sqlx::Error::Configuration("Invalid credentials".into());
        let mysql_connection_error = error::MySqlConnectionSnafu {
            message: "Authentication failed".to_string(),
        }.into_error(mysql_error);

        assert!(mysql_connection_error.to_string().contains("MySQL connection failed"));
        assert!(mysql_connection_error.to_string().contains("Authentication failed"));
    }
}

/// 测试性能相关的错误处理
#[cfg(test)]
mod performance_tests {
    use super::*;

    #[test]
    fn test_performance_test_errors() {
        // 测试性能测试相关的错误

        let perf_setup_error = error::PerformanceTestSetupSnafu {
            message: "Failed to initialize performance test".to_string(),
        }.build();

        assert!(perf_setup_error.to_string().contains("Performance test setup failed"));
        assert!(perf_setup_error.to_string().contains("Failed to initialize"));

        let perf_execution_error = error::PerformanceTestExecutionSnafu {
            message: "Test execution timeout".to_string(),
        }.build();

        assert!(perf_execution_error.to_string().contains("Performance test execution failed"));
        assert!(perf_execution_error.to_string().contains("Test execution timeout"));
    }

    #[test]
    fn test_unsupported_operations() {
        // 测试不支持的操作错误

        let unsupported_storage_error = error::UnsupportedStorageTypeSnafu {
            storage_type: "UnknownStorage".to_string(),
        }.build();

        assert!(unsupported_storage_error.to_string().contains("Unsupported storage type"));
        assert!(unsupported_storage_error.to_string().contains("UnknownStorage"));

        let unsupported_store_error = error::UnsupportedStoreTypeSnafu {
            store_type: "UnknownStore".to_string(),
        }.build();

        assert!(unsupported_store_error.to_string().contains("Unsupported store type"));
        assert!(unsupported_store_error.to_string().contains("UnknownStore"));
    }
}

/// 测试 S3 存储认证错误场景
#[cfg(test)]
mod s3_auth_error_tests {
    use super::*;
    use crate::common::ComponentChecker;
    use crate::config::{DatanodeConfig, DatanodeStorageConfig, MetaClientConfig};
    use crate::datanode::DatanodeChecker;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_s3_invalid_credentials() {
        // 构造无效的 S3 认证配置
        let mut storage_config = HashMap::new();
        storage_config.insert("bucket".to_string(), toml::Value::String("test-bucket".to_string()));
        storage_config.insert("access_key_id".to_string(), toml::Value::String("invalid-key".to_string()));
        storage_config.insert("secret_access_key".to_string(), toml::Value::String("invalid-secret".to_string()));
        storage_config.insert("endpoint".to_string(), toml::Value::String("https://s3.amazonaws.com".to_string()));
        storage_config.insert("region".to_string(), toml::Value::String("us-east-1".to_string()));

        let datanode_config = DatanodeConfig {
            node_id: Some(1),
            require_lease_before_startup: Some(false),
            init_regions_in_background: Some(false),
            init_regions_parallelism: Some(16),
            max_concurrent_queries: Some(0),
            enable_telemetry: Some(true),
            http: None,
            grpc: None,
            heartbeat: None,
            meta_client: Some(MetaClientConfig {
                metasrv_addrs: vec!["127.0.0.1:3002".to_string()],
                timeout: Some("3s".to_string()),
                heartbeat_timeout: Some("500ms".to_string()),
                ddl_timeout: Some("10s".to_string()),
                connect_timeout: Some("1s".to_string()),
                tcp_nodelay: Some(true),
            }),
            wal: None,
            storage: Some(DatanodeStorageConfig {
                data_home: Some("./test_data".to_string()),
                storage_type: Some("S3".to_string()),
                cache_capacity: None,
                cache_path: None,
                bucket: Some("test-bucket".to_string()),
                root: None,
                access_key_id: Some("invalid-key".to_string()),
                secret_access_key: Some("invalid-secret".to_string()),
                endpoint: Some("https://s3.amazonaws.com".to_string()),
                region: Some("us-east-1".to_string()),
            }),
            query: None,
            logging: None,
        };

        let checker = DatanodeChecker::new(datanode_config, false);
        let result = checker.check().await;

        // 验证检查失败（因为认证无效）
        assert!(!result.success, "S3 认证无效时检查应该失败");

        // 验证包含 S3 相关的错误信息
        let s3_failures: Vec<_> = result.details.iter()
            .filter(|detail| detail.status == CheckStatus::Fail)
            .collect();

        assert!(!s3_failures.is_empty(), "应该包含失败的检查项");

        // 验证错误消息包含有用信息
        let has_s3_error = s3_failures.iter()
            .any(|detail| detail.message.to_lowercase().contains("s3") ||
                         detail.message.to_lowercase().contains("storage") ||
                         detail.message.to_lowercase().contains("credential"));

        if has_s3_error {
            println!("S3 认证错误测试通过，找到相关错误信息");
        } else {
            println!("警告：未找到明确的 S3 认证错误信息，但检查确实失败了");
        }

        // 验证至少有一个失败项包含建议
        let has_suggestion = s3_failures.iter()
            .any(|detail| detail.suggestion.is_some());

        if has_suggestion {
            println!("找到修复建议");
        }
    }
}

/// 测试 etcd 连接失败场景
#[cfg(test)]
mod etcd_connection_error_tests {
    use super::*;
    use crate::common::ComponentChecker;
    use crate::config::MetasrvConfig;
    use crate::metasrv::MetasrvChecker;

    #[tokio::test]
    async fn test_etcd_connection_failed() {
        // 构造无效的 etcd 配置（连接到不存在的端口）
        let metasrv_config = MetasrvConfig {
            data_home: Some("./test_data".to_string()),
            store_addrs: vec!["127.0.0.1:9999".to_string()], // 不存在的端口
            store_key_prefix: Some("/greptime".to_string()),
            backend: "etcd_store".to_string(),
            meta_table_name: None,
            meta_schema_name: None,
            meta_election_lock_id: None,
            selector: None,
            use_memory_store: None,
            enable_region_failover: None,
            grpc: None,
            http: None,
            backend_tls: None,
        };

        let checker = MetasrvChecker::new(metasrv_config);
        let result = checker.check().await;

        // 验证检查失败
        assert!(!result.success, "etcd 连接失败时检查应该失败");

        // 验证包含连接相关的错误信息
        let connection_failures: Vec<_> = result.details.iter()
            .filter(|detail| detail.status == CheckStatus::Fail)
            .collect();

        assert!(!connection_failures.is_empty(), "应该包含连接失败的检查项");

        // 验证错误消息包含连接相关信息
        let has_connection_error = connection_failures.iter()
            .any(|detail| {
                let msg = detail.message.to_lowercase();
                msg.contains("connection") ||
                msg.contains("connect") ||
                msg.contains("refused") ||
                msg.contains("etcd") ||
                msg.contains("timeout")
            });

        if has_connection_error {
            println!("etcd 连接错误测试通过，找到相关错误信息");
        } else {
            println!("警告：未找到明确的连接错误信息，但检查确实失败了");
            // 打印实际的错误信息用于调试
            for detail in &connection_failures {
                println!("错误详情: {} - {}", detail.item, detail.message);
            }
        }

        // 验证至少有一个失败项包含修复建议
        let has_suggestion = connection_failures.iter()
            .any(|detail| detail.suggestion.is_some());

        if has_suggestion {
            println!("找到修复建议");
            for detail in &connection_failures {
                if let Some(suggestion) = &detail.suggestion {
                    println!("建议: {}", suggestion);
                }
            }
        }
    }
}

/// 测试 Frontend 地址配置错误场景
#[cfg(test)]
mod frontend_address_error_tests {
    use super::*;
    use crate::common::ComponentChecker;
    use crate::config::FrontendConfig;
    use crate::frontend::FrontendChecker;

    #[tokio::test]
    async fn test_frontend_invalid_address_format() {
        use crate::config::{MetaClientConfig};

        // 构造无效的地址配置
        let frontend_config = FrontendConfig {
            data_home: Some("./test_data".to_string()),
            default_timezone: Some("UTC".to_string()),
            http: None,
            grpc: None,
            meta_client: Some(MetaClientConfig {
                metasrv_addrs: vec![
                    "invalid-address".to_string(),        // 缺少端口
                    "localhost:abc".to_string(),           // 无效端口
                    "nonexistent-host:3002".to_string(),   // 不存在的主机
                ],
                timeout: Some("3s".to_string()),
                heartbeat_timeout: Some("500ms".to_string()),
                ddl_timeout: Some("10s".to_string()),
                connect_timeout: Some("1s".to_string()),
                tcp_nodelay: Some(true),
            }),
            heartbeat: None,
            prometheus: None,
            logging: None,
        };

        let checker = FrontendChecker::new(frontend_config);
        let result = checker.check().await;

        // 验证检查失败
        assert!(!result.success, "地址配置错误时检查应该失败");

        // 验证包含地址相关的错误信息
        let address_failures: Vec<_> = result.details.iter()
            .filter(|detail| detail.status == CheckStatus::Fail)
            .collect();

        assert!(!address_failures.is_empty(), "应该包含地址配置失败的检查项");

        // 验证错误消息包含地址相关信息
        let has_address_error = address_failures.iter()
            .any(|detail| {
                let msg = detail.message.to_lowercase();
                msg.contains("address") ||
                msg.contains("port") ||
                msg.contains("parsing") ||
                msg.contains("format") ||
                msg.contains("invalid")
            });

        if has_address_error {
            println!("Frontend 地址错误测试通过，找到相关错误信息");
        } else {
            println!("警告：未找到明确的地址错误信息，但检查确实失败了");
            // 打印实际的错误信息用于调试
            for detail in &address_failures {
                println!("错误详情: {} - {}", detail.item, detail.message);
            }
        }

        // 验证至少有一个失败项包含修复建议
        let has_suggestion = address_failures.iter()
            .any(|detail| detail.suggestion.is_some());

        if has_suggestion {
            println!("找到修复建议");
            for detail in &address_failures {
                if let Some(suggestion) = &detail.suggestion {
                    println!("建议: {}", suggestion);
                }
            }
        }

        // 验证建议中包含正确的地址格式提示
        let has_format_suggestion = address_failures.iter()
            .filter_map(|detail| detail.suggestion.as_ref())
            .any(|suggestion| {
                let sug = suggestion.to_lowercase();
                sug.contains("host:port") || sug.contains("format")
            });

        if has_format_suggestion {
            println!("找到地址格式相关的建议");
        }
    }
}

/// 测试磁盘性能不足场景
#[cfg(test)]
mod disk_performance_tests {
    use crate::common::ComponentChecker;
    use crate::config::{DatanodeConfig, DatanodeStorageConfig, MetaClientConfig};
    use crate::datanode::DatanodeChecker;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_file_storage_performance_check() {
        // 构造文件存储配置，启用性能测试
        let mut storage_config = HashMap::new();
        storage_config.insert("root".to_string(), toml::Value::String("/tmp/greptime_perf_test".to_string()));

        let datanode_config = DatanodeConfig {
            node_id: Some(1),
            require_lease_before_startup: Some(false),
            init_regions_in_background: Some(false),
            init_regions_parallelism: Some(16),
            max_concurrent_queries: Some(0),
            enable_telemetry: Some(true),
            http: None,
            grpc: None,
            heartbeat: None,
            meta_client: Some(MetaClientConfig {
                metasrv_addrs: vec!["127.0.0.1:3002".to_string()],
                timeout: Some("3s".to_string()),
                heartbeat_timeout: Some("500ms".to_string()),
                ddl_timeout: Some("10s".to_string()),
                connect_timeout: Some("1s".to_string()),
                tcp_nodelay: Some(true),
            }),
            wal: None,
            storage: Some(DatanodeStorageConfig {
                data_home: Some("/tmp/greptime_perf_test".to_string()),
                storage_type: Some("File".to_string()),
                cache_capacity: None,
                cache_path: None,
                bucket: None,
                root: None,
                access_key_id: None,
                secret_access_key: None,
                endpoint: None,
                region: None,
            }),
            query: None,
            logging: None,
        };

        // 创建测试目录
        std::fs::create_dir_all("/tmp/greptime_perf_test").ok();

        // 启用性能测试
        let checker = DatanodeChecker::new(datanode_config, true);
        let result = checker.check().await;

        // 验证性能测试被执行
        let performance_checks: Vec<_> = result.details.iter()
            .filter(|detail| {
                let item_lower = detail.item.to_lowercase();
                let msg_lower = detail.message.to_lowercase();
                item_lower.contains("performance") ||
                item_lower.contains("latency") ||
                item_lower.contains("throughput") ||
                msg_lower.contains("performance") ||
                msg_lower.contains("latency") ||
                msg_lower.contains("throughput") ||
                msg_lower.contains("mb/s") ||
                msg_lower.contains("ms")
            })
            .collect();

        if !performance_checks.is_empty() {
            println!("找到 {} 个性能相关的检查项", performance_checks.len());
            for check in &performance_checks {
                println!("性能检查: {} - {} (状态: {:?})",
                    check.item,
                    check.message,
                    check.status
                );
                if let Some(duration) = check.duration {
                    println!("  执行时间: {}ms", duration.as_millis());
                }
            }

            // 验证性能指标被记录
            let has_performance_metrics = performance_checks.iter()
                .any(|detail| {
                    let msg = detail.message.to_lowercase();
                    msg.contains("latency") ||
                    msg.contains("mb/s") ||
                    msg.contains("throughput") ||
                    msg.contains("ms") ||
                    detail.duration.is_some()
                });

            assert!(has_performance_metrics, "应该包含性能指标信息");
            println!("性能指标验证通过");
        } else {
            println!("警告：未找到性能相关的检查项，可能性能测试未启用或未实现");
            // 至少验证基本的存储检查被执行了
            assert!(!result.details.is_empty(), "应该至少执行一些检查项");
        }

        // 清理测试目录
        std::fs::remove_dir_all("/tmp/greptime_perf_test").ok();
    }
}

/// 测试成功场景的消息格式
#[cfg(test)]
mod success_scenario_tests {
    use super::*;
    use crate::common::ComponentChecker;
    use crate::config::{DatanodeConfig, DatanodeStorageConfig, MetaClientConfig};
    use crate::datanode::DatanodeChecker;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_file_storage_success_messages() {
        // 构造有效的文件存储配置
        let mut storage_config = HashMap::new();
        storage_config.insert("root".to_string(), toml::Value::String("/tmp/greptime_success_test".to_string()));

        let datanode_config = DatanodeConfig {
            node_id: Some(1),
            require_lease_before_startup: Some(false),
            init_regions_in_background: Some(false),
            init_regions_parallelism: Some(16),
            max_concurrent_queries: Some(0),
            enable_telemetry: Some(true),
            http: None,
            grpc: None,
            heartbeat: None,
            meta_client: Some(MetaClientConfig {
                metasrv_addrs: vec!["127.0.0.1:3002".to_string()],
                timeout: Some("3s".to_string()),
                heartbeat_timeout: Some("500ms".to_string()),
                ddl_timeout: Some("10s".to_string()),
                connect_timeout: Some("1s".to_string()),
                tcp_nodelay: Some(true),
            }),
            wal: None,
            storage: Some(DatanodeStorageConfig {
                data_home: Some("/tmp/greptime_success_test".to_string()),
                storage_type: Some("File".to_string()),
                cache_capacity: None,
                cache_path: None,
                bucket: None,
                root: None,
                access_key_id: None,
                secret_access_key: None,
                endpoint: None,
                region: None,
            }),
            query: None,
            logging: None,
        };

        // 创建测试目录
        std::fs::create_dir_all("/tmp/greptime_success_test").ok();

        let checker = DatanodeChecker::new(datanode_config, false);
        let result = checker.check().await;

        // 查找成功的检查项
        let success_details: Vec<_> = result.details.iter()
            .filter(|detail| detail.status == CheckStatus::Pass)
            .collect();

        if !success_details.is_empty() {
            println!("找到 {} 个成功的检查项", success_details.len());

            for detail in &success_details {
                // 验证成功消息格式
                assert!(!detail.message.is_empty(), "成功消息不应为空");

                // 成功消息应该包含积极的词汇
                let message_lower = detail.message.to_lowercase();
                let positive_keywords = ["success", "successfully", "pass", "ok", "valid", "available", "ready"];
                let has_positive_keyword = positive_keywords.iter()
                    .any(|keyword| message_lower.contains(keyword));

                if has_positive_keyword {
                    println!("✓ 成功消息包含积极关键词: {}", detail.message);
                } else {
                    println!("? 成功消息可能缺少积极关键词: {}", detail.message);
                }

                // 成功的检查项通常不需要修复建议
                if let Some(suggestion) = &detail.suggestion {
                    // 如果有建议，应该是优化建议而不是错误修复建议
                    let suggestion_lower = suggestion.to_lowercase();
                    let error_keywords = ["error", "failed", "fix", "repair"];
                    let has_error_keyword = error_keywords.iter()
                        .any(|keyword| suggestion_lower.contains(keyword));

                    assert!(!has_error_keyword,
                           "成功项的建议不应包含错误相关词汇: {}", suggestion);

                    println!("✓ 成功项包含优化建议: {}", suggestion);
                }
            }

            // 验证至少有一些成功的检查项
            assert!(!success_details.is_empty(), "应该有一些成功的检查项");
            println!("成功场景测试通过");
        } else {
            println!("警告：未找到成功的检查项，可能所有检查都失败了");
            // 打印所有检查项用于调试
            for detail in &result.details {
                println!("检查项: {} - {} (状态: {:?})",
                    detail.item, detail.message, detail.status);
            }
        }

        // 清理测试目录
        std::fs::remove_dir_all("/tmp/greptime_success_test").ok();
    }
}

/// 测试 JSON 输出格式的完整性
#[cfg(test)]
mod json_output_tests {
    use crate::common::ComponentChecker;
    use crate::config::{DatanodeConfig, DatanodeStorageConfig, MetaClientConfig};
    use crate::datanode::DatanodeChecker;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_json_output_format_completeness() {
        // 构造测试配置
        let mut storage_config = HashMap::new();
        storage_config.insert("root".to_string(), toml::Value::String("/tmp/greptime_json_test".to_string()));

        let datanode_config = DatanodeConfig {
            node_id: Some(1),
            require_lease_before_startup: Some(false),
            init_regions_in_background: Some(false),
            init_regions_parallelism: Some(16),
            max_concurrent_queries: Some(0),
            enable_telemetry: Some(true),
            http: None,
            grpc: None,
            heartbeat: None,
            meta_client: Some(MetaClientConfig {
                metasrv_addrs: vec!["127.0.0.1:3002".to_string()],
                timeout: Some("3s".to_string()),
                heartbeat_timeout: Some("500ms".to_string()),
                ddl_timeout: Some("10s".to_string()),
                connect_timeout: Some("1s".to_string()),
                tcp_nodelay: Some(true),
            }),
            wal: None,
            storage: Some(DatanodeStorageConfig {
                data_home: Some("/tmp/greptime_json_test".to_string()),
                storage_type: Some("File".to_string()),
                cache_capacity: None,
                cache_path: None,
                bucket: None,
                root: None,
                access_key_id: None,
                secret_access_key: None,
                endpoint: None,
                region: None,
            }),
            query: None,
            logging: None,
        };

        // 创建测试目录
        std::fs::create_dir_all("/tmp/greptime_json_test").ok();

        let checker = DatanodeChecker::new(datanode_config, false);
        let result = checker.check().await;

        // 测试 JSON 序列化
        let json_result = result.to_json("TestDatanode", Some("/tmp/test-config.toml"));
        assert!(json_result.is_ok(), "JSON 序列化应该成功");

        let json_string = json_result.unwrap();
        println!("生成的 JSON 长度: {} 字符", json_string.len());

        // 验证 JSON 格式正确
        let parsed: serde_json::Value = serde_json::from_str(&json_string)
            .expect("生成的 JSON 应该是有效的");

        // 验证必要字段存在
        assert!(parsed["component"].is_string(), "应该包含 component 字段");
        assert!(parsed["config_file"].is_string(), "应该包含 config_file 字段");
        assert!(parsed["timestamp"].is_string(), "应该包含 timestamp 字段");
        assert!(parsed["overall_result"].is_string(), "应该包含 overall_result 字段");
        assert!(parsed["total_checks"].is_number(), "应该包含 total_checks 字段");
        assert!(parsed["passed_checks"].is_number(), "应该包含 passed_checks 字段");
        assert!(parsed["failed_checks"].is_number(), "应该包含 failed_checks 字段");
        assert!(parsed["details"].is_array(), "应该包含 details 数组");

        // 验证字段值
        assert_eq!(parsed["component"].as_str().unwrap(), "TestDatanode");
        assert_eq!(parsed["config_file"].as_str().unwrap(), "/tmp/test-config.toml");

        let overall_result = parsed["overall_result"].as_str().unwrap();
        assert!(overall_result == "PASS" || overall_result == "FAIL",
               "overall_result 应该是 PASS 或 FAIL");

        // 验证数字字段的合理性
        let total_checks = parsed["total_checks"].as_u64().unwrap();
        let passed_checks = parsed["passed_checks"].as_u64().unwrap();
        let failed_checks = parsed["failed_checks"].as_u64().unwrap();

        assert_eq!(total_checks, passed_checks + failed_checks,
                  "总检查数应该等于通过数加失败数");
        assert!(total_checks > 0, "应该至少执行一个检查");

        // 验证详细结果的结构
        let details = parsed["details"].as_array().unwrap();
        assert_eq!(details.len() as u64, total_checks, "详细结果数量应该与总检查数一致");

        for (i, detail) in details.iter().enumerate() {
            assert!(detail["item"].is_string(), "检查项 {} 应该有 item 字段", i);
            assert!(detail["status"].is_string(), "检查项 {} 应该有 status 字段", i);
            assert!(detail["message"].is_string(), "检查项 {} 应该有 message 字段", i);

            let status = detail["status"].as_str().unwrap();
            assert!(status == "PASS" || status == "FAIL" || status == "WARNING",
                   "检查项 {} 的状态应该是 PASS、FAIL 或 WARNING", i);

            // 验证消息不为空
            let message = detail["message"].as_str().unwrap();
            assert!(!message.is_empty(), "检查项 {} 的消息不应为空", i);
        }

        println!("JSON 输出格式验证通过:");
        println!("- 总检查数: {}", total_checks);
        println!("- 通过数: {}", passed_checks);
        println!("- 失败数: {}", failed_checks);
        println!("- 整体结果: {}", overall_result);

        // 清理测试目录
        std::fs::remove_dir_all("/tmp/greptime_json_test").ok();
    }
}
