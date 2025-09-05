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

mod common;
mod config;
mod datanode;
mod error;
mod frontend;
#[allow(dead_code)]
mod metasrv;

#[cfg(test)]
mod tests;

use clap::{Parser, Subcommand};
use common::{ComponentChecker, CheckResult};
use config::ConfigParser;
use datanode::DatanodeChecker;
use frontend::FrontendChecker;
use metasrv::MetasrvChecker;

#[derive(Parser)]
#[command(author, version, about = "GreptimeDB Self-Test Tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check frontend components
    Frontend {
        /// Path to configuration file
        #[arg(short = 'c', long)]
        config: String,
        /// Enable verbose output
        #[arg(short, long)]
        verbose: bool,
        /// Output format: human (default) or json
        #[arg(long, default_value = "human")]
        output: String,
    },
    /// Check datanode components
    Datanode {
        /// Path to configuration file
        #[arg(short = 'c', long)]
        config: String,
        /// Enable verbose output
        #[arg(short, long)]
        verbose: bool,
        /// Include performance tests
        #[arg(long)]
        include_performance: bool,
        /// Output format: human (default) or json
        #[arg(long, default_value = "human")]
        output: String,
    },
    /// Check metasrv components
    Metasrv {
        /// Path to configuration file
        #[arg(short = 'c', long)]
        config: String,
        /// Enable verbose output
        #[arg(short, long)]
        verbose: bool,
        /// Output format: human (default) or json
        #[arg(long, default_value = "human")]
        output: String,
    },
}

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

    match result {
        Ok(success) => {
            if !success {
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

async fn run_frontend_check(config_path: &str, _verbose: bool, output_format: &str) -> error::Result<bool> {
    let config = ConfigParser::parse_frontend_config(config_path)?;
    let checker = FrontendChecker::new(config);
    let result = checker.check().await;

    output_result(&result, checker.component_name(), Some(config_path), output_format)?;
    Ok(result.success)
}

async fn run_datanode_check(config_path: &str, _verbose: bool, include_performance: bool, output_format: &str) -> error::Result<bool> {
    let config = ConfigParser::parse_datanode_config(config_path)?;
    let checker = DatanodeChecker::new(config, include_performance);
    let result = checker.check().await;

    output_result(&result, checker.component_name(), Some(config_path), output_format)?;
    Ok(result.success)
}

async fn run_metasrv_check(config_path: &str, _verbose: bool, output_format: &str) -> error::Result<bool> {
    let config = ConfigParser::parse_metasrv_config(config_path)?;
    let checker = MetasrvChecker::new(config);
    let result = checker.check().await;

    output_result(&result, checker.component_name(), Some(config_path), output_format)?;
    Ok(result.success)
}

fn output_result(result: &CheckResult, component_name: &str, config_file: Option<&str>, output_format: &str) -> error::Result<()> {
    use snafu::ResultExt;

    match output_format {
        "json" => {
            let json_output = result.to_json(component_name, config_file)
                .context(error::JsonSerializationSnafu {
                    message: "Failed to serialize result to JSON".to_string(),
                })?;
            println!("{}", json_output);
        }
        "human" | _ => {
            result.print_human_readable(component_name, config_file);
        }
    }
    Ok(())
}