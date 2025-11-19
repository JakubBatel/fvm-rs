use clap::{Parser, Subcommand};
use tracing_subscriber::fmt::format::{self, FormatEvent, FormatFields};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::EnvFilter;

mod commands;
mod config_manager;
mod gitignore_manager;
mod sdk_manager;
mod utils;

// Custom compact log format with short timestamp and single-letter levels
struct CompactFormat;

impl<S, N> FormatEvent<S, N> for CompactFormat
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &tracing_subscriber::fmt::FmtContext<'_, S, N>,
        mut writer: format::Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        let metadata = event.metadata();

        // Time in HH:MM:SS format with gray color
        use colored::Colorize;
        let now = chrono::Local::now();
        write!(writer, "{} ", now.format("%H:%M:%S").to_string().bright_black())?;

        // Level as single character with color (no brackets)
        let level_str = match *metadata.level() {
            tracing::Level::ERROR => "E".red(),
            tracing::Level::WARN => "W".yellow(),
            tracing::Level::INFO => "I".green(),
            tracing::Level::DEBUG => "D".blue(),
            tracing::Level::TRACE => "T".purple(),
        };
        write!(writer, "{} ", level_str)?;

        // Event fields (the message)
        ctx.field_format().format_fields(writer.by_ref(), event)?;

        writeln!(writer)
    }
}

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct FvmArgs {
    /// Enable verbose output (debug logging)
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    /// Installs a Flutter SDK version
    Install(commands::install::InstallArgs),
    /// Sets Flutter SDK version for current project
    Use(commands::r#use::UseArgs),
    /// Lists installed Flutter SDK versions
    #[command(alias = "ls")]
    List,
    /// Shows available Flutter SDK releases
    Releases(commands::releases::ReleasesArgs),
    /// Removes a Flutter SDK version
    #[command(alias = "rm")]
    Remove(commands::remove::RemoveArgs),
    /// Manages global configuration settings
    Config(commands::config::ConfigArgs),
    /// Sets or displays the global Flutter SDK version
    Global(commands::global::GlobalArgs),
    /// Shows FVM environment and project configuration
    Doctor(commands::doctor::DoctorArgs),
    /// Executes Flutter commands using a specific project flavor
    Flavor(commands::flavor::FlavorArgs),
    /// Runs Flutter commands using the project's configured SDK version
    Flutter(commands::flutter::FlutterArgs),
    /// Runs Dart commands using the project's configured Flutter SDK
    Dart(commands::dart::DartArgs),
    /// Executes arbitrary commands with project's configured SDK in PATH
    Exec(commands::exec::ExecArgs),
    /// Executes Flutter commands with a specific SDK version
    Spawn(commands::spawn::SpawnArgs),
    /// Completely removes the FVM cache directory and all cached versions
    Destroy(commands::destroy::DestroyArgs),
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let args = FvmArgs::parse();

    // Initialize tracing subscriber based on verbose flag
    let log_level = if args.verbose { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .event_format(CompactFormat)
        .with_env_filter(EnvFilter::new(log_level))
        .init();

    // cache::ensure_bare_cache(url, path)

    match args.cmd {
        Commands::Install(args) => commands::install::run(args).await,
        Commands::Use(args) => commands::r#use::run(args).await,
        Commands::List => commands::list::run().await,
        Commands::Releases(args) => commands::releases::run(args).await,
        Commands::Remove(args) => commands::remove::run(args).await,
        Commands::Config(args) => commands::config::run(args).await,
        Commands::Global(args) => commands::global::run(args).await,
        Commands::Doctor(args) => commands::doctor::run(args).await,
        Commands::Flavor(args) => commands::flavor::run(args).await,
        Commands::Flutter(args) => {
            let exit_code = commands::flutter::run(args).await?;
            std::process::exit(exit_code);
        }
        Commands::Dart(args) => {
            let exit_code = commands::dart::run(args).await?;
            std::process::exit(exit_code);
        }
        Commands::Exec(args) => {
            let exit_code = commands::exec::run(args).await?;
            std::process::exit(exit_code);
        }
        Commands::Spawn(args) => {
            let exit_code = commands::spawn::run(args).await?;
            std::process::exit(exit_code);
        }
        Commands::Destroy(args) => commands::destroy::run(args).await,
    }
}
