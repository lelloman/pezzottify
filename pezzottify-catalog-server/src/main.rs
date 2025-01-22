use anyhow::{Context, Result};
use clap::Parser;
use std::{fmt::Debug, path::PathBuf};

mod catalog;
use catalog::Catalog;

mod search;
use search::SearchVault;

mod server;
use server::run_server;

fn parse_root_dir(s: &str) -> Result<PathBuf> {
    let original_path = PathBuf::from(s).canonicalize()?;
    if original_path.is_absolute() {
        return Ok(original_path);
    }
    let cwd = std::env::current_dir()?;
    Ok(cwd.join(original_path))
}

#[derive(Parser, Debug)]
struct CliArgs {
    #[clap(value_parser = parse_root_dir)]
    pub path: Option<PathBuf>,

    #[clap(long)]
    pub check_only: bool,

    #[clap(short, long, default_value_t = 3001)]
    pub port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli_args = CliArgs::parse();

    tracing_subscriber::fmt::init();
    let catalog_path = match cli_args.path {
        Some(path) => path,
        None => Catalog::infer_path().with_context(|| {
            "Could not infer catalog directory, please specifiy it explicityly."
        })?,
    };
    let catalog = catalog::load_catalog(catalog_path)?;

    if cli_args.check_only {
        return Ok(());
    }

    let search_vault = SearchVault::new(&catalog);
    run_server(catalog, search_vault, cli_args.port).await
}
