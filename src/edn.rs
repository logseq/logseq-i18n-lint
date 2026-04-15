use std::collections::HashSet;
use std::hash::BuildHasher;
use std::path::Path;

use crate::parser::{self, SExp};

/// Parse an EDN dictionary file and return the set of keyword keys in the top-level map.
///
/// The expected format is `{ :ns/key "value" :ns/key2 "value2" ... }`.
/// Only top-level keyword keys are extracted; values are ignored.
pub fn parse_dict_keys(path: &Path) -> Result<HashSet<String>, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let forms = parser::parse_with_hint(&content, &path.to_string_lossy())?;

    let mut keys = HashSet::new();

    for form in &forms {
        if let SExp::Map(entries, _) = form {
            // Map entries alternate: key, value, key, value, ...
            for (i, entry) in entries.iter().enumerate() {
                if i % 2 == 0
                    && let SExp::Keyword(k, _) = entry
                {
                    keys.insert(format!(":{k}"));
                }
            }
        }
    }

    Ok(keys)
}

/// Remove the given keys from an EDN dictionary file.
///
/// Parses the file with the EDN parser to locate the exact start line of each
/// top-level map entry, then removes the lines belonging to each targeted
/// key–value pair.  This correctly handles multi-line values such as nested
/// maps or vectors.
///
/// Returns `Ok(true)` if the file was modified, `Ok(false)` if none of the
/// keys to remove were present in this file.  Returns an error if the file
/// uses a compact (single-line) format, which requires text-surgery rather
/// than line removal and is not supported by this function.
pub fn remove_keys_from_dict<S: BuildHasher>(
    path: &Path,
    keys_to_remove: &HashSet<String, S>,
) -> Result<bool, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let forms = parser::parse_with_hint(&content, &path.to_string_lossy())?;

    // Locate the top-level map's entry list.
    let entries = forms.iter().find_map(|f| {
        if let SExp::Map(e, _) = f {
            Some(e.as_slice())
        } else {
            None
        }
    });
    let entries = match entries {
        Some(e) if !e.is_empty() => e,
        _ => return Ok(false),
    };

    // Build an ordered list of (0-based start line, should_remove) per keyword.
    let mut key_lines: Vec<(usize, bool)> = Vec::new();
    for (i, entry) in entries.iter().enumerate() {
        if i % 2 == 0
            && let SExp::Keyword(k, span) = entry
        {
            let line_0 = span.line.saturating_sub(1) as usize;
            key_lines.push((line_0, keys_to_remove.contains(&format!(":{k}"))));
        }
    }

    if key_lines.iter().all(|(_, remove)| !remove) {
        return Ok(false);
    }

    let lines: Vec<&str> = content.lines().collect();

    // The closing `}` line — last line that is exactly `}`.
    let closing_line = lines
        .iter()
        .enumerate()
        .rev()
        .find(|(_, l)| l.trim() == "}")
        .map_or(lines.len().saturating_sub(1), |(i, _)| i);

    // Mark lines to remove: from the start of each removed entry up to (but not
    // including) the start of the next entry, or the closing brace.
    let mut lines_to_remove: HashSet<usize> = HashSet::new();
    for (idx, &(start_line, should_remove)) in key_lines.iter().enumerate() {
        if !should_remove {
            continue;
        }
        let end_exclusive = if idx + 1 < key_lines.len() {
            key_lines[idx + 1].0
        } else {
            closing_line
        };
        if start_line >= end_exclusive {
            // Compact or unsupported format: all entries share the same line, or
            // the last entry is on the same line as the closing `}`.  Line-based
            // removal cannot work here without risking data corruption.
            return Err(format!(
                "cannot remove key from '{}': compact or single-line EDN format is \
                 not supported for --fix.  Reformat the file to use one key–value \
                 pair per line.",
                path.display()
            )
            .into());
        }
        for line_idx in start_line..end_exclusive {
            lines_to_remove.insert(line_idx);
        }
    }

    let mut result = String::with_capacity(content.len());
    for (i, line) in lines.iter().enumerate() {
        if !lines_to_remove.contains(&i) {
            result.push_str(line);
            result.push('\n');
        }
    }

    if !content.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }

    std::fs::write(path, result)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp_edn(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn parse_simple_dict() {
        let f = write_temp_edn(
            r#"{
 :command/copy "Copy"
 :command/paste "Paste"
 :ui/save "Save"
}"#,
        );
        let keys = parse_dict_keys(f.path()).unwrap();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(":command/copy"));
        assert!(keys.contains(":command/paste"));
        assert!(keys.contains(":ui/save"));
    }

    #[test]
    fn parse_dict_with_comments() {
        let f = write_temp_edn(
            r#"{
 ;; UI commands
 :ui/ok "OK"
 ;; Navigation
 :nav/home "Home"
}"#,
        );
        let keys = parse_dict_keys(f.path()).unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(":ui/ok"));
        assert!(keys.contains(":nav/home"));
    }

    #[test]
    fn parse_empty_dict() {
        let f = write_temp_edn("{}");
        let keys = parse_dict_keys(f.path()).unwrap();
        assert!(keys.is_empty());
    }

    #[test]
    fn remove_keys_basic() {
        let f = write_temp_edn(
            r#"{
 :keep/one "Keep 1"
 :remove/this "Remove this"
 :keep/two "Keep 2"
}"#,
        );
        let mut to_remove = HashSet::new();
        to_remove.insert(":remove/this".to_string());
        remove_keys_from_dict(f.path(), &to_remove).unwrap();

        let result = std::fs::read_to_string(f.path()).unwrap();
        assert!(result.contains(":keep/one"));
        assert!(!result.contains(":remove/this"));
        assert!(result.contains(":keep/two"));
    }

    #[test]
    fn remove_keys_preserves_structure() {
        let f = write_temp_edn(
            r#"{
 :a/first "First"
 :a/second "Second"
 :a/third "Third"
}"#,
        );
        let mut to_remove = HashSet::new();
        to_remove.insert(":a/second".to_string());
        remove_keys_from_dict(f.path(), &to_remove).unwrap();

        let result = std::fs::read_to_string(f.path()).unwrap();
        assert!(result.contains(":a/first"));
        assert!(!result.contains(":a/second"));
        assert!(result.contains(":a/third"));
    }

    #[test]
    fn remove_keys_with_multiline_value() {
        let f = write_temp_edn(
            r#"{
 :keep/one "Keep 1"
 :remove/nested {:inner "val"
                 :other "val2"}
 :keep/two "Keep 2"
}"#,
        );
        let mut to_remove = HashSet::new();
        to_remove.insert(":remove/nested".to_string());
        remove_keys_from_dict(f.path(), &to_remove).unwrap();

        let result = std::fs::read_to_string(f.path()).unwrap();
        assert!(result.contains(":keep/one"));
        assert!(!result.contains(":remove/nested"));
        assert!(!result.contains(":inner"));
        assert!(result.contains(":keep/two"));
    }
}
