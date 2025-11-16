use std::collections::HashSet;

use crate::sdk_manager;
use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use colored::Colorize;
use tabled::{Table, Tabled, settings::Style};
use tracing::info;

#[derive(Debug, Clone, Args)]
pub struct ReleasesArgs {
    #[arg(
        long,
        value_parser = clap::builder::PossibleValuesParser::new(["stable", "beta", "dev", "all"]),
        default_value = "stable"
    )]
    pub channel: String,
}

pub async fn run(args: ReleasesArgs) -> Result<()> {
    info!("Fetching available Flutter releases for channel: {}", args.channel);

    let (versions_result, installed_versions_result) = tokio::join!(
        sdk_manager::list_available_versions(),
        sdk_manager::list_installed_versions()
    );

    let versions = versions_result?;
    let installed_versions: HashSet<String> = installed_versions_result?.into_iter().collect();

    info!("Retrieved {} releases, {} installed locally", versions.releases.len(), installed_versions.len());

    let releases_rows: Vec<ReleaseRow> = versions
        .releases
        .iter()
        .rev()
        .filter_map(|release| {
            if args.channel != "all" && args.channel != release.channel {
                None
            } else {
                Some(ReleaseRow {
                    version: release.version.clone(),
                    release_date: release.release_date,
                    channel: format!(
                        "{}{}",
                        release.channel,
                        if installed_versions.contains(&release.version) {
                            " ✓".green()
                        } else {
                            "".normal()
                        }
                    ),
                })
            }
        })
        .collect();

    let mut releases_table = Table::new(releases_rows);
    releases_table.with(Style::modern());

    println!("{}", releases_table.to_string());

    let channels_rows: Vec<ChannelRow> = vec![
        versions.current_releases.stable,
        versions.current_releases.beta,
    ]
    .iter()
    .filter_map(|release| {
        if args.channel != "all" && args.channel != release.channel {
            None
        } else {
            Some(ChannelRow {
                channel: release.channel.clone(),
                version: release.version.clone(),
                release_date: release.release_date,
            })
        }
    })
    .collect();

    let mut channels_table = Table::new(channels_rows);
    channels_table.with(Style::modern());

    println!();
    println!("Latest releases:");
    println!("{}", channels_table.to_string());

    return Ok(());
}

#[derive(Tabled)]
#[tabled(rename_all = "Upper Title Case")]
struct ReleaseRow {
    version: String,
    #[tabled(display = "format_date")]
    release_date: DateTime<Utc>,
    channel: String,
}

#[derive(Tabled)]
#[tabled(rename_all = "Upper Title Case")]
struct ChannelRow {
    channel: String,
    version: String,
    #[tabled(display = "format_date")]
    release_date: DateTime<Utc>,
}

fn format_date(date: &DateTime<Utc>) -> String {
    date.format("%b %e, %Y").to_string() // e.g., "Jun 25, 2025"
}
