use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

pub const SPINNER_TICK_CHARS: &str = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏";

pub fn spinner(message: &str) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_chars(SPINNER_TICK_CHARS),
    );
    spinner.enable_steady_tick(Duration::from_millis(80));
    spinner.set_message(message.to_string());
    spinner
}

pub fn status(action: &str, message: impl std::fmt::Display) {
    println!(
        "{} {} {}",
        style("→").cyan(),
        style(action).magenta().bold(),
        style(message.to_string()).cyan()
    );
}

pub fn status_dim(action: &str, message: impl std::fmt::Display) {
    println!(
        "  {} {}",
        style(format!("{action}:")).dim(),
        style(message.to_string()).dim()
    );
}

pub fn section(name: &str) {
    println!("{}", style(name).bold());
}

pub fn success(message: impl std::fmt::Display) {
    println!("{} {}", style("✓").green().bold(), message);
}

pub fn hint(message: impl std::fmt::Display) {
    println!("  {} {}", style("hint:").dim(), message);
}
