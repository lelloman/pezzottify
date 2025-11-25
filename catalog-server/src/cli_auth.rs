use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

mod catalog;
mod cli_style;
mod search;
mod server;
mod sqlite_persistence;
mod user;

use cli_style::get_styles;
use user::UserManager;

use catalog::Catalog;
use user::SqliteUserStore;

use rustyline::{
    completion::Completer,
    highlight::Highlighter,
    history::FileHistory,
    validate::Validator,
    CompletionType, Config, Editor, Helper,
};

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
    /// Creates a user with the given handle.
    AddUser { user_handle: String },

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

    /// Shows all available roles and their permissions.
    ListRoles,

    /// Adds a role to a user.
    AddRole {
        user_handle: String,
        role: String,
    },

    /// Removes a role from a user.
    RemoveRole {
        user_handle: String,
        role: String,
    },

    /// Shows the path of the current auth db.
    Where,

    /// Close this program.
    Exit,
}

enum CommandExecutionResult {
    Ok,
    Exit,
    Error(String),
}

const PROMPT: &str = ">> ";

fn execute_command(
    line: String,
    user_manager: &mut UserManager,
    db_path: String,
) -> CommandExecutionResult {
    if line.is_empty() {
        return CommandExecutionResult::Ok;
    }

    let args =
        shlex::split(&line).unwrap_or_else(|| line.split_whitespace().map(String::from).collect());

    let cli = InnerCli::try_parse_from(std::iter::once(" ").chain(args.iter().map(String::as_str)));

    match cli {
        Ok(cli) => {
            println!("{} {}", PROMPT, &line);
            match cli.command {
                InnerCommand::AddUser { user_handle } => {
                    if let Err(err) = user_manager.add_user(&user_handle) {
                        return CommandExecutionResult::Error(format!("{}", err));
                    }
                }
                InnerCommand::AddLogin {
                    user_handle,
                    password,
                } => {
                    if let Err(err) =
                        user_manager.create_password_credentials(&user_handle, password)
                    {
                        return CommandExecutionResult::Error(format!("{}", err));
                    }
                }
                InnerCommand::UpdateLogin {
                    user_handle,
                    password,
                } => {
                    if let Err(err) =
                        user_manager.update_password_credentials(&user_handle, password)
                    {
                        return CommandExecutionResult::Error(format!("{}", err));
                    }
                }
                InnerCommand::DeleteLogin { user_handle } => {
                    if let Err(err) = user_manager.delete_password_credentials(&user_handle) {
                        return CommandExecutionResult::Error(format!("{}", err));
                    }
                }
                InnerCommand::Show { user_handle } => {
                    let user_credentials = user_manager.get_user_credentials(&user_handle);
                    let user_token = user_manager.get_user_tokens(&user_handle);

                    println!("User Credentials:");
                    println!("{:#?}", user_credentials);

                    println!("\nAuth Tokens:");
                    for token in user_token.iter() {
                        println!("{:#?}", token);
                    }

                    // Get user_id to fetch roles and permissions
                    if let Some(creds) = user_credentials {
                        let user_id = creds.user_id;

                        // Show roles
                        match user_manager.get_user_roles(user_id) {
                            Ok(roles) => {
                                println!("\nRoles:");
                                if roles.is_empty() {
                                    println!("  (no roles assigned)");
                                } else {
                                    for role in roles.iter() {
                                        println!("  - {}", role.to_string());
                                    }
                                }
                            }
                            Err(err) => {
                                println!("\nFailed to get roles: {}", err);
                            }
                        }

                        // Show resolved permissions
                        match user_manager.get_user_permissions(user_id) {
                            Ok(permissions) => {
                                println!("\nResolved Permissions:");
                                if permissions.is_empty() {
                                    println!("  (no permissions)");
                                } else {
                                    for permission in permissions.iter() {
                                        println!("  - {:?}", permission);
                                    }
                                }
                            }
                            Err(err) => {
                                println!("\nFailed to get permissions: {}", err);
                            }
                        }
                    }
                }
                InnerCommand::UserHandles => {
                    println!("{:#?}", user_manager.get_all_user_handles());
                }
                InnerCommand::ListRoles => {
                    use user::UserRole;
                    println!("Available Roles:\n");
                    for role in &[UserRole::Admin, UserRole::Regular] {
                        println!("Role: {}", role.to_string());
                        println!("Permissions:");
                        for permission in role.permissions() {
                            println!("  - {:?}", permission);
                        }
                        println!();
                    }
                }
                InnerCommand::AddRole { user_handle, role } => {
                    use user::UserRole;
                    let role_enum = match UserRole::from_str(&role) {
                        Some(r) => r,
                        None => {
                            return CommandExecutionResult::Error(format!(
                                "Invalid role '{}'. Valid roles are: Admin, Regular",
                                role
                            ));
                        }
                    };

                    let user_id = match user_manager.get_user_credentials(&user_handle) {
                        Some(creds) => creds.user_id,
                        None => {
                            return CommandExecutionResult::Error(format!(
                                "User '{}' not found",
                                user_handle
                            ));
                        }
                    };

                    if let Err(err) = user_manager.add_user_role(user_id, role_enum) {
                        return CommandExecutionResult::Error(format!("{}", err));
                    }
                    println!("Role '{}' added to user '{}'", role, user_handle);
                }
                InnerCommand::RemoveRole { user_handle, role } => {
                    use user::UserRole;
                    let role_enum = match UserRole::from_str(&role) {
                        Some(r) => r,
                        None => {
                            return CommandExecutionResult::Error(format!(
                                "Invalid role '{}'. Valid roles are: Admin, Regular",
                                role
                            ));
                        }
                    };

                    let user_id = match user_manager.get_user_credentials(&user_handle) {
                        Some(creds) => creds.user_id,
                        None => {
                            return CommandExecutionResult::Error(format!(
                                "User '{}' not found",
                                user_handle
                            ));
                        }
                    };

                    if let Err(err) = user_manager.remove_user_role(user_id, role_enum) {
                        return CommandExecutionResult::Error(format!("{}", err));
                    }
                    println!("Role '{}' removed from user '{}'", role, user_handle);
                }
                InnerCommand::Where => {
                    println!("{}", db_path);
                }
                InnerCommand::CheckPassword {
                    user_handle,
                    password,
                } => {
                    let user_credentials = match user_manager.get_user_credentials(&user_handle) {
                        Some(x) => x,
                        None => {
                            return CommandExecutionResult::Error(format!(
                                "User {} not found.",
                                user_handle
                            ));
                        }
                    };
                    let password_credentials = match user_credentials.username_password {
                        Some(x) => x,
                        None => {
                            return CommandExecutionResult::Error(format!(
                                "User {} has no password set.",
                                user_handle
                            ));
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
                InnerCommand::Exit => return CommandExecutionResult::Exit,
            }
        }

        Err(e) => {
            if let Err(_) = e.print() {
                println!("{}", e);
            }
        }
    }
    CommandExecutionResult::Ok
}

#[derive(rustyline_derive::Hinter)]
struct MyHelper {
    commands_names: Vec<String>,
}

impl MyHelper {
    pub fn new() -> Self {
        let commands_names: Vec<String> = InnerCli::command()
            .get_subcommands()
            .map(|sc| sc.get_name().to_string())
            .collect();

        MyHelper { commands_names }
    }
}

impl Completer for MyHelper {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        _pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<String>)> {
        if line.contains(" ") {
            return Ok((0, Vec::with_capacity(0)));
        }
        let matches = self
            .commands_names
            .iter()
            .filter(|c| c.starts_with(line))
            .map(|c| c.to_string())
            .collect::<Vec<_>>();

        Ok((0, matches))
    }
}

impl Highlighter for MyHelper {}
impl Validator for MyHelper {}
impl Helper for MyHelper {}

fn main() -> Result<()> {
    let cli_args = CliArgs::parse();
    let auth_store_file_path = match cli_args.path {
        Some(path) => path,
        None => SqliteUserStore::infer_path().with_context(|| {
            "Could not infer UserStore DB file path, please specify it explicitly."
        })?,
    };
    let user_store = SqliteUserStore::new(auth_store_file_path.clone())?;
    let catalog = Arc::new(Mutex::new(Catalog::dummy()));
    let mut user_manager = UserManager::new(catalog, Box::new(user_store));

    InnerCli::command().print_long_help()?;

    let config = Config::builder()
        .completion_type(CompletionType::List)
        .build();

    let mut rl = Editor::<MyHelper, FileHistory>::with_config(config)?;

    let helper = MyHelper::new();
    rl.set_helper(Some(helper));
    let _ = rl.clear_screen();

    loop {
        let readline = rl.readline(PROMPT);

        let _ = rl.clear_screen();
        match readline {
            Ok(line) => {
                let _ = rl.add_history_entry(&line);
                match execute_command(
                    line,
                    &mut user_manager,
                    auth_store_file_path.display().to_string(),
                ) {
                    CommandExecutionResult::Ok => {}
                    CommandExecutionResult::Exit => {
                        break;
                    }
                    CommandExecutionResult::Error(err) => {
                        eprintln!("Error: {:?}", err);
                        continue;
                    }
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                println!("CTRL-D: exiting.");
                break;
            }
            Err(e) => {
                println!("Error: {:?}", e);
                break;
            }
        }
    }
    Ok(())
}
