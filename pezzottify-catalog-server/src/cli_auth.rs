use anyhow::{Context, Result};
use clap::{Subcommand, Parser};
use std::{io::{self, Write, BufRead}, path::PathBuf};

mod file_auth_store;
use file_auth_store::FileAuthStore;

mod server;
mod catalog;
mod search;

use server::{AuthManager, UserId};

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
    pub path: Option<PathBuf>,
}

#[derive(Parser)]
struct InnerCli {
    #[command(subcommand)]
    command: InnerCommand,
}

#[derive(Subcommand)]
enum InnerCommand {
    AddLogin {
        user_id: UserId,
        password: String,
    },
    UpdateLogin {
        user_id: UserId,
        password: String,
    },
    DeleteLogin {
        user_id: UserId,
    },
    Exit,
}

fn main() -> Result<()> {
    let cli_args = CliArgs::parse();
    let auth_store_file_path = match cli_args.path {
        Some(path) => path,
        None => FileAuthStore::infer_path().with_context(|| {
            "Could not infer auth store file path, please specify it explicitly."
        })?,
    };
    let auth_store = FileAuthStore::initialize(auth_store_file_path);
    let mut auth_manager = AuthManager::initialize(Box::new(auth_store))?;

    let stdin = io::stdin();
    let mut reader = stdin.lock();
    loop {
        print!("> ");
        io::stdout().flush().context("Failed to flush stdout")?;
        
        let mut line = String::new();
        reader.read_line(&mut line).context("Failed to read line")?;
        let line = line.trim();
        
        if line.is_empty() {
            continue;
        }

        let args = shlex::split(line).unwrap_or_else(|| line.split_whitespace().map(String::from).collect());
        let cli = InnerCli::try_parse_from(std::iter::once(" ").chain(args.iter().map(String::as_str)));

        match cli {
            Ok(cli) => match cli.command {
                InnerCommand::AddLogin { user_id, password } => {
                    if let Err(err) = auth_manager.create_password_credentials(&user_id, password) {
                        eprintln!("Something went wrong: {}", err);
                        continue;
                    }
                }
                InnerCommand::UpdateLogin { user_id, password } => {
                    if let Err(err) = auth_manager.update_password_credentials(&user_id, password) {
                        eprintln!("Something went wrong: {}", err);
                        continue;
                    }
                }
                InnerCommand::DeleteLogin { user_id } => {
                    if let Err(err) = auth_manager.delete_password_credentials(&user_id) {
                        eprintln!("Something went wrong: {}", err);
                        continue;
                    }
                }
                InnerCommand::Exit => break,
            },
            Err(e) => {
                eprintln!("{}", e);
                continue;
            }
        }
        println!("Done.");
    }
    Ok(())
}
