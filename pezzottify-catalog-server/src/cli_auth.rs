use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
};

mod file_auth_store;
use file_auth_store::FileAuthStore;

mod catalog;
mod cli_style;
mod search;
mod server;

use cli_style::get_styles;
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
#[command(styles=get_styles())]
struct CliArgs {
    #[clap(value_parser = parse_path)]
    pub path: Option<PathBuf>,
}

#[derive(Parser)]
#[command(styles=get_styles(),name = "")]
struct InnerCli {
    #[command(subcommand)]
    command: InnerCommand,
}

#[derive(Subcommand)]
enum InnerCommand {
    /// Creates a password authentication for the given user id.
    /// Fails if the user already has a password set.
    AddLogin { user_id: UserId, password: String },

    /// Change the password of a user, fails if no password was set.
    UpdateLogin { user_id: UserId, password: String },

    /// Deletes the password authentication for a given user.
    DeleteLogin { user_id: UserId },

    /// Shows authentication information of a given user.
    Show { user_id: UserId },

    /// Verifies the password of a given user, it doesn't make any
    /// persistent change, nor it creates any token, it just
    /// compares the password hash.
    CheckPassword { user_id: UserId, password: String },

    /// Shows all user ids.
    UserIds,

    /// Close this program.
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

    InnerCli::command().print_long_help()?;
    loop {
        print!("> ");
        io::stdout().flush().context("Failed to flush stdout")?;

        let mut line = String::new();
        reader.read_line(&mut line).context("Failed to read line")?;
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        let args = shlex::split(line)
            .unwrap_or_else(|| line.split_whitespace().map(String::from).collect());

        let cli =
            InnerCli::try_parse_from(std::iter::once(" ").chain(args.iter().map(String::as_str)));

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
                InnerCommand::Show { user_id } => {
                    let user_credentials = auth_manager.get_user_credentials(&user_id);
                    let user_token = auth_manager.get_user_tokens(&user_id);
                    println!("{:#?}", user_credentials);
                    for token in user_token.iter() {
                        println!("{:#?}", token);
                    }
                }
                InnerCommand::UserIds => {
                    println!("{:#?}", auth_manager.get_all_user_ids());
                }
                InnerCommand::CheckPassword { user_id, password } => {
                    let user_credentials = match auth_manager.get_user_credentials(&user_id) {
                        Some(x) => x,
                        None => {
                            eprintln!("User {} not found.", user_id);
                            continue;
                        }
                    };
                    let password_credentials = match user_credentials.username_password {
                        Some(x) => x,
                        None => {
                            eprintln!("User {} has no password set.", user_id);
                            continue;
                        }
                    };
                    let msg = match password_credentials.hasher.verify(
                        password,
                        password_credentials.hash,
                        password_credentials.salt,
                    ) {
                        Ok(true) => "The password provided is correct!",
                        Ok(false) => "Wrong password.",
                        Err(err) => &format!(
                            "Could not verify the password, something went wrong: {}",
                            err
                        ),
                    };
                    println!("{}", msg);
                }
                InnerCommand::Exit => break,
            },
            Err(e) => {
                if let Err(_) = e.print() {
                    println!("{}", e);
                }
                continue;
            }
        }
    }
    Ok(())
}
