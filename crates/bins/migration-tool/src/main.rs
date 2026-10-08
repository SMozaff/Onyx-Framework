//! ONYX schema migration CLI.

mod migrations;

use std::{env, path::PathBuf, str::FromStr};

use anyhow::{bail, Context};
use migrations::{create_migration, migrate_down, migrate_up, migration_status, DatabaseKind};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().json().with_target(false).init();

    let mut arguments = env::args().skip(1);
    let command = arguments.next().unwrap_or_else(|| "help".to_string());
    let kind = DatabaseKind::from_str(
        &env::var("ONYX_DATABASE_KIND").unwrap_or_else(|_| "postgres".to_string()),
    )?;
    let database_url = env::var("DATABASE_URL").unwrap_or_default();

    match command.as_str() {
        "up" => {
            require_database_url(&database_url)?;
            migrate_up(kind, &database_url).await?;
            println!("migrations applied and idempotency pass completed");
        }
        "down" => {
            require_database_url(&database_url)?;
            let remaining: Vec<String> = arguments.collect();
            let target = parse_target(&remaining)?;
            migrate_down(kind, &database_url, target).await?;
            println!("database rolled back to target {target}");
        }
        "status" => {
            require_database_url(&database_url)?;
            let rows = migration_status(kind, &database_url).await?;
            println!("VERSION\tSTATE\tDESCRIPTION\tINSTALLED_ON");
            for row in rows {
                let state = match (row.installed, row.success) {
                    (true, true) => "applied",
                    (true, false) => "failed",
                    (false, _) => "pending",
                };
                println!(
                    "{}\t{}\t{}\t{}",
                    row.version,
                    state,
                    row.description,
                    row.installed_on.unwrap_or_else(|| "-".to_string())
                );
            }
        }
        "create" => {
            let name = arguments
                .next()
                .context("usage: migration-tool create <name>")?;
            let root = workspace_root()?;
            for path in create_migration(&root, &name)? {
                println!("created {}", path.display());
            }
        }
        "help" | "--help" | "-h" => print_help(),
        other => bail!("unknown command '{other}'\n\n{}", help_text()),
    }

    Ok(())
}

fn parse_target(arguments: &[String]) -> anyhow::Result<i64> {
    for window in arguments.windows(2) {
        if window[0] == "--target" {
            return window[1]
                .parse::<i64>()
                .context("--target must be an integer migration version");
        }
    }
    Ok(0)
}

fn require_database_url(database_url: &str) -> anyhow::Result<()> {
    if database_url.is_empty() {
        bail!("DATABASE_URL must be set for up, down, and status");
    }
    Ok(())
}

fn workspace_root() -> anyhow::Result<PathBuf> {
    let start = env::current_dir()?;
    for ancestor in start.ancestors() {
        if ancestor.join("Cargo.toml").exists() && ancestor.join("migrations").exists() {
            return Ok(ancestor.to_path_buf());
        }
    }
    bail!("could not locate the ONYX workspace root")
}

fn print_help() {
    println!("{}", help_text());
}

fn help_text() -> &'static str {
    "ONYX migration-tool\n\n\
Usage:\n  migration-tool up\n  migration-tool down --target <version>\n  migration-tool status\n  migration-tool create <name>\n\n\
Environment:\n  DATABASE_URL       PostgreSQL or SQLite connection URL\n  ONYX_DATABASE_KIND postgres (default) or sqlite"
}
