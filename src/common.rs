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

use async_trait::async_trait;
use colored::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Common trait for all component checkers
#[async_trait]
pub trait ComponentChecker {
    /// Perform the check and return the result
    async fn check(&self) -> CheckResult;
    
    /// Get the name of the component being checked
    fn component_name(&self) -> &'static str;
}

/// Result of a component check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    /// Whether the overall check was successful
    pub success: bool,
    /// Overall message describing the result
    pub message: String,
    /// Detailed results for individual check items
    pub details: Vec<CheckDetail>,
    /// Total duration of all checks
    pub total_duration: Option<Duration>,
}

/// Detailed result for a specific check item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckDetail {
    /// Name of the check item
    pub item: String,
    /// Status of the check
    pub status: CheckStatus,
    /// Descriptive message about the result
    pub message: String,
    /// Duration of this specific check
    pub duration: Option<Duration>,
    /// Optional suggestion for fixing issues
    pub suggestion: Option<String>,
}

/// Status of a check item
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckStatus {
    /// Check passed successfully
    Pass,
    /// Check failed
    Fail,
    /// Check passed with warnings
    Warning,
}

impl CheckResult {
    /// Create a new successful check result
    pub fn success(message: String, details: Vec<CheckDetail>) -> Self {
        let total_duration = details
            .iter()
            .filter_map(|d| d.duration)
            .reduce(|acc, d| acc + d);
            
        Self {
            success: true,
            message,
            details,
            total_duration,
        }
    }

    /// Create a new failed check result
    pub fn failure(message: String, details: Vec<CheckDetail>) -> Self {
        let total_duration = details
            .iter()
            .filter_map(|d| d.duration)
            .reduce(|acc, d| acc + d);
            
        Self {
            success: false,
            message,
            details,
            total_duration,
        }
    }

    /// Create a mixed result based on the details
    pub fn from_details(details: Vec<CheckDetail>) -> Self {
        let success = details.iter().all(|d| matches!(d.status, CheckStatus::Pass | CheckStatus::Warning));
        let has_warnings = details.iter().any(|d| d.status == CheckStatus::Warning);
        let failed_count = details.iter().filter(|d| d.status == CheckStatus::Fail).count();
        let passed_count = details.iter().filter(|d| d.status == CheckStatus::Pass).count();
        let warning_count = details.iter().filter(|d| d.status == CheckStatus::Warning).count();
        
        let message = if success {
            if has_warnings {
                format!("Checks completed with warnings ({} passed, {} warnings)", passed_count, warning_count)
            } else {
                format!("All checks passed ({} passed)", passed_count)
            }
        } else {
            format!("Some checks failed ({} passed, {} warnings, {} failed)", passed_count, warning_count, failed_count)
        };

        let total_duration = details
            .iter()
            .filter_map(|d| d.duration)
            .reduce(|acc, d| acc + d);

        Self {
            success,
            message,
            details,
            total_duration,
        }
    }

    /// Print the result in a human-readable format
    pub fn print_human_readable(&self, component_name: &str, config_file: Option<&str>) {
        println!("\n{}", "GreptimeDB Self-Test Report".bold().blue());
        println!("{}", "===========================".blue());
        println!();
        println!("{}: {}", "Component".bold(), component_name);
        if let Some(config) = config_file {
            println!("{}: {}", "Configuration".bold(), config);
        }
        if let Some(duration) = self.total_duration {
            println!("{}: {:?}", "Total Duration".bold(), duration);
        }
        println!();

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

            let duration_text = if let Some(duration) = detail.duration {
                format!(" ({:?})", duration)
            } else {
                String::new()
            };

            println!("{} {:<30} {} {} - {}", 
                status_symbol, 
                detail.item, 
                status_text, 
                duration_text,
                detail.message
            );

            if let Some(suggestion) = &detail.suggestion {
                println!("    💡 {}: {}", "Suggestion".yellow(), suggestion);
            }
        }

        println!();
        let overall_status = if self.success {
            format!("Overall Result: {}", "PASS".green().bold())
        } else {
            format!("Overall Result: {}", "FAIL".red().bold())
        };
        println!("{}", overall_status);
        println!();
    }

    /// Convert the result to JSON format
    pub fn to_json(&self, component_name: &str, config_file: Option<&str>) -> serde_json::Result<String> {
        let json_result = serde_json::json!({
            "component": component_name,
            "config_file": config_file,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "overall_result": if self.success { "PASS" } else { "FAIL" },
            "total_checks": self.details.len(),
            "passed_checks": self.details.iter().filter(|d| d.status == CheckStatus::Pass).count(),
            "failed_checks": self.details.iter().filter(|d| d.status == CheckStatus::Fail).count(),
            "warning_checks": self.details.iter().filter(|d| d.status == CheckStatus::Warning).count(),
            "total_duration_ms": self.total_duration.map(|d| d.as_millis()),
            "message": self.message,
            "details": self.details.iter().map(|d| serde_json::json!({
                "item": d.item,
                "status": match d.status {
                    CheckStatus::Pass => "PASS",
                    CheckStatus::Fail => "FAIL",
                    CheckStatus::Warning => "WARNING",
                },
                "message": d.message,
                "duration_ms": d.duration.map(|dur| dur.as_millis()),
                "suggestion": d.suggestion,
            })).collect::<Vec<_>>()
        });

        serde_json::to_string_pretty(&json_result)
    }
}

impl CheckDetail {
    /// Create a new passing check detail
    pub fn pass(item: String, message: String, duration: Option<Duration>) -> Self {
        Self {
            item,
            status: CheckStatus::Pass,
            message,
            duration,
            suggestion: None,
        }
    }

    /// Create a new failing check detail
    pub fn fail(item: String, message: String, duration: Option<Duration>, suggestion: Option<String>) -> Self {
        Self {
            item,
            status: CheckStatus::Fail,
            message,
            duration,
            suggestion,
        }
    }

    /// Create a new warning check detail
    pub fn warning(item: String, message: String, duration: Option<Duration>, suggestion: Option<String>) -> Self {
        Self {
            item,
            status: CheckStatus::Warning,
            message,
            duration,
            suggestion,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_result_success() {
        let details = vec![
            CheckDetail::pass("Test 1".to_string(), "Passed".to_string(), None),
            CheckDetail::pass("Test 2".to_string(), "Passed".to_string(), None),
        ];

        let result = CheckResult::from_details(details);
        assert!(result.success);
        assert_eq!(result.details.len(), 2);
    }

    #[test]
    fn test_check_result_failure() {
        let details = vec![
            CheckDetail::pass("Test 1".to_string(), "Passed".to_string(), None),
            CheckDetail::fail("Test 2".to_string(), "Failed".to_string(), None, None),
        ];

        let result = CheckResult::from_details(details);
        assert!(!result.success);
        assert_eq!(result.details.len(), 2);
    }

    #[test]
    fn test_check_result_with_warnings() {
        let details = vec![
            CheckDetail::pass("Test 1".to_string(), "Passed".to_string(), None),
            CheckDetail::warning("Test 2".to_string(), "Warning".to_string(), None, None),
        ];

        let result = CheckResult::from_details(details);
        assert!(result.success); // Warnings don't fail the overall result
        assert_eq!(result.details.len(), 2);
    }

    #[test]
    fn test_check_detail_creation() {
        let detail = CheckDetail::pass("Test".to_string(), "Message".to_string(), None);
        assert_eq!(detail.status, CheckStatus::Pass);
        assert_eq!(detail.item, "Test");
        assert_eq!(detail.message, "Message");
        assert!(detail.suggestion.is_none());

        let detail = CheckDetail::fail("Test".to_string(), "Message".to_string(), None, Some("Fix it".to_string()));
        assert_eq!(detail.status, CheckStatus::Fail);
        assert_eq!(detail.suggestion, Some("Fix it".to_string()));
    }

    #[test]
    fn test_json_serialization() {
        let details = vec![
            CheckDetail::pass("Test 1".to_string(), "Passed".to_string(), None),
        ];

        let result = CheckResult::from_details(details);
        let json = result.to_json("TestComponent", Some("/test/config.toml"));
        assert!(json.is_ok());

        let json_str = json.unwrap();
        assert!(json_str.contains("TestComponent"));
        assert!(json_str.contains("/test/config.toml"));
        assert!(json_str.contains("PASS"));
    }
}
