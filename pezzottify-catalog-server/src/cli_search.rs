use anyhow::Result;
use clap::Parser;
use std::io;
use std::path::PathBuf;

mod catalog;
use catalog::load_catalog;

mod search;
use search::{SearchResult, SearchVault};

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
    pub path: PathBuf,
}

fn main() -> Result<()> {
    let cli_args = CliArgs::parse();
    let catalog = load_catalog(cli_args.path)?;
    let search_vault = SearchVault::new(&catalog);

    loop {
        println!("Please enter your search query:");

        let mut user_input = String::new();

        io::stdin()
            .read_line(&mut user_input)
            .expect("Failed to read line");

        let user_input = user_input.trim();

        let results: Vec<SearchResult> = search_vault.search(user_input).collect();
        if results.is_empty() {
            println!("No matches found for \"{}\".", user_input);
        } else {
            println!("Found {} matches for \"{}\":\n", results.len(), user_input);
        }
    }
}
