use std::path::{Path, PathBuf};

use serde::Deserialize;

const DEFAULT_CONFIG: &str = include_str!("../config/default.toml");

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub include_dirs: Vec<String>,

    #[serde(default)]
    pub exclude_patterns: Vec<String>,

    #[serde(default = "default_file_extensions")]
    pub file_extensions: Vec<String>,

    #[serde(default = "default_text_preview_length")]
    pub text_preview_length: usize,

    #[serde(default)]
    pub allow_strings: Vec<String>,

    #[serde(default)]
    pub allow_patterns: Vec<String>,

    #[serde(default)]
    pub ui_functions: Vec<String>,

    #[serde(default)]
    pub ui_namespaces: Vec<String>,

    #[serde(default)]
    pub ui_attributes: Vec<String>,

    #[serde(default)]
    pub ignore_context_functions: Vec<String>,

    /// Translation functions whose call sites are skipped entirely.
    #[serde(default)]
    pub i18n_functions: Vec<String>,

    /// Exception/error constructor functions whose arguments are skipped.
    #[serde(default)]
    pub exception_functions: Vec<String>,

    /// Alert/notification functions where the first string arg is user-visible text.
    #[serde(default)]
    pub alert_functions: Vec<String>,

    /// Pure (non-UI) functions — string args inside are not reported even in UI context.
    #[serde(default)]
    pub pure_functions: Vec<String>,

    /// Format/printf functions — only the first argument (the template string) is flagged,
    /// and only when the call site is inside a UI context (hiccup vector or UI function call).
    #[serde(default)]
    pub format_functions: Vec<String>,

    #[serde(default)]
    pub project_root: String,
}

fn default_file_extensions() -> Vec<String> {
    vec!["clj".into(), "cljs".into(), "cljc".into()]
}

fn default_text_preview_length() -> usize {
    60
}

impl AppConfig {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let config_path = Path::new(path);
        if config_path.exists() {
            let user_content = std::fs::read_to_string(config_path)?;
            let config: AppConfig = toml::from_str(&user_content)?;
            Ok(config)
        } else {
            let default: AppConfig = toml::from_str(DEFAULT_CONFIG)?;
            Ok(default)
        }
    }

    pub fn resolve_include_dirs(&self, base: &Path) -> Vec<PathBuf> {
        self.include_dirs
            .iter()
            .map(|d| base.join(d))
            .filter(|p| p.exists())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_default_config() {
        let config = AppConfig::load("nonexistent.toml").unwrap();
        assert!(!config.include_dirs.is_empty());
        assert!(!config.file_extensions.is_empty());
        assert!(config.text_preview_length > 0);
    }

    #[test]
    fn default_config_parses() {
        let config: AppConfig = toml::from_str(DEFAULT_CONFIG).unwrap();
        assert!(!config.ui_attributes.is_empty());
        assert!(!config.ignore_context_functions.is_empty());
        assert!(!config.include_dirs.is_empty());
    }
}
