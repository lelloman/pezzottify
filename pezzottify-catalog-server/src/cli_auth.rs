use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
};

mod catalog;
mod cli_style;
mod search;
mod server;
mod user;

use cli_style::get_styles;
use user::UserManager;

mod sqlite_persistence;
use sqlite_persistence::SqliteUserStore;

fn parse_path(s: &str) -> Result<PathBuf> {
    let original_path = PathBuf::from(s);
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
    AddLogin {
        user_handle: String,
        password: String,
    },

    /// Change the password of a user, fails if no password was set.
    UpdateLogin {
        user_handle: String,
        password: String,
    },

    /// Deletes the password authentication for a given user.
    DeleteLogin { user_handle: String },

    /// Shows authentication information of a given user.
    Show { user_handle: String },

    /// Verifies the password of a given user, it doesn't make any
    /// persistent change, nor it creates any token, it just
    /// compares the password hash.
    CheckPassword {
        user_handle: String,
        password: String,
    },

    /// Shows all user handles.
    UserHandles,

    /// Close this program.
    Exit,
}

fn main() -> Result<()> {
    let cli_args = CliArgs::parse();
    let auth_store_file_path = match cli_args.path {
        Some(path) => path,
        None => SqliteUserStore::infer_path()
            .with_context(|| "Could not infer DB file path, please specify it explicitly.")?,
    };
    let user_store = SqliteUserStore::new(auth_store_file_path)?;
    let mut user_manager = UserManager::new(Box::new(user_store));

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
                InnerCommand::AddLogin {
                    user_handle,
                    password,
                } => {
                    if let Err(err) =
                        user_manager.create_password_credentials(&user_handle, password)
                    {
                        eprintln!("Something went wrong: {}", err);
                        continue;
                    }
                }
                InnerCommand::UpdateLogin {
                    user_handle,
                    password,
                } => {
                    if let Err(err) =
                        user_manager.update_password_credentials(&user_handle, password)
                    {
                        eprintln!("Something went wrong: {}", err);
                        continue;
                    }
                }
                InnerCommand::DeleteLogin { user_handle } => {
                    if let Err(err) = user_manager.delete_password_credentials(&user_handle) {
                        eprintln!("Something went wrong: {}", err);
                        continue;
                    }
                }
                InnerCommand::Show { user_handle } => {
                    let user_credentials = user_manager.get_user_credentials(&user_handle);
                    let user_token = user_manager.get_user_tokens(&user_handle);
                    println!("{:#?}", user_credentials);
                    for token in user_token.iter() {
                        println!("{:#?}", token);
                    }
                }
                InnerCommand::UserHandles => {
                    println!("{:#?}", user_manager.get_all_user_handles());
                }
                InnerCommand::CheckPassword {
                    user_handle,
                    password,
                } => {
                    let user_credentials = match user_manager.get_user_credentials(&user_handle) {
                        Some(x) => x,
                        None => {
                            eprintln!("User {} not found.", user_handle);
                            continue;
                        }
                    };
                    let password_credentials = match user_credentials.username_password {
                        Some(x) => x,
                        None => {
                            eprintln!("User {} has no password set.", user_handle);
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
