use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "fdml",
    about = "FDML (Feature-Driven Modeling Language) CLI tools",
    long_about = "FDML CLI provides tools for parsing, validating, generating code, and managing FDML specifications.\n\nExamples:\n  fdml init my-project\n  fdml add entity user --name \"User\" --target app.fdml\n  fdml generate app.fdml --language typescript --with-tests\n  fdml list entities --target app.fdml",
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
    #[command(
        about = "Add FDML entities directly to specification files",
        long_about = "Add features, entities, actions, constraints, and fields directly to FDML files.\n\nExamples:\n  fdml add entity user --name \"User\" --target app.fdml\n  fdml add field user email --field-type string --required\n  fdml add action login --name \"User Login\" --description \"Authenticate user\""
    )]
    Add {
        #[command(subcommand)]
        operation: AddCommands,
    },
    
    /// List FDML entities from specification files
    #[command(
        about = "List FDML entities from specification files", 
        long_about = "List and display information about features, entities, actions, and constraints.\n\nExamples:\n  fdml list entities --target app.fdml\n  fdml list features\n  fdml list actions --target spec.fdml"
    )]
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
    #[command(
        about = "Add a new feature to FDML specification",
        long_about = "Add a new feature with title and optional description.\n\nExample:\n  fdml add feature user_auth --title \"User Authentication\" --description \"Login system\" --target app.fdml"
    )]
    Feature {
        /// Feature ID (unique identifier)
        id: String,
        
        /// Feature title (human-readable name)
        #[arg(long)]
        title: String,
        
        /// Feature description (optional)
        #[arg(long)]
        description: Option<String>,
        
        /// Target FDML file to modify (auto-detected if not specified)
        #[arg(short, long)]
        target: Option<String>,
    },
    
    /// Add a new entity
    #[command(
        about = "Add a new entity to FDML specification", 
        long_about = "Add a new entity with name and optional description.\n\nExample:\n  fdml add entity user --name \"User\" --description \"User account data\" --target app.fdml"
    )]
    Entity {
        /// Entity ID (unique identifier)
        id: String,
        
        /// Entity name (human-readable name)
        #[arg(long)]
        name: String,
        
        /// Entity description (optional)
        #[arg(long)]
        description: Option<String>,
        
        /// Target FDML file to modify (auto-detected if not specified)
        #[arg(short, long)]
        target: Option<String>,
    },
    
    /// Add a new action
    #[command(
        about = "Add a new action to FDML specification",
        long_about = "Add a new action with name and optional description.\n\nExample:\n  fdml add action login --name \"User Login\" --description \"Authenticate user credentials\" --target app.fdml"
    )]
    Action {
        /// Action ID (unique identifier)
        id: String,
        
        /// Action name (human-readable name)
        #[arg(long)]
        name: String,
        
        /// Action description (optional)
        #[arg(long)]
        description: Option<String>,
        
        /// Target FDML file to modify (auto-detected if not specified)
        #[arg(short, long)]
        target: Option<String>,
    },
    
    /// Add a new constraint
    #[command(
        about = "Add a new constraint to FDML specification",
        long_about = "Add a new business constraint with condition and target.\n\nExample:\n  fdml add constraint email_unique --name \"Email Uniqueness\" --condition \"unique(email)\" --applies-to \"user.email\" --target app.fdml"
    )]
    Constraint {
        /// Constraint ID (unique identifier)
        id: String,
        
        /// Constraint name (human-readable name)
        #[arg(long)]
        name: String,
        
        /// Constraint condition/rule (e.g., \"unique(email)\", \"min_length(8)\")
        #[arg(long)]
        condition: String,
        
        /// What the constraint applies to (e.g., \"user.email\", \"product.price\")
        #[arg(long)]
        applies_to: String,
        
        /// Constraint description (optional)
        #[arg(long)]
        description: Option<String>,
        
        /// Error message for constraint violations (optional)
        #[arg(long)]
        message: Option<String>,
        
        /// Target FDML file to modify (auto-detected if not specified)
        #[arg(short, long)]
        target: Option<String>,
    },
    
    /// Add a field to an entity
    #[command(
        about = "Add a field to an existing entity",
        long_about = "Add a new field to an existing entity with type and optional constraints.\n\nExamples:\n  fdml add field user email --field-type string --required --target app.fdml\n  fdml add field user age --field-type integer --default \"18\""
    )]
    Field {
        /// Entity ID to add field to
        entity_id: String,
        
        /// Field name
        field_name: String,
        
        /// Field type (string, integer, float, boolean, etc.)
        #[arg(long)]
        field_type: String,
        
        /// Whether field is required (flag, default: false)
        #[arg(long)]
        required: bool,
        
        /// Default value for field (optional)
        #[arg(long)]
        default: Option<String>,
        
        /// Target FDML file to modify (auto-detected if not specified)
        #[arg(short, long)]
        target: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ListCommands {
    /// List all features
    #[command(
        about = "List all features in FDML specification",
        long_about = "Display all features with their titles, descriptions, and scenario counts.\n\nExample:\n  fdml list features --target app.fdml"
    )]
    Features {
        /// Target FDML file to read from (auto-detected if not specified)
        #[arg(short, long)]
        target: Option<String>,
    },
    
    /// List all entities
    #[command(
        about = "List all entities in FDML specification",
        long_about = "Display all entities with their names, descriptions, and field counts.\n\nExample:\n  fdml list entities --target app.fdml"
    )]
    Entities {
        /// Target FDML file to read from (auto-detected if not specified)
        #[arg(short, long)]
        target: Option<String>,
    },
    
    /// List all actions
    #[command(
        about = "List all actions in FDML specification",
        long_about = "Display all actions with their names and descriptions.\n\nExample:\n  fdml list actions --target app.fdml"
    )]
    Actions {
        /// Target FDML file to read from (auto-detected if not specified)
        #[arg(short, long)]
        target: Option<String>,
    },
    
    /// List all constraints
    #[command(
        about = "List all constraints in FDML specification",
        long_about = "Display all constraints with their names, rules, and descriptions.\n\nExample:\n  fdml list constraints --target app.fdml"
    )]
    Constraints {
        /// Target FDML file to read from (auto-detected if not specified)
        #[arg(short, long)]
        target: Option<String>,
    },
}