use clap::{Parser, Subcommand};

mod commands;
mod config_manager;
mod sdk_manager;
mod utils;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct FvmArgs {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    /// Installs a Flutter SDK version
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

    // cache::ensure_bare_cache(url, path)

    match args.cmd {
        Commands::Use(args) => commands::r#use::run(args).await,
        Commands::Ls => commands::ls::run().await,
        Commands::Releases(args) => commands::releases::run(args).await,
        Commands::Rm(args) => commands::rm::run(args).await,
    }
}
