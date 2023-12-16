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
    let catalog = match Catalog::build(&cli_args.path) {
        Ok(x) => x,
        Err(x) => {
            eprintln!("Could not parse catalog.\n{:?}", x);
            return;
        }
    };

    if cli_args.check_only {
        println!("Performed check only, catalog is OK.");
        println!(
            "Catalog has:\n{} artists\n{} albums\n{} tracks",
            catalog.get_artists_count(),
            catalog.get_albums_count(),
            catalog.get_tracks_count()
        );
        return;
    }
}
