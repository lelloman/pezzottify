use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use crossterm::style::Stylize;
use std::{path::PathBuf, sync::Arc};

mod catalog_store;
mod cli_style;
mod search;
mod server;
mod sqlite_persistence;
mod user;

use cli_style::{
    box_chars, colors, get_prompt, get_styles, print_command_echo, print_empty_list, print_error,
    print_goodbye, print_help, print_key_value, print_key_value_highlight, print_list_item,
    print_section_footer, print_section_header, print_success, print_warning, print_welcome,
    CommandHelp, TableBuilder,
};
use user::UserManager;

use catalog_store::NullCatalogStore;
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
#[command(styles=get_styles(), name = "", disable_help_subcommand = true)]
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

    /// Show available commands.
    Help,

    /// Close this program.
    Exit,
}

enum CommandExecutionResult {
    Ok,
    Exit,
    Error(String),
}

fn get_commands_help() -> Vec<CommandHelp> {
    vec![
        CommandHelp {
            name: "add-user",
            args: "<handle>",
            description: "Create a new user",
        },
        CommandHelp {
            name: "user-handles",
            args: "",
            description: "List all user handles",
        },
        CommandHelp {
            name: "show",
            args: "<handle>",
            description: "Show user details",
        },
        CommandHelp {
            name: "add-login",
            args: "<handle> <password>",
            description: "Set password for user",
        },
        CommandHelp {
            name: "update-login",
            args: "<handle> <password>",
            description: "Update user password",
        },
        CommandHelp {
            name: "delete-login",
            args: "<handle>",
            description: "Remove password auth",
        },
        CommandHelp {
            name: "check-password",
            args: "<handle> <password>",
            description: "Verify password",
        },
        CommandHelp {
            name: "add-role",
            args: "<handle> <role>",
            description: "Assign role to user",
        },
        CommandHelp {
            name: "remove-role",
            args: "<handle> <role>",
            description: "Remove role from user",
        },
        CommandHelp {
            name: "list-roles",
            args: "",
            description: "Show available roles",
        },
        CommandHelp {
            name: "where",
            args: "",
            description: "Show database path",
        },
        CommandHelp {
            name: "help",
            args: "",
            description: "Show this help",
        },
        CommandHelp {
            name: "exit",
            args: "",
            description: "Exit the CLI",
        },
    ]
}

fn format_timestamp(time: &std::time::SystemTime) -> String {
    let duration = time
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();

    // Convert to a readable format
    let datetime = chrono::DateTime::from_timestamp(secs as i64, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| "Unknown".to_string());
    datetime
}

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
            print_command_echo(&line);
            println!();

            match cli.command {
                InnerCommand::AddUser { user_handle } => {
                    if let Err(err) = user_manager.add_user(&user_handle) {
                        return CommandExecutionResult::Error(format!("{}", err));
                    }
                    print_success(&format!("User '{}' created successfully", user_handle));
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
                    print_success(&format!(
                        "Password credentials set for user '{}'",
                        user_handle
                    ));
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
                    print_success(&format!("Password updated for user '{}'", user_handle));
                }
                InnerCommand::DeleteLogin { user_handle } => {
                    if let Err(err) = user_manager.delete_password_credentials(&user_handle) {
                        return CommandExecutionResult::Error(format!("{}", err));
                    }
                    print_success(&format!(
                        "Password credentials deleted for user '{}'",
                        user_handle
                    ));
                }
                InnerCommand::Show { user_handle } => {
                    let user_credentials = match user_manager.get_user_credentials(&user_handle) {
                        Ok(creds) => creds,
                        Err(err) => {
                            return CommandExecutionResult::Error(format!(
                                "Failed to get user credentials: {}",
                                err
                            ));
                        }
                    };
                    let user_tokens = match user_manager.get_user_tokens(&user_handle) {
                        Ok(tokens) => tokens,
                        Err(err) => {
                            return CommandExecutionResult::Error(format!(
                                "Failed to get user tokens: {}",
                                err
                            ));
                        }
                    };

                    if let Some(creds) = user_credentials {
                        let user_id = creds.user_id;

                        // User Info Section
                        print_section_header(&format!("User: {}", user_handle));
                        println!();
                        print_key_value_highlight("User ID", &user_id.to_string());

                        // Credentials Section
                        println!();
                        println!(
                            "  {} {}",
                            box_chars::DIAMOND.with(colors::MAGENTA),
                            "Credentials".with(colors::MAGENTA).bold()
                        );

                        if let Some(ref pwd_creds) = creds.username_password {
                            print_key_value("  Hasher", &format!("{:?}", pwd_creds.hasher));
                            print_key_value("  Created", &format_timestamp(&pwd_creds.created));
                            if let Some(ref last_tried) = pwd_creds.last_tried {
                                print_key_value("  Last Tried", &format_timestamp(last_tried));
                            }
                            if let Some(ref last_used) = pwd_creds.last_used {
                                print_key_value("  Last Used", &format_timestamp(last_used));
                            }
                        } else {
                            print_empty_list("No password credentials set");
                        }

                        // API Keys Section
                        println!();
                        println!(
                            "  {} {}",
                            box_chars::DIAMOND.with(colors::CYAN),
                            "API Keys".with(colors::CYAN).bold()
                        );
                        if creds.keys.is_empty() {
                            print_empty_list("No API keys");
                        } else {
                            for key in &creds.keys {
                                print_list_item(&format!("{:?}", key), 2);
                            }
                        }

                        // Auth Tokens Section
                        println!();
                        println!(
                            "  {} {}",
                            box_chars::DIAMOND.with(colors::PURPLE),
                            "Auth Tokens".with(colors::PURPLE).bold()
                        );
                        if user_tokens.is_empty() {
                            print_empty_list("No active tokens");
                        } else {
                            for token in user_tokens.iter() {
                                println!();
                                print_key_value("    User ID", &token.user_id.to_string());
                                print_key_value("    Created", &format_timestamp(&token.created));
                                if let Some(ref last_used) = token.last_used {
                                    print_key_value("    Last Used", &format_timestamp(last_used));
                                }
                                // Show truncated token value for security
                                let token_preview = if token.value.0.len() > 16 {
                                    format!("{}...", &token.value.0[..16])
                                } else {
                                    token.value.0.clone()
                                };
                                print_key_value("    Token", &token_preview);
                            }
                        }

                        // Roles Section
                        println!();
                        println!(
                            "  {} {}",
                            box_chars::DIAMOND.with(colors::ORANGE),
                            "Roles".with(colors::ORANGE).bold()
                        );
                        match user_manager.get_user_roles(user_id) {
                            Ok(roles) => {
                                if roles.is_empty() {
                                    print_empty_list("No roles assigned");
                                } else {
                                    for role in roles.iter() {
                                        print_list_item(&role.to_string(), 2);
                                    }
                                }
                            }
                            Err(err) => {
                                print_error(&format!("Failed to get roles: {}", err));
                            }
                        }

                        // Permissions Section
                        println!();
                        println!(
                            "  {} {}",
                            box_chars::DIAMOND.with(colors::GREEN),
                            "Resolved Permissions".with(colors::GREEN).bold()
                        );
                        match user_manager.get_user_permissions(user_id) {
                            Ok(permissions) => {
                                if permissions.is_empty() {
                                    print_empty_list("No permissions");
                                } else {
                                    for permission in permissions.iter() {
                                        print_list_item(&format!("{:?}", permission), 2);
                                    }
                                }
                            }
                            Err(err) => {
                                print_error(&format!("Failed to get permissions: {}", err));
                            }
                        }

                        print_section_footer();
                    } else {
                        print_warning(&format!("User '{}' not found", user_handle));
                    }
                }
                InnerCommand::UserHandles => {
                    match user_manager.get_all_user_handles() {
                        Ok(handles) => {
                            print_section_header("Registered Users");
                            println!();

                            if handles.is_empty() {
                                print_empty_list("No users registered");
                            } else {
                                let mut table = TableBuilder::new(vec!["#", "Handle"]);
                                for (i, handle) in handles.iter().enumerate() {
                                    table.add_row(vec![&(i + 1).to_string(), handle]);
                                }
                                table.print();
                            }

                            println!();
                            println!(
                                "  {} {}",
                                "Total:".with(colors::DIM),
                                format!("{} user(s)", handles.len())
                                    .with(colors::CYAN)
                                    .bold()
                            );
                            print_section_footer();
                        }
                        Err(err) => {
                            return CommandExecutionResult::Error(format!(
                                "Failed to get user handles: {}",
                                err
                            ));
                        }
                    }
                }
                InnerCommand::ListRoles => {
                    use user::UserRole;

                    print_section_header("Available Roles");
                    println!();

                    for role in &[UserRole::Admin, UserRole::Regular] {
                        println!(
                            "  {} {}",
                            box_chars::STAR.with(colors::YELLOW),
                            role.to_string().with(colors::CYAN).bold()
                        );

                        let permissions = role.permissions();
                        if permissions.is_empty() {
                            print_empty_list("No permissions");
                        } else {
                            for permission in permissions {
                                print_list_item(&format!("{:?}", permission), 2);
                            }
                        }
                        println!();
                    }

                    print_section_footer();
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
                        Ok(Some(creds)) => creds.user_id,
                        Ok(None) => {
                            return CommandExecutionResult::Error(format!(
                                "User '{}' not found",
                                user_handle
                            ));
                        }
                        Err(err) => {
                            return CommandExecutionResult::Error(format!(
                                "Failed to get user credentials: {}",
                                err
                            ));
                        }
                    };

                    if let Err(err) = user_manager.add_user_role(user_id, role_enum) {
                        return CommandExecutionResult::Error(format!("{}", err));
                    }
                    print_success(&format!(
                        "Role '{}' added to user '{}'",
                        role.with(colors::CYAN).bold(),
                        user_handle.with(colors::GREEN).bold()
                    ));
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
                        Ok(Some(creds)) => creds.user_id,
                        Ok(None) => {
                            return CommandExecutionResult::Error(format!(
                                "User '{}' not found",
                                user_handle
                            ));
                        }
                        Err(err) => {
                            return CommandExecutionResult::Error(format!(
                                "Failed to get user credentials: {}",
                                err
                            ));
                        }
                    };

                    if let Err(err) = user_manager.remove_user_role(user_id, role_enum) {
                        return CommandExecutionResult::Error(format!("{}", err));
                    }
                    print_success(&format!(
                        "Role '{}' removed from user '{}'",
                        role.with(colors::CYAN).bold(),
                        user_handle.with(colors::GREEN).bold()
                    ));
                }
                InnerCommand::Where => {
                    print_section_header("Database Location");
                    println!();
                    print_key_value_highlight("Path", &db_path);
                    print_section_footer();
                }
                InnerCommand::CheckPassword {
                    user_handle,
                    password,
                } => {
                    let user_credentials = match user_manager.get_user_credentials(&user_handle) {
                        Ok(Some(x)) => x,
                        Ok(None) => {
                            return CommandExecutionResult::Error(format!(
                                "User '{}' not found",
                                user_handle
                            ));
                        }
                        Err(err) => {
                            return CommandExecutionResult::Error(format!(
                                "Failed to get user credentials: {}",
                                err
                            ));
                        }
                    };
                    let password_credentials = match user_credentials.username_password {
                        Some(x) => x,
                        None => {
                            return CommandExecutionResult::Error(format!(
                                "User '{}' has no password set",
                                user_handle
                            ));
                        }
                    };
                    match password_credentials.hasher.verify(
                        password,
                        password_credentials.hash,
                        password_credentials.salt,
                    ) {
                        Ok(true) => {
                            print_success("Password is correct!");
                        }
                        Ok(false) => {
                            print_error("Wrong password");
                        }
                        Err(err) => {
                            return CommandExecutionResult::Error(format!(
                                "Could not verify password: {}",
                                err
                            ));
                        }
                    }
                }
                InnerCommand::Help => {
                    print_help(&get_commands_help());
                }
                InnerCommand::Exit => return CommandExecutionResult::Exit,
            }
        }

        Err(e) => {
            // For parse errors, show a styled error
            print_error(&format!("Invalid command: {}", e.kind()));
            println!();
            println!(
                "  {} Type '{}' for available commands",
                box_chars::ARROW_RIGHT.with(colors::DIM),
                "help".with(colors::GREEN).bold()
            );
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
    let catalog_store = Arc::new(NullCatalogStore);
    let mut user_manager = UserManager::new(catalog_store, Box::new(user_store));

    // Print welcome screen instead of clap help
    print_welcome(&auth_store_file_path.display().to_string());

    let config = Config::builder()
        .completion_type(CompletionType::List)
        .build();

    let mut rl = Editor::<MyHelper, FileHistory>::with_config(config)?;

    let helper = MyHelper::new();
    rl.set_helper(Some(helper));

    // Use the styled prompt
    let prompt = get_prompt();

    loop {
        let readline = rl.readline(&prompt);

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
                        print_goodbye();
                        break;
                    }
                    CommandExecutionResult::Error(err) => {
                        print_error(&err);
                        println!();
                        continue;
                    }
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                print_goodbye();
                break;
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                print_goodbye();
                break;
            }
            Err(e) => {
                print_error(&format!("Error: {:?}", e));
                break;
            }
        }
    }
    Ok(())
}
