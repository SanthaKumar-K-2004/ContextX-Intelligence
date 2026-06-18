use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "contextx")]
#[command(about = "Local-first context compression and usage intelligence")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Run the MCP stdio server.
    Mcp(McpArgs),
    /// Run a localhost OpenAI-compatible proxy.
    Proxy(ProxyArgs),
    /// Run the local daemon API. In this MVP it is the same lightweight HTTP surface as proxy.
    Daemon(DaemonArgs),
    /// Wrap and track a local CLI command.
    Wrap(WrapArgs),
    /// Show a live terminal dashboard.
    Tui(TuiArgs),
    /// Print current memory-only stats.
    Stats(StatsArgs),
    /// Print a compact human-readable usage table.
    Status(StatsArgs),
    /// Check local setup and recommendations.
    Doctor(DoctorArgs),
    /// One-command setup: install ContextX locally, update PATH, and configure clients.
    Setup(SetupArgs),
    /// Auto-configure supported local clients.
    Install(InstallArgs),
    /// Print an MCP client config snippet without editing files.
    PrintConfig(ClientArgs),
    /// Verify that a client config contains ContextX.
    VerifyClient(ClientArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ProxyArgs {
    #[arg(long, default_value_t = 8787)]
    pub port: u16,
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    #[arg(
        long,
        env = "CONTEXTX_UPSTREAM_BASE_URL",
        default_value = "https://api.openai.com"
    )]
    pub upstream: String,
    /// Allow binding to a non-localhost address. Disabled by default for privacy.
    #[arg(long)]
    pub allow_non_localhost: bool,
}

#[derive(Debug, Clone, Args)]
pub struct DaemonArgs {
    #[arg(long, default_value_t = 8787)]
    pub port: u16,
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    /// Allow binding to a non-localhost address. Disabled by default for privacy.
    #[arg(long)]
    pub allow_non_localhost: bool,
}

impl From<DaemonArgs> for ProxyArgs {
    fn from(value: DaemonArgs) -> Self {
        Self {
            port: value.port,
            host: value.host,
            upstream: "https://api.openai.com".to_string(),
            allow_non_localhost: value.allow_non_localhost,
        }
    }
}

#[derive(Debug, Clone, Args)]
pub struct McpArgs {
    /// Prefer a running daemon for shared state, falling back to local memory if unavailable.
    #[arg(long, default_value = "http://127.0.0.1:8787")]
    pub daemon_url: String,
    /// Force MCP to use private in-process memory instead of a shared daemon.
    #[arg(long)]
    pub local: bool,
}

#[derive(Debug, Clone, Args)]
pub struct WrapArgs {
    pub command: String,
    #[arg(trailing_var_arg = true)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Args)]
pub struct TuiArgs {
    #[arg(long, default_value_t = 2)]
    pub refresh_seconds: u64,
    #[arg(long, default_value = "http://127.0.0.1:8787")]
    pub daemon_url: String,
    #[arg(long)]
    pub local: bool,
}

#[derive(Debug, Clone, Args)]
pub struct StatsArgs {
    #[arg(long)]
    pub watch: bool,
    #[arg(long, default_value_t = 2)]
    pub refresh_seconds: u64,
    #[arg(long, default_value = "http://127.0.0.1:8787")]
    pub daemon_url: String,
    #[arg(long)]
    pub local: bool,
}

#[derive(Debug, Clone, Args)]
pub struct DoctorArgs {
    /// Apply safe setup fixes, equivalent to install --all.
    #[arg(long)]
    pub fix: bool,
    /// Show what --fix would do without writing files.
    #[arg(long)]
    pub dry_run: bool,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Args)]
pub struct ClientArgs {
    /// Client name: claude-desktop, cursor, vscode, or zed.
    #[arg(long, default_value = "claude-desktop")]
    pub client: String,
}

#[derive(Debug, Clone, Args)]
pub struct InstallArgs {
    #[arg(long)]
    pub all: bool,
    /// Install only one client: claude-desktop, cursor, vscode, or zed.
    #[arg(long)]
    pub client: Option<String>,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Args)]
pub struct SetupArgs {
    #[arg(long)]
    pub all: bool,
    /// Setup only one client: claude-desktop, cursor, vscode, or zed.
    #[arg(long)]
    pub client: Option<String>,
    /// Show setup actions without writing files.
    #[arg(long)]
    pub dry_run: bool,
}
