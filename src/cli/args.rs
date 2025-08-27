use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "fdml",
    about = "FDML (Feature-Driven Modeling Language) CLI tools",
    version = env!("CARGO_PKG_VERSION"),
    author = "FDML Contributors"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    
    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new FDML project
    Init {
        /// Project name
        name: String,
        
        /// Force initialization even if directory exists
        #[arg(short, long)]
        force: bool,
    },
    
    /// Validate an FDML specification file
    Validate {
        /// Path to the FDML file to validate
        file: String,
        
        /// Use strict validation (fail on warnings)
        #[arg(short, long)]
        strict: bool,
        
        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        output: String,
    },
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}