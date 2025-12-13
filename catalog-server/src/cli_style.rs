use clap::builder::styling::{AnsiColor, Color, Style};
use clap::builder::Styles;
use crossterm::style::{Attribute, Color as CtColor, Stylize};
use std::io::{self, Write};
use unicode_width::UnicodeWidthStr;

// ═══════════════════════════════════════════════════════════════════════════════
// Clap Styles
// ═══════════════════════════════════════════════════════════════════════════════

pub fn get_styles() -> Styles {
    clap::builder::Styles::styled()
        .usage(
            Style::new()
                .bold()
                .underline()
                .fg_color(Some(Color::Ansi(AnsiColor::Cyan))),
        )
        .header(
            Style::new()
                .bold()
                .underline()
                .fg_color(Some(Color::Ansi(AnsiColor::Cyan))),
        )
        .literal(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Green))),
        )
        .invalid(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Red))),
        )
        .error(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Red))),
        )
        .valid(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Green))),
        )
        .placeholder(Style::new().fg_color(Some(Color::Ansi(AnsiColor::BrightBlack))))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Color Palette - Modern Cyberpunk/Neon Theme
// ═══════════════════════════════════════════════════════════════════════════════

pub mod colors {
    use crossterm::style::Color;

    pub const CYAN: Color = Color::Rgb {
        r: 0,
        g: 255,
        b: 255,
    };
    pub const MAGENTA: Color = Color::Rgb {
        r: 255,
        g: 0,
        b: 255,
    };
    pub const PURPLE: Color = Color::Rgb {
        r: 180,
        g: 100,
        b: 255,
    };
    pub const PINK: Color = Color::Rgb {
        r: 255,
        g: 105,
        b: 180,
    };
    pub const GREEN: Color = Color::Rgb {
        r: 0,
        g: 255,
        b: 136,
    };
    pub const ORANGE: Color = Color::Rgb {
        r: 255,
        g: 165,
        b: 0,
    };
    pub const YELLOW: Color = Color::Rgb {
        r: 255,
        g: 255,
        b: 0,
    };
    pub const RED: Color = Color::Rgb {
        r: 255,
        g: 85,
        b: 85,
    };
    #[allow(dead_code)]
    pub const BLUE: Color = Color::Rgb {
        r: 100,
        g: 149,
        b: 237,
    };
    pub const DIM: Color = Color::Rgb {
        r: 128,
        g: 128,
        b: 128,
    };
    pub const WHITE: Color = Color::Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
}

// ═══════════════════════════════════════════════════════════════════════════════
// Box Drawing Characters
// ═══════════════════════════════════════════════════════════════════════════════

pub mod box_chars {
    // Double line box
    pub const DOUBLE_TOP_LEFT: &str = "╔";
    pub const DOUBLE_TOP_RIGHT: &str = "╗";
    pub const DOUBLE_BOTTOM_LEFT: &str = "╚";
    pub const DOUBLE_BOTTOM_RIGHT: &str = "╝";
    pub const DOUBLE_HORIZONTAL: &str = "═";
    pub const DOUBLE_VERTICAL: &str = "║";

    // Single line box
    #[allow(dead_code)]
    pub const SINGLE_TOP_LEFT: &str = "┌";
    #[allow(dead_code)]
    pub const SINGLE_TOP_RIGHT: &str = "┐";
    #[allow(dead_code)]
    pub const SINGLE_BOTTOM_LEFT: &str = "└";
    #[allow(dead_code)]
    pub const SINGLE_BOTTOM_RIGHT: &str = "┘";
    pub const SINGLE_HORIZONTAL: &str = "─";
    pub const SINGLE_VERTICAL: &str = "│";

    // Rounded box
    pub const ROUND_TOP_LEFT: &str = "╭";
    pub const ROUND_TOP_RIGHT: &str = "╮";
    pub const ROUND_BOTTOM_LEFT: &str = "╰";
    pub const ROUND_BOTTOM_RIGHT: &str = "╯";

    // T-junctions
    pub const T_LEFT: &str = "├";
    pub const T_RIGHT: &str = "┤";
    pub const T_TOP: &str = "┬";
    pub const T_BOTTOM: &str = "┴";
    pub const CROSS: &str = "┼";

    // Arrows and bullets
    pub const ARROW_RIGHT: &str = "▶";
    #[allow(dead_code)]
    pub const ARROW_LEFT: &str = "◀";
    pub const BULLET: &str = "●";
    pub const BULLET_EMPTY: &str = "○";
    pub const DIAMOND: &str = "◆";
    pub const STAR: &str = "★";
    pub const CHECK: &str = "✓";
    pub const CROSS_MARK: &str = "✗";
}

// ═══════════════════════════════════════════════════════════════════════════════
// Banner
// ═══════════════════════════════════════════════════════════════════════════════

pub fn print_banner() {
    let banner = r#"
    ██████╗ ███████╗███████╗███████╗ ██████╗ ████████╗████████╗██╗███████╗██╗   ██╗
    ██╔══██╗██╔════╝╚══███╔╝╚══███╔╝██╔═══██╗╚══██╔══╝╚══██╔══╝██║██╔════╝╚██╗ ██╔╝
    ██████╔╝█████╗    ███╔╝   ███╔╝ ██║   ██║   ██║      ██║   ██║█████╗   ╚████╔╝
    ██╔═══╝ ██╔══╝   ███╔╝   ███╔╝  ██║   ██║   ██║      ██║   ██║██╔══╝    ╚██╔╝
    ██║     ███████╗███████╗███████╗╚██████╔╝   ██║      ██║   ██║██║        ██║
    ╚═╝     ╚══════╝╚══════╝╚══════╝ ╚═════╝    ╚═╝      ╚═╝   ╚═╝╚═╝        ╚═╝
"#;

    // Print with gradient effect
    let lines: Vec<&str> = banner.lines().collect();
    let gradient_colors = [
        colors::CYAN,
        colors::CYAN,
        colors::PURPLE,
        colors::PURPLE,
        colors::MAGENTA,
        colors::MAGENTA,
        colors::PINK,
        colors::PINK,
    ];

    for (i, line) in lines.iter().enumerate() {
        let color = gradient_colors.get(i).unwrap_or(&colors::CYAN);
        println!("{}", line.with(*color).bold());
    }

    let subtitle = "  ═══════════════════  AUTH MANAGEMENT CLI  ═══════════════════";
    println!("{}", subtitle.with(colors::DIM));
    println!();
}

// ═══════════════════════════════════════════════════════════════════════════════
// Status Indicators
// ═══════════════════════════════════════════════════════════════════════════════

pub fn print_success(message: &str) {
    println!(
        " {} {}",
        box_chars::CHECK.to_string().with(colors::GREEN).bold(),
        message.with(colors::GREEN)
    );
}

pub fn print_error(message: &str) {
    println!(
        " {} {}",
        box_chars::CROSS_MARK.to_string().with(colors::RED).bold(),
        message.with(colors::RED)
    );
}

pub fn print_warning(message: &str) {
    println!(
        " {} {}",
        "⚠".with(colors::ORANGE).bold(),
        message.with(colors::ORANGE)
    );
}

#[allow(dead_code)]
pub fn print_info(message: &str) {
    println!(
        " {} {}",
        "ℹ".with(colors::BLUE).bold(),
        message.with(colors::BLUE)
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Section Headers
// ═══════════════════════════════════════════════════════════════════════════════

pub fn print_section_header(title: &str) {
    let width: usize = 60;
    let title_len = title.width();
    let padding = width.saturating_sub(title_len + 4) / 2;

    println!();
    print!("{}", box_chars::ROUND_TOP_LEFT.with(colors::CYAN));
    print!(
        "{}",
        box_chars::SINGLE_HORIZONTAL
            .repeat(padding)
            .with(colors::CYAN)
    );
    print!(
        " {} ",
        title.with(colors::CYAN).bold().attribute(Attribute::Italic)
    );
    print!(
        "{}",
        box_chars::SINGLE_HORIZONTAL
            .repeat(width.saturating_sub(title_len + 4 + padding))
            .with(colors::CYAN)
    );
    println!("{}", box_chars::ROUND_TOP_RIGHT.with(colors::CYAN));
}

pub fn print_section_footer() {
    let width = 60;
    print!("{}", box_chars::ROUND_BOTTOM_LEFT.with(colors::CYAN));
    print!(
        "{}",
        box_chars::SINGLE_HORIZONTAL
            .repeat(width)
            .with(colors::CYAN)
    );
    println!("{}", box_chars::ROUND_BOTTOM_RIGHT.with(colors::CYAN));
    println!();
}

// ═══════════════════════════════════════════════════════════════════════════════
// Key-Value Display
// ═══════════════════════════════════════════════════════════════════════════════

pub fn print_key_value(key: &str, value: &str) {
    println!(
        "  {} {} {}",
        box_chars::BULLET.with(colors::PURPLE),
        format!("{}:", key).with(colors::DIM),
        value.with(colors::WHITE)
    );
}

pub fn print_key_value_highlight(key: &str, value: &str) {
    println!(
        "  {} {} {}",
        box_chars::DIAMOND.with(colors::MAGENTA),
        format!("{}:", key).with(colors::CYAN).bold(),
        value.with(colors::GREEN).bold()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// List Display
// ═══════════════════════════════════════════════════════════════════════════════

pub fn print_list_item(item: &str, indent: usize) {
    let indent_str = "  ".repeat(indent);
    println!(
        "{}{}  {}",
        indent_str,
        box_chars::ARROW_RIGHT.with(colors::CYAN),
        item.with(colors::WHITE)
    );
}

#[allow(dead_code)]
pub fn print_list_item_styled(item: &str, color: CtColor, indent: usize) {
    let indent_str = "  ".repeat(indent);
    println!(
        "{}{}  {}",
        indent_str,
        box_chars::ARROW_RIGHT.with(color),
        item.with(color)
    );
}

pub fn print_empty_list(message: &str) {
    println!(
        "  {} {}",
        box_chars::BULLET_EMPTY.with(colors::DIM),
        message.with(colors::DIM).attribute(Attribute::Italic)
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Table Display
// ═══════════════════════════════════════════════════════════════════════════════

pub struct TableBuilder {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    col_widths: Vec<usize>,
}

impl TableBuilder {
    pub fn new(headers: Vec<&str>) -> Self {
        let col_widths: Vec<usize> = headers.iter().map(|h| h.width()).collect();
        TableBuilder {
            headers: headers.into_iter().map(String::from).collect(),
            rows: Vec::new(),
            col_widths,
        }
    }

    pub fn add_row(&mut self, row: Vec<&str>) {
        for (i, cell) in row.iter().enumerate() {
            if i < self.col_widths.len() {
                self.col_widths[i] = self.col_widths[i].max(cell.width());
            }
        }
        self.rows.push(row.into_iter().map(String::from).collect());
    }

    pub fn print(&self) {
        // Calculate total width (reserved for potential future use)
        let _total_width: usize =
            self.col_widths.iter().sum::<usize>() + (self.col_widths.len() * 3) + 1;

        // Top border
        print!("{}", box_chars::ROUND_TOP_LEFT.with(colors::CYAN));
        for (i, width) in self.col_widths.iter().enumerate() {
            print!(
                "{}",
                box_chars::SINGLE_HORIZONTAL
                    .repeat(width + 2)
                    .with(colors::CYAN)
            );
            if i < self.col_widths.len() - 1 {
                print!("{}", box_chars::T_TOP.with(colors::CYAN));
            }
        }
        println!("{}", box_chars::ROUND_TOP_RIGHT.with(colors::CYAN));

        // Header
        print!("{}", box_chars::SINGLE_VERTICAL.with(colors::CYAN));
        for (i, header) in self.headers.iter().enumerate() {
            let padding = self.col_widths[i] - header.width();
            print!(
                " {}{} ",
                header.clone().with(colors::CYAN).bold(),
                " ".repeat(padding)
            );
            print!("{}", box_chars::SINGLE_VERTICAL.with(colors::CYAN));
        }
        println!();

        // Header separator
        print!("{}", box_chars::T_LEFT.with(colors::CYAN));
        for (i, width) in self.col_widths.iter().enumerate() {
            print!(
                "{}",
                box_chars::SINGLE_HORIZONTAL
                    .repeat(width + 2)
                    .with(colors::CYAN)
            );
            if i < self.col_widths.len() - 1 {
                print!("{}", box_chars::CROSS.with(colors::CYAN));
            }
        }
        println!("{}", box_chars::T_RIGHT.with(colors::CYAN));

        // Rows
        for row in &self.rows {
            print!("{}", box_chars::SINGLE_VERTICAL.with(colors::CYAN));
            for (i, cell) in row.iter().enumerate() {
                let width = self.col_widths.get(i).unwrap_or(&0);
                let padding = width.saturating_sub(cell.width());
                print!(
                    " {}{} ",
                    cell.clone().with(colors::WHITE),
                    " ".repeat(padding)
                );
                print!("{}", box_chars::SINGLE_VERTICAL.with(colors::CYAN));
            }
            println!();
        }

        // Bottom border
        print!("{}", box_chars::ROUND_BOTTOM_LEFT.with(colors::CYAN));
        for (i, width) in self.col_widths.iter().enumerate() {
            print!(
                "{}",
                box_chars::SINGLE_HORIZONTAL
                    .repeat(width + 2)
                    .with(colors::CYAN)
            );
            if i < self.col_widths.len() - 1 {
                print!("{}", box_chars::T_BOTTOM.with(colors::CYAN));
            }
        }
        println!("{}", box_chars::ROUND_BOTTOM_RIGHT.with(colors::CYAN));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Prompt Styling
// ═══════════════════════════════════════════════════════════════════════════════

pub fn get_prompt() -> String {
    format!(
        "{}{}{} ",
        "❯".with(colors::CYAN).bold(),
        "❯".with(colors::PURPLE).bold(),
        "❯".with(colors::MAGENTA).bold(),
    )
}

pub fn print_command_echo(command: &str) {
    println!(
        "{}{}{}  {}",
        "❯".with(colors::CYAN).bold(),
        "❯".with(colors::PURPLE).bold(),
        "❯".with(colors::MAGENTA).bold(),
        command.with(colors::GREEN).bold()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Welcome Message
// ═══════════════════════════════════════════════════════════════════════════════

pub fn print_welcome(db_path: &str) {
    print_banner();

    let box_width = 64;
    let _inner_width = box_width - 4; // Reserved for potential future use

    // Top border
    print!("  {}", box_chars::DOUBLE_TOP_LEFT.with(colors::PURPLE));
    print!(
        "{}",
        box_chars::DOUBLE_HORIZONTAL
            .repeat(box_width)
            .with(colors::PURPLE)
    );
    println!("{}", box_chars::DOUBLE_TOP_RIGHT.with(colors::PURPLE));

    // Content
    let lines = [
        ("Database", db_path),
        ("Version", env!("APP_VERSION")),
    ];

    print!("  {}", box_chars::DOUBLE_VERTICAL.with(colors::PURPLE));
    print!("  {}  ", "Connected to user database".with(colors::GREEN));
    let padding = box_width - 2 - "Connected to user database".width() - 2;
    print!("{}", " ".repeat(padding));
    println!("{}", box_chars::DOUBLE_VERTICAL.with(colors::PURPLE));

    print!("  {}", box_chars::DOUBLE_VERTICAL.with(colors::PURPLE));
    print!("{}", " ".repeat(box_width));
    println!("{}", box_chars::DOUBLE_VERTICAL.with(colors::PURPLE));

    for (key, value) in lines {
        print!("  {}", box_chars::DOUBLE_VERTICAL.with(colors::PURPLE));
        let content = format!("  {} {}", format!("{}:", key).with(colors::DIM), value);
        let visible_len = key.len() + 2 + value.len() + 2;
        print!("{}", content);
        print!("{}", " ".repeat(box_width.saturating_sub(visible_len)));
        println!("{}", box_chars::DOUBLE_VERTICAL.with(colors::PURPLE));
    }

    print!("  {}", box_chars::DOUBLE_VERTICAL.with(colors::PURPLE));
    print!("{}", " ".repeat(box_width));
    println!("{}", box_chars::DOUBLE_VERTICAL.with(colors::PURPLE));

    print!("  {}", box_chars::DOUBLE_VERTICAL.with(colors::PURPLE));
    let help_msg = "  Type 'help' for available commands";
    print!("{}", help_msg.with(colors::DIM));
    print!("{}", " ".repeat(box_width - help_msg.len()));
    println!("{}", box_chars::DOUBLE_VERTICAL.with(colors::PURPLE));

    // Bottom border
    print!("  {}", box_chars::DOUBLE_BOTTOM_LEFT.with(colors::PURPLE));
    print!(
        "{}",
        box_chars::DOUBLE_HORIZONTAL
            .repeat(box_width)
            .with(colors::PURPLE)
    );
    println!("{}", box_chars::DOUBLE_BOTTOM_RIGHT.with(colors::PURPLE));
    println!();
}

// ═══════════════════════════════════════════════════════════════════════════════
// Help Display
// ═══════════════════════════════════════════════════════════════════════════════

pub struct CommandHelp {
    pub name: &'static str,
    pub args: &'static str,
    pub description: &'static str,
}

pub fn print_help(commands: &[CommandHelp]) {
    println!();
    print_section_header("Available Commands");
    println!();

    // Group commands by category
    let user_commands: Vec<_> = commands
        .iter()
        .filter(|c| matches!(c.name, "add-user" | "user-handles" | "show"))
        .collect();

    let auth_commands: Vec<_> = commands
        .iter()
        .filter(|c| {
            matches!(
                c.name,
                "add-login" | "update-login" | "delete-login" | "check-password"
            )
        })
        .collect();

    let role_commands: Vec<_> = commands
        .iter()
        .filter(|c| matches!(c.name, "add-role" | "remove-role" | "list-roles"))
        .collect();

    let system_commands: Vec<_> = commands
        .iter()
        .filter(|c| matches!(c.name, "where" | "exit" | "help"))
        .collect();

    fn print_command_group(title: &str, commands: &[&CommandHelp], color: CtColor) {
        println!(
            "  {} {}",
            box_chars::DIAMOND.with(color),
            title.with(color).bold()
        );
        for cmd in commands {
            println!(
                "      {} {}  {}",
                cmd.name.with(colors::GREEN).bold(),
                cmd.args.with(colors::DIM),
                cmd.description.with(colors::WHITE)
            );
        }
        println!();
    }

    print_command_group("User Management", &user_commands, colors::CYAN);
    print_command_group("Authentication", &auth_commands, colors::MAGENTA);
    print_command_group("Role Management", &role_commands, colors::PURPLE);
    print_command_group("System", &system_commands, colors::ORANGE);

    print_section_footer();
}

// ═══════════════════════════════════════════════════════════════════════════════
// Goodbye Message
// ═══════════════════════════════════════════════════════════════════════════════

pub fn print_goodbye() {
    println!();
    println!(
        "  {} {}",
        "👋".with(colors::CYAN),
        "Goodbye! Thanks for using Pezzottify Auth CLI"
            .with(colors::PURPLE)
            .bold()
    );
    println!();
}

// ═══════════════════════════════════════════════════════════════════════════════
// Flush Output
// ═══════════════════════════════════════════════════════════════════════════════

#[allow(dead_code)]
pub fn flush() {
    let _ = io::stdout().flush();
}
