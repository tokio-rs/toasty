use console::style;
use dialoguer::theme::ColorfulTheme;

/// Returns the standard theme used for interactive prompts
pub fn dialoguer_theme() -> ColorfulTheme {
    ColorfulTheme {
        active_item_style: console::Style::new().cyan().bold(),
        active_item_prefix: style("❯".to_string()).cyan().bold(),
        inactive_item_prefix: style(" ".to_string()),
        checked_item_prefix: style("✔".to_string()).green(),
        unchecked_item_prefix: style("✖".to_string()).red(),
        prompt_style: console::Style::new().bold(),
        prompt_prefix: style("?".to_string()).yellow().bold(),
        success_prefix: style("✔".to_string()).green().bold(),
        error_prefix: style("✖".to_string()).red().bold(),
        hint_style: console::Style::new().dim(),
        values_style: console::Style::new().cyan(),
        ..Default::default()
    }
}
