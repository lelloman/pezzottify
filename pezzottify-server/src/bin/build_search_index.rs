use anyhow::{bail, Context, Result};
use clap::Parser;
use pezzottify_server::backup::DbRegistry;
use pezzottify_server::catalog_store::{CatalogStore, SqliteCatalogStore};
use pezzottify_server::search::{
    Fts5LevenshteinSearchVault, IndexState, SearchBuildOptions, SearchVault,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(about = "Build or resume the FTS search index without starting the server")]
struct Args {
    #[arg(long)]
    catalog_db: PathBuf,

    #[arg(long)]
    search_db: PathBuf,

    #[arg(long, default_value_t = 2)]
    catalog_read_pool_size: usize,

    #[arg(long, default_value_t = 10)]
    poll_interval_secs: u64,

    #[arg(long, default_value_t = 200_000)]
    batch_size: usize,

    #[arg(long, default_value_t = 8)]
    preparation_threads: usize,

    /// Store only available IDs in the availability lookup. Unavailable
    /// entities are still fully indexed in FTS.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    sparse_availability: bool,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init()
        .ok();

    let args = Args::parse();
    if !args.catalog_db.is_file() {
        bail!(
            "catalog database does not exist: {}",
            args.catalog_db.display()
        );
    }
    let search_parent = args
        .search_db
        .parent()
        .context("search database path must have a parent directory")?;
    if !search_parent.is_dir() {
        bail!(
            "search database directory does not exist: {}",
            search_parent.display()
        );
    }
    if args.catalog_read_pool_size == 0 {
        bail!("catalog read pool size must be greater than zero");
    }

    let registry = DbRegistry::new();
    let catalog = Arc::new(SqliteCatalogStore::new(
        &args.catalog_db,
        search_parent,
        args.catalog_read_pool_size,
        &registry,
    )?);
    let vault = Arc::new(Fts5LevenshteinSearchVault::new_lazy_with_build_options(
        &args.search_db,
        &registry,
        SearchBuildOptions {
            batch_size: args.batch_size,
            preparation_threads: args.preparation_threads,
            sparse_availability: args.sparse_availability,
        },
    )?);

    vault.start_background_build(catalog as Arc<dyn CatalogStore>);

    loop {
        let stats = vault.get_stats();
        match stats.state {
            IndexState::Ready => {
                info!(
                    indexed_items = stats.indexed_items,
                    "Search index build ready"
                );
                return Ok(());
            }
            IndexState::Failed { error } => bail!("search index build failed: {error}"),
            IndexState::Building { processed, total } => {
                info!(processed, total, "Search index build running");
            }
            IndexState::Empty => info!("Waiting for search index build to start"),
        }
        std::thread::sleep(Duration::from_secs(args.poll_interval_secs));
    }
}
