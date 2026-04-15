use std::collections::HashSet;
use std::path::{Path, PathBuf};

use regex::RegexSet;

use crate::config::AppConfig;
use crate::edn;
use crate::key_collector;
use crate::scanner;

pub struct CheckResult {
    pub unused_keys: Vec<String>,
    pub total_defined: usize,
    pub total_referenced: usize,
}

/// Check for unused translation keys in the project.
///
/// 1. Parse the primary dictionary to get all defined keys
/// 2. Scan source files and collect referenced keys via AST analysis
/// 3. Filter out keys matching `always_used_key_patterns` or `ignore_key_namespaces`
/// 4. Return keys that are defined but not referenced
pub fn check_unused_keys(
    config: &AppConfig,
    base_dir: &Path,
) -> Result<CheckResult, Box<dyn std::error::Error>> {
    let dict_path = base_dir.join(&config.check_keys.primary_dict);
    if !dict_path.exists() {
        return Err(format!("primary dictionary not found: {}", dict_path.display()).into());
    }

    let defined_keys = edn::parse_dict_keys(&dict_path)?;
    let total_defined = defined_keys.len();

    let files = scanner::scan_files(
        &scanner::ScanConfig {
            include_dirs: &config.include_dirs,
            exclude_patterns: &config.check_keys.exclude_patterns,
            file_extensions: &config.file_extensions,
        },
        base_dir,
    )?;
    let referenced_keys = key_collector::collect_referenced_keys(&files, config);

    // Collect keys derived from built-in db-ident definitions
    let db_ident_keys =
        key_collector::collect_db_ident_keys(&config.check_keys.db_ident_defs, base_dir);

    let mut all_referenced = referenced_keys;
    all_referenced.extend(db_ident_keys);
    let total_referenced = all_referenced.len();

    // Build filters for always-used patterns and ignored namespaces
    let always_used = if config.check_keys.always_used_key_patterns.is_empty() {
        None
    } else {
        Some(
            RegexSet::new(&config.check_keys.always_used_key_patterns)
                .map_err(|e| format!("invalid always_used_key_patterns regex: {e}"))?,
        )
    };

    let ignore_ns = &config.check_keys.ignore_key_namespaces;

    let mut unused_keys: Vec<String> = defined_keys
        .difference(&all_referenced)
        .filter(|key| {
            // Skip keys matching always_used patterns
            if let Some(ref patterns) = always_used
                && patterns.is_match(key)
            {
                return false;
            }
            // Skip keys in ignored namespaces
            // Key format: ":namespace/name" — extract namespace part
            if let Some(ns) = extract_key_namespace(key)
                && ignore_ns
                    .iter()
                    .any(|ignored| ns == ignored || ns.starts_with(&format!("{ignored}.")))
            {
                return false;
            }
            true
        })
        .cloned()
        .collect();

    unused_keys.sort();

    Ok(CheckResult {
        unused_keys,
        total_defined,
        total_referenced,
    })
}

/// Remove unused keys from all dictionary files in the dicts directory.
pub fn fix_unused_keys(
    config: &AppConfig,
    base_dir: &Path,
    unused: &[String],
) -> Result<usize, Box<dyn std::error::Error>> {
    let dicts_dir = base_dir.join(&config.check_keys.dicts_dir);
    if !dicts_dir.exists() {
        return Err(format!("dictionary directory not found: {}", dicts_dir.display()).into());
    }

    let keys_to_remove: HashSet<String> = unused.iter().cloned().collect();
    let mut fixed_count = 0;

    let dict_files = find_dict_files(&dicts_dir)?;
    for dict_path in &dict_files {
        if edn::remove_keys_from_dict(dict_path, &keys_to_remove)? {
            fixed_count += 1;
        }
    }

    Ok(fixed_count)
}

/// Find all .edn files in the dictionary directory.
fn find_dict_files(dir: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("edn") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// Extract the namespace part from a keyword string like ":ns/name".
fn extract_key_namespace(key: &str) -> Option<&str> {
    let key = key.strip_prefix(':')?;
    key.split('/').next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_namespace() {
        assert_eq!(extract_key_namespace(":ui/save"), Some("ui"));
        assert_eq!(
            extract_key_namespace(":command.editor/copy"),
            Some("command.editor")
        );
        assert_eq!(extract_key_namespace(":simple"), Some("simple"));
        assert_eq!(extract_key_namespace(""), None);
    }

    #[test]
    fn ignore_namespace_matching() {
        let ignore_ns = ["config.deprecated".to_string()];

        let key = ":config.deprecated/old-key";
        let ns = extract_key_namespace(key).unwrap();
        let matched = ignore_ns
            .iter()
            .any(|ignored| ns == ignored || ns.starts_with(&format!("{ignored}.")));
        assert!(matched);

        let key2 = ":config.active/new-key";
        let ns2 = extract_key_namespace(key2).unwrap();
        let matched2 = ignore_ns
            .iter()
            .any(|ignored| ns2 == ignored || ns2.starts_with(&format!("{ignored}.")));
        assert!(!matched2);
    }
}
