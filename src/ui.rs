use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

const SPINNER_TICK_CHARS: &str = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏";

pub fn spinner(message: &str) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap()
            .tick_chars(SPINNER_TICK_CHARS),
    );
    spinner.enable_steady_tick(Duration::from_millis(80));
    spinner.set_message(message.to_string());
    spinner
}

pub fn status(action: &str, message: impl std::fmt::Display) {
    println!(
        "{} {}",
        style(format!("{action:>8}")).green().bold(),
        message
    );
}

pub fn status_dim(action: &str, message: impl std::fmt::Display) {
    println!(
        "{} {}",
        style(format!("{action:>8}")).dim(),
        style(message.to_string()).dim()
    );
}

pub fn section(name: &str) {
    println!("{}", style(name).bold());
}

pub fn success(message: impl std::fmt::Display) {
    println!("{} {}", style("✓").green().bold(), message);
}
