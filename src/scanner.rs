use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use walkdir::WalkDir;

use crate::config::AppConfig;

pub fn scan_files(config: &AppConfig, base_dir: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let dirs = config.resolve_include_dirs(base_dir);

    if dirs.is_empty() {
        return Ok(Vec::new());
    }

    let exclude_set = build_glob_set(&config.exclude_patterns)?;

    let mut files = Vec::new();
    for dir in &dirs {
        for entry in WalkDir::new(dir).follow_links(true).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();

            if !has_target_extension(path, &config.file_extensions) {
                continue;
            }

            let rel = path.strip_prefix(base_dir).unwrap_or(path);
            let rel_str = rel.to_string_lossy();

            if exclude_set.is_match(rel_str.as_ref()) {
                continue;
            }

            files.push(path.to_path_buf());
        }
    }

    files.sort();
    files.dedup();
    Ok(files)
}

fn has_target_extension(path: &std::path::Path, extensions: &[String]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| extensions.iter().any(|e| e == ext))
}

fn build_glob_set(patterns: &[String]) -> Result<GlobSet, globset::Error> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern)?);
    }
    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_check() {
        let exts = vec!["cljs".into(), "clj".into()];
        assert!(has_target_extension(std::path::Path::new("foo.cljs"), &exts));
        assert!(has_target_extension(std::path::Path::new("bar.clj"), &exts));
        assert!(!has_target_extension(std::path::Path::new("baz.rs"), &exts));
        assert!(!has_target_extension(std::path::Path::new("no_ext"), &exts));
    }

    #[test]
    fn glob_set_matches() {
        let patterns = vec!["**/test/**".into(), "**/node_modules/**".into()];
        let set = build_glob_set(&patterns).unwrap();
        assert!(set.is_match("src/test/foo.cljs"));
        assert!(set.is_match("node_modules/pkg/bar.clj"));
        assert!(!set.is_match("src/main/foo.cljs"));
    }
}
