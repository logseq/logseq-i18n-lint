use std::path::{Path, PathBuf};
use std::process::Command;

use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::config::AppConfig;

/// Get list of git-changed files that match the same rules as a full scan:
///
/// - file extension is in `config.file_extensions`
/// - file is under at least one directory in `config.include_dirs`
/// - file does not match any glob in `config.lint.exclude_patterns`
///
/// `base_dir` must be the root of the repository to analyse (the resolved
/// `project_root`).  All git commands are executed with that directory as the
/// working directory so that relative paths returned by git are correct.
pub fn changed_files(
    config: &AppConfig,
    base_dir: &Path,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut all_files = Vec::new();

    // Unstaged changes
    if let Ok(files) = git_diff_files(base_dir, &["diff", "--name-only"]) {
        all_files.extend(files);
    }

    // Staged changes
    if let Ok(files) = git_diff_files(base_dir, &["diff", "--name-only", "--cached"]) {
        all_files.extend(files);
    }

    // All uncommitted changes relative to HEAD (staged + unstaged combined)
    if let Ok(files) = git_diff_files(base_dir, &["diff", "--name-only", "HEAD"]) {
        all_files.extend(files);
    }

    // Untracked files (new files not yet staged)
    if let Ok(files) = git_untracked_files(base_dir) {
        all_files.extend(files);
    }

    // Dedup before the heavier per-file checks.
    all_files.sort();
    all_files.dedup();

    let include_dirs: Vec<PathBuf> = config.include_dirs.iter().map(PathBuf::from).collect();
    let exclude_set = build_exclude_set(&config.lint.exclude_patterns)?;

    let result: Vec<PathBuf> = all_files
        .into_iter()
        // Extension filter — same as a full scan.
        .filter(|f| has_target_extension(f, &config.file_extensions))
        // include_dirs filter: file must be under at least one configured directory.
        .filter(|f| {
            let p = Path::new(f.as_str());
            include_dirs.iter().any(|dir| p.starts_with(dir))
        })
        // exclude_patterns filter: file must not match any exclusion glob.
        .filter(|f| !exclude_set.is_match(f.as_str()))
        .map(|f| base_dir.join(&f))
        .filter(|p| p.exists())
        .collect();

    Ok(result)
}

fn git_diff_files(
    base_dir: &Path,
    args: &[&str],
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(base_dir)
        .output()?;

    if !output.status.success() {
        return Err(format!("git {} failed", args.join(" ")).into());
    }

    let stdout = String::from_utf8(output.stdout)?;
    let files: Vec<String> = stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect();

    Ok(files)
}

fn git_untracked_files(base_dir: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard"])
        .current_dir(base_dir)
        .output()?;

    if !output.status.success() {
        return Err("git ls-files --others failed".into());
    }

    let stdout = String::from_utf8(output.stdout)?;
    let files: Vec<String> = stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect();

    Ok(files)
}

fn has_target_extension(file: &str, extensions: &[String]) -> bool {
    extensions
        .iter()
        .any(|ext| file.ends_with(&format!(".{ext}")))
}

fn build_exclude_set(patterns: &[String]) -> Result<GlobSet, globset::Error> {
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
    fn extension_filter() {
        let exts = vec!["cljs".into(), "clj".into()];
        assert!(has_target_extension("foo.cljs", &exts));
        assert!(has_target_extension("path/to/bar.clj", &exts));
        assert!(!has_target_extension("baz.rs", &exts));
        assert!(!has_target_extension("no_ext", &exts));
    }
}
