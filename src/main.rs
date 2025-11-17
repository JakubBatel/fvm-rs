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
    Ls,
    Releases(commands::releases::ReleasesArgs),
    /// Removes a Flutter SDK version
    Rm(commands::rm::RmArgs),
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
        Commands::Ls => commands::ls::run().await,
        Commands::Releases(args) => commands::releases::run(args).await,
        Commands::Rm(args) => commands::rm::run(args).await,
    }
}
