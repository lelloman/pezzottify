use anyhow::{Context, Result};
use clap::Parser;
use std::{fmt::Debug, path::PathBuf};
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::{
    filter::Directive, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

mod catalog;
use catalog::Catalog;

mod search;
use search::{NoOpSearchVault, PezzotHashSearchVault, SearchVault};

mod server;
use server::{run_server, RequestsLoggingLevel};

mod sqlite_persistence;

mod user;
use user::SqliteUserStore;

fn parse_path(s: &str) -> Result<PathBuf> {
    let original_path = PathBuf::from(s).canonicalize()?;
    if original_path.is_absolute() {
        return Ok(original_path);
    }
    let cwd = std::env::current_dir()?;
    Ok(cwd.join(original_path))
}

#[derive(Parser, Debug)]
struct CliArgs {
    #[clap(value_parser = parse_path)]
    pub catalog_path: Option<PathBuf>,

    #[clap(value_parser = parse_path)]
    pub user_store_file_path: Option<PathBuf>,

    #[clap(long)]
    pub check_only: bool,

    #[clap(short, long, default_value_t = 3001)]
    pub port: u16,

    #[clap(long, default_value = "path")]
    pub logging_level: RequestsLoggingLevel,

    #[clap(long, default_value_t = 3600)]
    pub content_cache_age_sec: usize,

    #[clap(long)]
    pub frontend_dir_path: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli_args = CliArgs::parse();

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .with_env_var("LOG_LEVEL")
                .from_env_lossy(),
        )
        .try_init()
        .unwrap();

    let catalog_path = match cli_args.catalog_path {
        Some(path) => path,
        None => Catalog::infer_path().with_context(|| {
            "Could not infer catalog directory, please specifiy it explicityly."
        })?,
    };

    info!("Loading catalog...");
    let catalog = catalog::load_catalog(catalog_path)?;

    if cli_args.check_only {
        return Ok(());
    }

    let user_store_file_path = match cli_args.user_store_file_path {
        Some(path) => path,
        None => SqliteUserStore::infer_path()
            .with_context(|| "Could not infer DB file path, please specify it explicitly.")?,
    };
    let user_store = Box::new(SqliteUserStore::new(&user_store_file_path)?);
    info!("Indexing content for search...");

    #[cfg(not(feature = "no_search"))]
    let search_vault: Box<dyn SearchVault> = Box::new(PezzotHashSearchVault::new(&catalog));

    #[cfg(feature = "no_search")]
    let search_vault: Box<dyn SearchVault> = Box::new(NoOpSearchVault {});

    info!("Ready to serve!");
    run_server(
        catalog,
        search_vault,
        user_store,
        cli_args.logging_level,
        cli_args.port,
        cli_args.content_cache_age_sec,
        cli_args.frontend_dir_path,
    )
    .await
}
