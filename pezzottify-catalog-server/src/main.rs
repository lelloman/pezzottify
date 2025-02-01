use anyhow::{Context, Result};
use clap::Parser;
use std::{fmt::Debug, path::PathBuf};
use tracing::info;

mod catalog;
use catalog::Catalog;

mod search;
use search::{NoOpSearchVault, PezzotHashSearchVault, SearchVault};

mod file_auth_store;
use file_auth_store::FileAuthStore;

mod server;
use server::run_server;

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
    pub auth_store_file_path: Option<PathBuf>,

    #[clap(long)]
    pub check_only: bool,

    #[clap(short, long, default_value_t = 3001)]
    pub port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli_args = CliArgs::parse();

    tracing_subscriber::fmt::init();
    let catalog_path = match cli_args.catalog_path {
        Some(path) => path,
        None => Catalog::infer_path().with_context(|| {
            "Could not infer catalog directory, please specifiy it explicityly."
        })?,
    };
    let catalog = catalog::load_catalog(catalog_path)?;

    if cli_args.check_only {
        return Ok(());
    }

    let auth_store_file_path = match cli_args.auth_store_file_path {
        Some(path) => path,
        None => FileAuthStore::infer_path().with_context(|| {
            "Could not infer auth store file path, please specify it explicitly."
        })?,
    };
    let auth_store = Box::new(FileAuthStore::initialize(auth_store_file_path));
    info!("Indexing content for search...");

    #[cfg(not(feature = "no_search"))]
    let search_vault: Box<dyn SearchVault> = Box::new(PezzotHashSearchVault::new(&catalog));

    #[cfg(feature = "no_search")]
    let search_vault: Box<dyn SearchVault> = Box::new(NoOpSearchVault {});

    info!("Ready to serve!");
    run_server(catalog, search_vault, auth_store, cli_args.port).await
}
