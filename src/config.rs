use std::path::Path;

use serde::Deserialize;

const DEFAULT_CONFIG: &str = include_str!("../config/default.toml");

/// Settings shared by both subcommands.
#[derive(Debug, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub project_root: String,

    #[serde(default)]
    pub include_dirs: Vec<String>,

    #[serde(default = "default_file_extensions")]
    pub file_extensions: Vec<String>,

    /// Translation functions — calls to these provide translation keys for both subcommands.
    #[serde(default)]
    pub i18n_functions: Vec<String>,

    /// Alert/notification functions — the first argument is user-visible text (lint).
    /// It is analyzed in UI context, so `str`, conditional, and format calls inside it
    /// are also detected.  The first keyword argument is a translation key reference (check-keys).
    #[serde(default)]
    pub alert_functions: Vec<String>,

    /// UI component functions — string args are user-visible text (lint),
    /// keyword args are translation key references (check-keys).
    #[serde(default)]
    pub ui_functions: Vec<String>,

    /// Namespace prefixes where every function is treated as a UI function.
    #[serde(default)]
    pub ui_namespaces: Vec<String>,

    /// Hiccup/map attributes whose string values are user-visible text (lint)
    /// and whose keyword values are translation key references (check-keys).
    #[serde(default)]
    pub ui_attributes: Vec<String>,

    #[serde(default)]
    pub lint: LintConfig,

    #[serde(rename = "check-keys", default)]
    pub check_keys: CheckKeysConfig,
}

/// Settings specific to the `lint` subcommand.
#[derive(Debug, Deserialize)]
pub struct LintConfig {
    #[serde(default)]
    pub exclude_patterns: Vec<String>,

    #[serde(default = "default_text_preview_length")]
    pub text_preview_length: usize,

    #[serde(default)]
    pub allow_strings: Vec<String>,

    #[serde(default)]
    pub allow_patterns: Vec<String>,

    /// Exception/error constructor functions — arguments are developer-facing, not UI text.
    #[serde(default)]
    pub exception_functions: Vec<String>,

    /// Functions whose arguments are not checked (logging, introspection, dev tools).
    #[serde(default)]
    pub ignore_context_functions: Vec<String>,

    /// Pure (non-UI) functions — string args inside are not reported even in a UI context.
    #[serde(default)]
    pub pure_functions: Vec<String>,

    /// Format/printf functions — only the first argument is flagged, only in UI context.
    #[serde(default)]
    pub format_functions: Vec<String>,
}

impl Default for LintConfig {
    fn default() -> Self {
        Self {
            exclude_patterns: vec![],
            text_preview_length: 60,
            allow_strings: vec![],
            allow_patterns: vec![],
            exception_functions: vec![],
            ignore_context_functions: vec![],
            pure_functions: vec![],
            format_functions: vec![],
        }
    }
}

/// Settings specific to the `check-keys` subcommand.
#[derive(Debug, Deserialize, Default)]
pub struct CheckKeysConfig {
    #[serde(default)]
    pub exclude_patterns: Vec<String>,

    /// Directory containing all dictionary EDN files (relative to `project_root`).
    #[serde(default)]
    pub dicts_dir: String,

    /// Primary dictionary file whose keys define the full translation key set (relative to `project_root`).
    #[serde(default)]
    pub primary_dict: String,

    /// Regex patterns matching keys that are always considered used (e.g. dynamically generated keys).
    #[serde(default)]
    pub always_used_key_patterns: Vec<String>,

    /// Namespace prefixes whose keys are excluded from unused-key checking entirely.
    #[serde(default)]
    pub ignore_key_namespaces: Vec<String>,

    /// Built-in db-ident definition sources. Each entry scopes extraction to a specific named
    /// `def` or `defonce` form within a file.
    #[serde(default)]
    pub db_ident_defs: Vec<DbIdentDef>,

    /// Map attribute keys whose keyword values are translation key references.
    /// Combined with `ui_attributes` during check-keys analysis.
    #[serde(default)]
    pub translation_key_attributes: Vec<String>,
}

/// A scoped reference to a built-in db-ident definition.
#[derive(Debug, Deserialize)]
pub struct DbIdentDef {
    /// Path to the file containing the definition (relative to `project_root`).
    pub file: String,
    /// Name of the `def` or `defonce` form to extract keywords from.
    pub def: String,
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

    /// Validate that fields required by the `lint` subcommand are configured.
    ///
    /// Returns an error string if a critical field is absent or empty.  The check
    /// is intentionally limited to fields whose absence would produce silently wrong
    /// output (e.g. zero files scanned) rather than a diagnostic message.
    /// `project_root` is not validated here; resolution and error reporting happen
    /// in `resolve_base_dir`.
    pub fn validate_for_lint(&self) -> Result<(), String> {
        if self.include_dirs.is_empty() {
            return Err(
                "include_dirs is empty — no source files will be scanned.\n\
                 Add at least one directory to include_dirs in your config file."
                    .into(),
            );
        }
        Ok(())
    }

    /// Validate that fields required by the `check-keys` subcommand are configured.
    ///
    /// Returns an error string if a critical field is absent or empty.  All groups
    /// are checked because missing any one silently produces empty or misleading results:
    /// - `include_dirs`: no source files → every key appears unused
    /// - `dicts_dir` / `primary_dict`: no keys loaded → nothing to check
    /// - key-reference mechanisms: no references found → every key appears unused.
    ///   A configuration is valid if at least one of `i18n_functions`, `ui_functions`,
    ///   `ui_namespaces`, `alert_functions`, or `translation_key_attributes` is non-empty.
    pub fn validate_for_check_keys(&self) -> Result<(), String> {
        if self.include_dirs.is_empty() {
            return Err(
                "include_dirs is empty — no source files will be scanned.\n\
                 Add at least one directory to include_dirs in your config file."
                    .into(),
            );
        }
        if self.check_keys.dicts_dir.is_empty() {
            return Err(
                "[check-keys] dicts_dir is not set.\n\
                 Specify the directory that contains your EDN dictionary files."
                    .into(),
            );
        }
        if self.check_keys.primary_dict.is_empty() {
            return Err(
                "[check-keys] primary_dict is not set.\n\
                 Specify the primary dictionary file whose keys define the key set."
                    .into(),
            );
        }
        if self.i18n_functions.is_empty()
            && self.ui_functions.is_empty()
            && self.ui_namespaces.is_empty()
            && self.alert_functions.is_empty()
            && self.check_keys.translation_key_attributes.is_empty()
        {
            return Err(
                "no key-reference mechanisms are configured: i18n_functions, \
                 ui_functions, ui_namespaces, alert_functions, and \
                 translation_key_attributes are all empty.\n\
                 All translation keys would appear unused."
                    .into(),
            );
        }
        Ok(())
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
        assert!(config.lint.text_preview_length > 0);
    }

    #[test]
    fn default_config_parses() {
        let config: AppConfig = toml::from_str(DEFAULT_CONFIG).unwrap();
        assert!(!config.ui_attributes.is_empty());
        assert!(!config.lint.ignore_context_functions.is_empty());
        assert!(!config.include_dirs.is_empty());
    }

    #[test]
    fn check_keys_defaults() {
        // translation_key_attributes is defined in default.toml, not as a code default.
        // When parsing empty TOML (no default.toml), the field is empty by design.
        let config: AppConfig = toml::from_str("").unwrap();
        assert!(config.check_keys.translation_key_attributes.is_empty());
    }

    #[test]
    fn validate_for_lint_rejects_empty_include_dirs() {
        let config: AppConfig = toml::from_str("").unwrap();
        assert!(config.validate_for_lint().is_err());
    }

    #[test]
    fn validate_for_lint_accepts_valid_config() {
        let config: AppConfig = toml::from_str(DEFAULT_CONFIG).unwrap();
        assert!(config.validate_for_lint().is_ok());
    }

    #[test]
    fn validate_for_check_keys_rejects_missing_fields() {
        let config: AppConfig = toml::from_str("").unwrap();
        assert!(config.validate_for_check_keys().is_err());
    }

    #[test]
    fn validate_for_check_keys_rejects_when_no_key_mechanism() {
        let toml = r#"
include_dirs = ["src"]
[check-keys]
dicts_dir    = "dicts"
primary_dict = "dicts/en.edn"
"#;
        let config: AppConfig = toml::from_str(toml).unwrap();
        let err = config.validate_for_check_keys().unwrap_err();
        assert!(
            err.contains("i18n_functions") && err.contains("translation_key_attributes"),
            "expected key-mechanism error, got: {err}"
        );
    }
}

