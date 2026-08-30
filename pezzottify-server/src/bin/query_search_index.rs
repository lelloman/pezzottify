use anyhow::{bail, Result};
use clap::{Parser, ValueEnum};
use pezzottify_server::backup::DbRegistry;
use pezzottify_server::search::{Fts5LevenshteinSearchVault, HashedItemType, SearchVault};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ItemType {
    Artist,
    Album,
    Track,
}

impl From<ItemType> for HashedItemType {
    fn from(value: ItemType) -> Self {
        match value {
            ItemType::Artist => Self::Artist,
            ItemType::Album => Self::Album,
            ItemType::Track => Self::Track,
        }
    }
}

#[derive(Debug, Parser)]
#[command(about = "Run bounded smoke queries against an existing search index")]
struct Args {
    #[arg(long)]
    search_db: PathBuf,

    #[arg(long, required = true)]
    query: Vec<String>,

    #[arg(long, default_value_t = 20)]
    limit: usize,

    #[arg(long, value_enum)]
    item_type: Vec<ItemType>,

    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    available_only: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if !args.search_db.is_file() {
        bail!(
            "search database does not exist: {}",
            args.search_db.display()
        );
    }
    let vault = Fts5LevenshteinSearchVault::new_lazy(&args.search_db, &DbRegistry::new())?;
    let filter = (!args.item_type.is_empty()).then(|| {
        args.item_type
            .iter()
            .copied()
            .map(HashedItemType::from)
            .collect::<Vec<_>>()
    });

    for query in args.query {
        let results = vault.search_expanded_with_availability(
            &query,
            args.limit,
            filter.clone(),
            args.available_only,
        );
        println!("query={query:?} results={}", results.len());
        for (rank, result) in results.iter().enumerate() {
            println!(
                "{}\t{:?}\t{}\t{}\t{}",
                rank + 1,
                result.item_type,
                result.item_id,
                result.adjusted_score,
                result.matchable_text
            );
        }
    }
    Ok(())
}
