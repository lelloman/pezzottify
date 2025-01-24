use anyhow::{Context, Result};
use clap::Parser;
use std::io;
use std::path::PathBuf;

mod catalog;
use catalog::{load_catalog, Catalog};

mod search;
use search::{HashedItemType, SearchResult, SearchVault};

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
}

fn print_result(catalog: &Catalog, result: SearchResult) {
    let name = match result.item_type {
        HashedItemType::Artist => catalog.get_artist(&result.item_id).map(|a| a.name),
        HashedItemType::Track => catalog.get_track(&result.item_id).map(|t| t.name),
        HashedItemType::Album => catalog.get_album(&result.item_id).map(|a| a.name),
    };
    println!(
        "{} -> {:?} {}->{} - {}",
        name.unwrap_or_else(|| "ERROR".to_string()),
        result.item_type,
        result.score,
        result.adjusted_score,
        result.item_id,
    );
}

fn main() -> Result<()> {
    let cli_args = CliArgs::parse();
    let catalog_path = match cli_args.path {
        Some(path) => path,
        None => Catalog::infer_path()
            .with_context(|| "Could not infer catalog directory, please specify it explicitly.")?,
    };
    println!("Cli Search loading catalog at {}...", catalog_path.canonicalize().unwrap().display());

    let catalog = load_catalog(catalog_path)?;
    let search_vault = SearchVault::new(&catalog);
    println!("Done!");

    loop {
        println!("Please enter your search query:");

        let mut user_input = String::new();

        io::stdin()
            .read_line(&mut user_input)
            .expect("Failed to read line");

        let user_input = user_input.trim();

        let results: Vec<SearchResult> = search_vault.search(user_input, 60).collect();
        if results.is_empty() {
            println!("No matches found for \"{}\".", user_input);
        } else {
            println!("Found {} matches for \"{}\":\n", results.len(), user_input);
            for result in results {
                print_result(&catalog, result);
            }
        }
        println!("\n");
    }
}
