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

mod datanode;
mod frontend;
mod metasrv;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
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
        config: Option<String>,
    },
    /// Check datanode components
    Datanode {
        /// Path to configuration file
        #[arg(short = 'c', long)]
        config: Option<String>,
    },
    /// Check metasrv components
    Metasrv {
        /// Path to configuration file
        #[arg(short = 'c', long)]
        config: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Frontend { config } => {
            println!("Starting frontend service");
            if let Some(config_path) = config {
                println!("Using config file: {}", config_path);
            }
        }
        Commands::Datanode { config } => {
            println!("Starting datanode service");
            if let Some(config_path) = config {
                println!("Using config file: {}", config_path);
            }
        }
        Commands::Metasrv { config } => {
            println!("Starting metasrv service");
            if let Some(config_path) = config {
                println!("Using config file: {}", config_path);
            }
        }
    }
}
