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

    /// Scan existing source code and extract inventory (entities, actions, relationships)
    #[command(name = "parse-code")]
    ParseCode {
        /// Path to the source code directory or file to scan
        input: String,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<String>,

        /// Output format (yaml, json)
        #[arg(short, long, default_value = "yaml")]
        format: String,

        /// Directories to exclude from scanning (comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        exclude: Vec<String>,
    },

    /// Launch interactive visual spec viewer in the browser
    Serve {
        /// Path to the .fdml file to visualize
        file: String,

        /// Port to serve on
        #[arg(short, long, default_value = "3000")]
        port: u16,

        /// Don't auto-open the browser
        #[arg(long)]
        no_open: bool,

        /// Generate FDML spec from a project directory (shows live progress, then switches to viewer)
        #[arg(long)]
        generate: Option<String>,

        /// Use fast model (haiku) for generation
        #[arg(long)]
        fast: bool,

        /// Specific model to use for generation
        #[arg(long)]
        model: Option<String>,

        /// LLM provider: "cli", "api", or "ollama"
        #[arg(long)]
        provider: Option<String>,

        /// Number of parallel LLM calls for per-system generation (default: 1 = sequential)
        #[arg(long, default_value = "1")]
        parallel: usize,

        /// Ollama base URL (default: http://localhost:11434)
        #[arg(long)]
        ollama_url: Option<String>,

        /// Context window size for Ollama (default: auto-calculated from prompt size)
        #[arg(long)]
        num_ctx: Option<usize>,

        /// Chunking strategy: "monolithic" (one big prompt) or "sectional" (split into sub-prompts)
        #[arg(long)]
        chunk_strategy: Option<String>,
    },

    /// Link code inventory to FDML spec — match classes→entities, methods→actions, modules→features
    #[command(name = "link-code")]
    LinkCode {
        /// Path to code inventory file (from parse-code output)
        #[arg(long)]
        code: String,

        /// Path to FDML spec file (can be empty/missing — generates draft)
        #[arg(long)]
        fdml: Option<String>,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<String>,

        /// Output format (yaml, json, prompt)
        #[arg(short, long, default_value = "yaml")]
        format: String,

        /// Send metaprompt to LLM via claude CLI and get FDML spec back
        #[arg(long)]
        llm: bool,

        /// Generate spec instantly from code analysis without LLM (heuristic classification)
        #[arg(long)]
        no_llm: bool,

        /// Use fast model (haiku) for quick drafts
        #[arg(long)]
        fast: bool,

        /// Specific model to use with --llm (default: sonnet)
        #[arg(long)]
        model: Option<String>,

        /// LLM provider: "cli", "api", or "ollama". Default: auto-detect
        #[arg(long)]
        provider: Option<String>,

        /// Ollama base URL (default: http://localhost:11434)
        #[arg(long)]
        ollama_url: Option<String>,

        /// Context window size for Ollama (default: auto-calculated)
        #[arg(long)]
        num_ctx: Option<usize>,

        /// Chunking strategy: "monolithic" or "sectional"
        #[arg(long)]
        chunk_strategy: Option<String>,
    },

    /// Scan a multi-system project and generate an FDML 1.4 platform spec
    #[command(name = "scan-platform")]
    ScanPlatform {
        /// Root directory of the multi-system project
        input: String,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<String>,

        /// Output format (yaml, json, prompt)
        #[arg(short, long, default_value = "yaml")]
        format: String,

        /// Directories to exclude from scanning (comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        exclude: Vec<String>,

        /// Send metaprompt to LLM and get FDML 1.4 spec back
        #[arg(long)]
        llm: bool,

        /// Use fast model (haiku) for quick drafts
        #[arg(long)]
        fast: bool,

        /// Specific model to use with --llm
        #[arg(long)]
        model: Option<String>,

        /// LLM provider: "cli", "api", or "ollama". Default: auto-detect
        #[arg(long)]
        provider: Option<String>,

        /// Ollama base URL (default: http://localhost:11434)
        #[arg(long)]
        ollama_url: Option<String>,

        /// Context window size for Ollama (default: auto-calculated)
        #[arg(long)]
        num_ctx: Option<usize>,

        /// Chunking strategy: "monolithic" or "sectional"
        #[arg(long)]
        chunk_strategy: Option<String>,
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