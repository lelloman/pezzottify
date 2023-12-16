use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

mod catalog;

use catalog::Catalog;

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

    #[clap(long)]
    pub check_only: bool,
}

fn main() {
    let cli_args = CliArgs::parse();
    let catalog_result = Catalog::build(&cli_args.path);
    let problems = catalog_result.problems;
    let catalog = catalog_result.catalog;

    if cli_args.check_only {
        if !problems.is_empty() {
            println!("Found {} problems:", problems.len());
            for problem in problems.iter() {
                println!("- {:?}", problem);
            }
            println!("");
        }

        match (&catalog, problems.is_empty()) {
            (Some(_), true) => println!("Catalog checked, no issues found."),
            (Some(_), false) => println!("Catalog was built, but check the issues above."),
            (None, _) => {
                println!("Check the problems above, the catalog could not be initialized.")
            }
        }
        if let Some(catalog) = catalog {
            println!(
                "Catalog has:\n{} artists\n{} albums\n{} tracks",
                catalog.get_artists_count(),
                catalog.get_albums_count(),
                catalog.get_tracks_count()
            );
        }
        return;
    }
}
