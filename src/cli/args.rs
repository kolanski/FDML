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
    
    /// Parse and display AST from FDML files
    Parse {
        /// Path to the FDML file to parse
        file: String,
        
        /// Output format (json, yaml)
        #[arg(short, long, default_value = "json")]
        output: String,
        
        /// Enable debug mode with detailed parsing info
        #[arg(short, long)]
        debug: bool,
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
    
    /// Generate code from FDML features
    Generate {
        /// Path to the FDML file or project directory
        input: String,
        
        /// Target language (typescript, python, go)
        #[arg(short, long)]
        language: String,
        
        /// Output directory for generated code
        #[arg(short, long, default_value = "./generated")]
        output: String,
        
        /// Template directory (optional)
        #[arg(short, long)]
        template: Option<String>,
        
        /// Generate tests along with code
        #[arg(long)]
        with_tests: bool,
    },
    
    /// Add FDML entities directly to specification files
    Add {
        #[command(subcommand)]
        operation: AddCommands,
    },
    
    /// List FDML entities from specification files
    List {
        #[command(subcommand)]
        operation: ListCommands,
    },
    
    /// Run migration operations
    Migrate {
        #[command(subcommand)]
        operation: MigrateCommands,
    },
    
    /// Traceability operations
    Trace {
        #[command(subcommand)]
        operation: TraceCommands,
    },
}

#[derive(Subcommand)]
pub enum MigrateCommands {
    /// Apply migrations
    Apply {
        /// Path to migration files
        #[arg(short, long, default_value = "./migrations")]
        path: String,
        
        /// Target FDML file to modify
        #[arg(short, long)]
        target: Option<String>,
        
        /// Dry run mode (don't apply changes)
        #[arg(long)]
        dry_run: bool,
    },
    
    /// Rollback migrations
    Rollback {
        /// Path to migration files
        #[arg(short, long, default_value = "./migrations")]
        path: String,
        
        /// Target FDML file to modify
        #[arg(short, long)]
        target: Option<String>,
        
        /// Number of migrations to rollback
        #[arg(short, long, default_value = "1")]
        count: usize,
        
        /// Dry run mode (don't apply changes)
        #[arg(long)]
        dry_run: bool,
    },
    
    /// Show migration status
    Status {
        /// Path to migration files
        #[arg(short, long, default_value = "./migrations")]
        path: String,
    },
}

#[derive(Subcommand)]
pub enum TraceCommands {
    /// Validate traceability links
    Validate {
        /// Path to the FDML project
        #[arg(default_value = ".")]
        path: String,
    },
    
    /// Generate traceability graph
    Graph {
        /// Path to the FDML project
        #[arg(default_value = ".")]
        path: String,
        
        /// Output format (dot, svg, png)
        #[arg(short, long, default_value = "dot")]
        format: String,
        
        /// Output file
        #[arg(short, long, default_value = "traceability.dot")]
        output: String,
    },
    
    /// Generate traceability matrix
    Matrix {
        /// Path to the FDML project
        #[arg(default_value = ".")]
        path: String,
        
        /// Output format (csv, html, json)
        #[arg(short, long, default_value = "csv")]
        format: String,
        
        /// Output file
        #[arg(short, long, default_value = "traceability.csv")]
        output: String,
    },
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}

#[derive(Subcommand)]
pub enum AddCommands {
    /// Add a new feature
    Feature {
        /// Feature ID
        id: String,
        
        /// Feature title
        #[arg(long)]
        title: String,
        
        /// Feature description
        #[arg(long)]
        description: Option<String>,
        
        /// Target FDML file to modify
        #[arg(short, long)]
        target: Option<String>,
    },
    
    /// Add a new entity
    Entity {
        /// Entity ID
        id: String,
        
        /// Entity name
        #[arg(long)]
        name: String,
        
        /// Entity description
        #[arg(long)]
        description: Option<String>,
        
        /// Target FDML file to modify
        #[arg(short, long)]
        target: Option<String>,
    },
    
    /// Add a new action
    Action {
        /// Action ID
        id: String,
        
        /// Action name
        #[arg(long)]
        name: String,
        
        /// Action description
        #[arg(long)]
        description: Option<String>,
        
        /// Target FDML file to modify
        #[arg(short, long)]
        target: Option<String>,
    },
    
    /// Add a new constraint
    Constraint {
        /// Constraint ID
        id: String,
        
        /// Constraint name
        #[arg(long)]
        name: String,
        
        /// Constraint condition/rule
        #[arg(long)]
        condition: String,
        
        /// What the constraint applies to
        #[arg(long)]
        applies_to: String,
        
        /// Constraint description
        #[arg(long)]
        description: Option<String>,
        
        /// Error message for constraint violations
        #[arg(long)]
        message: Option<String>,
        
        /// Target FDML file to modify
        #[arg(short, long)]
        target: Option<String>,
    },
    
    /// Add a field to an entity
    Field {
        /// Entity ID to add field to
        entity_id: String,
        
        /// Field name
        field_name: String,
        
        /// Field type
        #[arg(long)]
        field_type: String,
        
        /// Whether field is required
        #[arg(long)]
        required: bool,
        
        /// Default value for field
        #[arg(long)]
        default: Option<String>,
        
        /// Target FDML file to modify
        #[arg(short, long)]
        target: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ListCommands {
    /// List all features
    Features {
        /// Target FDML file to read from
        #[arg(short, long)]
        target: Option<String>,
    },
    
    /// List all entities
    Entities {
        /// Target FDML file to read from
        #[arg(short, long)]
        target: Option<String>,
    },
    
    /// List all actions
    Actions {
        /// Target FDML file to read from
        #[arg(short, long)]
        target: Option<String>,
    },
    
    /// List all constraints
    Constraints {
        /// Target FDML file to read from
        #[arg(short, long)]
        target: Option<String>,
    },
}