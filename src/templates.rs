use std::path::Path;

use chrono::Local;
use minijinja::{Environment, context};

pub fn render(template: &str, title: &str) -> String {
    let now = Local::now();
    let env = Environment::new();
    env.render_str(
        template,
        context! {
            title => title,
            date => now.format("%Y-%m-%d").to_string(),
            time => now.format("%H:%M").to_string(),
        },
    )
    .unwrap_or_else(|_| template.to_string())
}

pub fn load(vault_root: &Path, name: &str) -> Option<String> {
    let path = vault_root.join("templates").join(format!("{name}.md"));
    std::fs::read_to_string(path).ok()
}

pub fn daily_filename() -> String {
    format!("{}.md", Local::now().format("%Y-%m-%d"))
}

pub fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_placeholders() {
        let out = render("# {{title}} on {{date}}", "Hello");
        assert!(out.contains("Hello"));
        assert!(out.contains("on "));
        assert!(!out.contains("{{"));
    }
}
