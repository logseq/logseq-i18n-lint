use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use rayon::prelude::*;

use crate::config::{AppConfig, DbIdentDef};
use crate::parser::{self, SExp};

/// File path and line number of a key's first occurrence in source.
pub struct KeyLocation {
    pub file: PathBuf,
    pub line: u32,
}

/// Collect all referenced translation keys from source files via AST analysis.
///
/// Two-pass analysis per file:
/// - Pass 1: Build a symbol table from top-level `def`/`defonce` bindings
/// - Pass 2: Walk the AST to find translation function calls and map entries
///   with translation key attributes (e.g. `:i18n-key`, `:prompt-key`)
pub fn collect_referenced_keys(files: &[PathBuf], config: &AppConfig) -> HashSet<String> {
    files
        .par_iter()
        .flat_map(|path| {
            let Ok(source) = std::fs::read_to_string(path) else {
                return Vec::new();
            };
            let Ok(forms) = parser::parse(&source) else {
                return Vec::new();
            };

            let mut ctx = CollectorContext::new(config);

            // Pass 1: build symbol table from top-level defs
            for form in &forms {
                ctx.collect_def_bindings(form);
            }

            // Pass 2: collect referenced keys
            for form in &forms {
                ctx.collect_keys(form);
            }

            ctx.keys.into_iter().collect::<Vec<_>>()
        })
        .collect()
}

/// Collect referenced keys from explicit `i18n_functions` calls and
/// `translation_key_attributes` map entries, with the first source location per key.
///
/// `alert_functions`, `ui_functions`/`ui_namespaces`, and `ui_attributes` map scanning
/// are excluded to prevent false positives in `check-missing`.
pub fn collect_referenced_keys_strict(
    files: &[PathBuf],
    config: &AppConfig,
) -> HashMap<String, KeyLocation> {
    // Collect (key, file, line) from each file in parallel
    let mut all_occurrences: Vec<(String, PathBuf, u32)> = files
        .par_iter()
        .flat_map(|path| {
            let Ok(source) = std::fs::read_to_string(path) else {
                return Vec::new();
            };
            let Ok(forms) = parser::parse(&source) else {
                return Vec::new();
            };

            let mut ctx = CollectorContext::new_strict(config);

            // Pass 1: build symbol table from top-level defs
            for form in &forms {
                ctx.collect_def_bindings(form);
            }

            // Pass 2: collect referenced keys
            for form in &forms {
                ctx.collect_keys(form);
            }

            ctx.key_locations
                .into_iter()
                .map(|(key, line)| (key, path.clone(), line))
                .collect::<Vec<_>>()
        })
        .collect();

    // Sort by (file, line) for deterministic first-occurrence selection
    all_occurrences.sort_by(|a, b| a.1.cmp(&b.1).then(a.2.cmp(&b.2)));

    let mut locations: HashMap<String, KeyLocation> = HashMap::new();
    for (key, file, line) in all_occurrences {
        locations.entry(key).or_insert(KeyLocation { file, line });
    }
    locations
}

/// Collect i18n keys derived from built-in db-ident definitions.
///
/// Each `DbIdentDef` entry scopes the keyword extraction to the value of a specific
/// named `def` or `defonce` form within a file, avoiding false positives from other
/// keyword literals in the same file.
pub fn collect_db_ident_keys(defs: &[DbIdentDef], base_dir: &Path) -> HashSet<String> {
    defs.iter()
        .flat_map(|def_entry| {
            let path = base_dir.join(&def_entry.file);
            let Ok(source) = std::fs::read_to_string(&path) else {
                return Vec::new();
            };
            let Ok(forms) = parser::parse(&source) else {
                return Vec::new();
            };

            let mut keywords = Vec::new();
            if let Some(value) = find_named_def_value(&forms, &def_entry.def) {
                extract_all_keywords(value, &mut keywords);
            } else {
                // Fallback: scan the entire file
                for form in &forms {
                    extract_all_keywords(form, &mut keywords);
                }
            }

            keywords
                .iter()
                .filter_map(|kw| db_ident_to_i18n_key(kw))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Find the value expression of a named `def` or `defonce` top-level form.
fn find_named_def_value<'a>(forms: &'a [SExp], def_name: &str) -> Option<&'a SExp> {
    for form in forms {
        if let SExp::List(items, _) = form
            && items.len() >= 3
            && let SExp::Symbol(head, _) = &items[0]
            && (head == "def" || head == "defonce")
        {
            // Find the name symbol (skip metadata if present)
            let name_idx = items
                .iter()
                .enumerate()
                .skip(1)
                .find(|(_, item)| matches!(item, SExp::Symbol(_, _)))
                .map(|(i, _)| i);
            if let Some(idx) = name_idx
                && let SExp::Symbol(name, _) = &items[idx]
                && name == def_name
            {
                return items.get(idx + 1);
            }
        }
    }
    None
}

/// Convert a built-in db-ident keyword to its corresponding i18n translation key.
///
/// Conversion rules:
/// - `:logseq.class/Name`               → `:class.built-in/name` (lowercased)
/// - `:logseq.property/name`            → `:property.built-in/name`
/// - `:logseq.property/name?`           → `:property.built-in/name` (trailing ? removed)
/// - `:logseq.property/status.doing`    → `:property.status/doing` (dot splits)
/// - `:logseq.property.sub/name`        → `:property.built-in/sub-name`
/// - `:logseq.property.sub/name.choice` → `:property.sub-name/choice`
/// - `:block/name`                      → `:property.built-in/name`
/// - `:block/name?`                     → `:property.built-in/name`
fn db_ident_to_i18n_key(keyword: &str) -> Option<String> {
    // keyword is like ":ns/name" — strip leading ':'
    let kw = keyword.strip_prefix(':')?;
    let (ns_str, name) = kw.split_once('/')?;

    if ns_str == "logseq.class" {
        let lower = name.to_lowercase();
        return Some(format!(":class.built-in/{lower}"));
    }

    if ns_str == "logseq.property" || ns_str.starts_with("logseq.property.") {
        let sub_ns = if ns_str == "logseq.property" {
            None
        } else {
            Some(&ns_str["logseq.property.".len()..])
        };

        let clean_name = name.strip_suffix('?').unwrap_or(name);

        if let Some(dot_idx) = clean_name.find('.') {
            let prop_part = &clean_name[..dot_idx];
            let choice_part = &clean_name[dot_idx + 1..];
            let subdomain = match sub_ns {
                Some(sub) => format!("{sub}-{prop_part}"),
                None => prop_part.to_string(),
            };
            return Some(format!(":property.{subdomain}/{choice_part}"));
        }

        return match sub_ns {
            Some(sub) => Some(format!(":property.built-in/{sub}-{clean_name}")),
            None => Some(format!(":property.built-in/{clean_name}")),
        };
    }

    if ns_str == "block" {
        let clean_name = name.strip_suffix('?').unwrap_or(name);
        return Some(format!(":property.built-in/{clean_name}"));
    }

    None
}

/// Recursively extract all keyword strings from an AST.
fn extract_all_keywords(expr: &SExp, result: &mut Vec<String>) {
    match expr {
        SExp::Keyword(k, _) => {
            result.push(format!(":{k}"));
        }
        SExp::List(items, _)
        | SExp::Vector(items, _)
        | SExp::Set(items, _)
        | SExp::Map(items, _)
        | SExp::AnonFn(items, _)
        | SExp::ReaderConditional(items, _)
        | SExp::ReaderConditionalSplicing(items, _) => {
            for item in items {
                extract_all_keywords(item, result);
            }
        }
        SExp::Quote(inner, _)
        | SExp::SyntaxQuote(inner, _)
        | SExp::Unquote(inner, _)
        | SExp::UnquoteSplicing(inner, _)
        | SExp::Deref(inner, _)
        | SExp::VarQuote(inner, _)
        | SExp::Meta(_, inner, _)
        | SExp::TaggedLiteral(_, inner, _)
        | SExp::Discard(inner, _) => {
            extract_all_keywords(inner, result);
        }
        _ => {}
    }
}

struct CollectorContext {
    i18n_functions: Vec<String>,
    alert_functions: Vec<String>,
    ui_functions: Vec<String>,
    ui_namespaces: Vec<String>,
    /// Translation key attribute names used for map-entry scanning.
    /// In strict mode: only `translation_key_attributes` (excludes `ui_attributes`).
    /// In normal mode: `translation_key_attributes` ∪ `ui_attributes`.
    all_key_attributes: Vec<String>,
    keys: HashSet<String>,
    /// key → first line number seen in the current file (1-based).
    key_locations: HashMap<String, u32>,
    /// Symbol name → keywords from its top-level `def`/`defonce` value.
    symbol_table: HashMap<String, Vec<String>>,
    /// Stack of let-binding scopes: each frame maps symbol name → keywords from binding value.
    let_scope_stack: Vec<HashMap<String, Vec<String>>>,
    /// When true, only collect from i18n functions and `translation_key_attributes` map entries.
    /// `alert_functions`, `ui_functions`/`ui_namespaces`, and `ui_attributes` are excluded.
    skip_ambient_collection: bool,
}

/// Find the value of a `(def name value)` or `(def ^:meta name value)` form.
fn find_def_value(items: &[SExp]) -> Option<&SExp> {
    for (i, item) in items.iter().enumerate().skip(1) {
        if matches!(item, SExp::Symbol(_, _)) && i + 1 < items.len() {
            return Some(&items[i + 1]);
        }
    }
    None
}

/// Extract all keyword strings from an expression (recursively).
fn extract_keywords_from_expr(expr: &SExp) -> Vec<String> {
    let mut result = Vec::new();
    walk_for_keywords(expr, &mut result);
    result
}

fn walk_for_keywords(expr: &SExp, result: &mut Vec<String>) {
    match expr {
        SExp::Keyword(k, _) => {
            result.push(format!(":{k}"));
        }
        SExp::List(items, _) => {
            // For conditional forms, only walk value/branch positions to avoid
            // collecting keywords that appear as conditions or test-case values.
            if let Some(SExp::Symbol(head, _)) = items.first() {
                match head.as_str() {
                    // Skip condition (index 1), walk then/else/body branches (index 2+)
                    "if" | "if-not" | "when" | "when-not" => {
                        for item in items.iter().skip(2) {
                            walk_for_keywords(item, result);
                        }
                        return;
                    }
                    // Walk only value positions (indices 2, 4, 6, ...)
                    "cond" => {
                        for (i, item) in items.iter().enumerate() {
                            if i >= 1 && i % 2 == 0 {
                                walk_for_keywords(item, result);
                            }
                        }
                        return;
                    }
                    // Walk only result positions (indices 3, 5, 7, ...) plus default
                    "case" => {
                        for (i, item) in items.iter().enumerate() {
                            if i >= 3 && i % 2 == 1 {
                                walk_for_keywords(item, result);
                            }
                        }
                        let after_head_expr = items.len().saturating_sub(2);
                        if after_head_expr % 2 == 1
                            && let Some(last) = items.last()
                        {
                            walk_for_keywords(last, result);
                        }
                        return;
                    }
                    _ => {}
                }
            }
            for item in items {
                walk_for_keywords(item, result);
            }
        }
        SExp::Vector(items, _)
        | SExp::Set(items, _)
        | SExp::Map(items, _)
        | SExp::AnonFn(items, _)
        | SExp::ReaderConditional(items, _)
        | SExp::ReaderConditionalSplicing(items, _) => {
            for item in items {
                walk_for_keywords(item, result);
            }
        }
        SExp::Quote(inner, _)
        | SExp::SyntaxQuote(inner, _)
        | SExp::Unquote(inner, _)
        | SExp::UnquoteSplicing(inner, _)
        | SExp::Deref(inner, _)
        | SExp::VarQuote(inner, _)
        | SExp::Meta(_, inner, _)
        | SExp::TaggedLiteral(_, inner, _) => {
            walk_for_keywords(inner, result);
        }
        _ => {}
    }
}

impl CollectorContext {
    fn new(config: &AppConfig) -> Self {
        Self::new_inner(config, false)
    }

    fn new_strict(config: &AppConfig) -> Self {
        Self::new_inner(config, true)
    }

    fn new_inner(config: &AppConfig, skip_ambient_collection: bool) -> Self {
        let all_key_attributes = if skip_ambient_collection {
            // Strict mode: only explicit translation key attributes, not generic UI attributes.
            config.check_keys.translation_key_attributes.clone()
        } else {
            let mut attrs = config.check_keys.translation_key_attributes.clone();
            for attr in &config.ui_attributes {
                if !attrs.contains(attr) {
                    attrs.push(attr.clone());
                }
            }
            attrs
        };
        Self {
            i18n_functions: config.i18n_functions.clone(),
            alert_functions: config.alert_functions.clone(),
            ui_functions: config.ui_functions.clone(),
            ui_namespaces: config.ui_namespaces.clone(),
            all_key_attributes,
            keys: HashSet::new(),
            key_locations: HashMap::new(),
            symbol_table: HashMap::new(),
            let_scope_stack: Vec::new(),
            skip_ambient_collection,
        }
    }

    /// Insert a key and record its line number if this is the first occurrence in the file.
    fn insert_key(&mut self, key: String, line: u32) {
        self.key_locations.entry(key.clone()).or_insert(line);
        self.keys.insert(key);
    }

    /// Check if a function name is an i18n translation function.
    fn is_i18n_fn(&self, name: &str) -> bool {
        self.i18n_functions
            .iter()
            .any(|f| f == name || name.ends_with(&format!("/{f}")))
    }

    fn is_alert_fn(&self, name: &str) -> bool {
        self.alert_functions
            .iter()
            .any(|f| f == name || name.ends_with(&format!("/{f}")))
    }

    fn is_ui_fn(&self, name: &str) -> bool {
        self.ui_functions
            .iter()
            .any(|f| f == name || name.ends_with(&format!("/{f}")))
            || self
                .ui_namespaces
                .iter()
                .any(|ns| name.starts_with(&format!("{ns}/")))
    }

    fn is_translation_key_attr(&self, attr: &str) -> bool {
        self.all_key_attributes.iter().any(|a| a == attr)
    }

    /// Look up a symbol's associated keywords, checking let scopes (innermost first) then the symbol table.
    fn lookup_symbol(&self, sym: &str) -> Option<Vec<String>> {
        for frame in self.let_scope_stack.iter().rev() {
            if let Some(keywords) = frame.get(sym) {
                return Some(keywords.clone());
            }
        }
        self.symbol_table.get(sym).cloned()
    }

    // ── Pass 1: symbol table ──

    /// Collect top-level `(def name value)` and `(defonce name value)` bindings.
    fn collect_def_bindings(&mut self, form: &SExp) {
        if let SExp::List(items, _) = form
            && items.len() >= 3
            && let SExp::Symbol(head, _) = &items[0]
            && (head == "def" || head == "defonce")
            && let SExp::Symbol(name, _) = &items[1]
            && let Some(val) = find_def_value(items)
        {
            let keywords = extract_keywords_from_expr(val);
            if !keywords.is_empty() {
                self.symbol_table.insert(name.clone(), keywords);
            }
        }
    }

    // ── Pass 2: key collection ──

    fn collect_keys(&mut self, form: &SExp) {
        match form {
            SExp::List(items, _) => {
                // Route let-like forms through let-scope tracking
                if let Some(SExp::Symbol(head, _)) = items.first()
                    && matches!(head.as_str(), "let" | "when-let" | "if-let" | "loop")
                {
                    self.collect_keys_from_let(items);
                    return;
                }
                self.collect_keys_from_list(items);
                // Continue recursing into all children
                for item in items {
                    self.collect_keys(item);
                }
            }
            SExp::Vector(items, _)
            | SExp::Set(items, _)
            | SExp::AnonFn(items, _)
            | SExp::ReaderConditional(items, _)
            | SExp::ReaderConditionalSplicing(items, _) => {
                for item in items {
                    self.collect_keys(item);
                }
            }
            SExp::Map(items, _) => {
                self.collect_keys_from_map(items);
                for item in items {
                    self.collect_keys(item);
                }
            }
            SExp::Quote(inner, _)
            | SExp::SyntaxQuote(inner, _)
            | SExp::Unquote(inner, _)
            | SExp::UnquoteSplicing(inner, _)
            | SExp::Deref(inner, _)
            | SExp::VarQuote(inner, _)
            | SExp::Meta(_, inner, _)
            | SExp::Discard(inner, _)
            | SExp::TaggedLiteral(_, inner, _) => {
                self.collect_keys(inner);
            }
            _ => {}
        }
    }

    /// Handle `let`/`when-let`/`if-let`/`loop` forms with scope tracking.
    ///
    /// Pushes a scope frame populated with keywords extracted from each binding's value
    /// expression, then recurses into body forms within that scope. This allows symbol
    /// references in the body (e.g. `{:i18n-key my-key-var}`) to resolve to the
    /// keywords assigned in the bindings.
    fn collect_keys_from_let(&mut self, items: &[SExp]) {
        let mut scope: HashMap<String, Vec<String>> = HashMap::new();

        if let Some(SExp::Vector(bindings, _)) = items.get(1) {
            let mut i = 0;
            while i + 1 < bindings.len() {
                // Only handle simple symbol bindings (skip destructuring patterns)
                if let SExp::Symbol(name, _) = &bindings[i] {
                    let keywords = extract_keywords_from_expr(&bindings[i + 1]);
                    if !keywords.is_empty() {
                        scope.insert(name.clone(), keywords);
                    }
                }
                // Also collect i18n keys from inside the binding value expressions
                self.collect_keys(&bindings[i + 1]);
                i += 2;
            }
        }

        self.let_scope_stack.push(scope);

        // Recurse into body forms within this scope
        for body_form in items.iter().skip(2) {
            self.collect_keys(body_form);
        }

        self.let_scope_stack.pop();
    }

    /// Detect i18n function calls and extract keyword arguments.
    fn collect_keys_from_list(&mut self, items: &[SExp]) {
        if items.is_empty() {
            return;
        }

        let head_name = match &items[0] {
            SExp::Symbol(name, _) => name.as_str(),
            _ => return,
        };

        if self.is_i18n_fn(head_name) {
            // (t :keyword) or (t symbol) or (t (if ...)) or (t (or ...))
            if items.len() >= 2 {
                match &items[1] {
                    SExp::Keyword(k, span) => {
                        self.insert_key(format!(":{k}"), span.line);
                    }
                    SExp::Symbol(sym, sym_span) => {
                        if let Some(keywords) = self.lookup_symbol(sym.as_str()) {
                            for kw in keywords {
                                self.insert_key(kw, sym_span.line);
                            }
                        }
                    }
                    SExp::List(inner, _) => {
                        // (t (if cond :key-a :key-b)) or (t (or val :fallback))
                        self.collect_keys_from_conditional(inner);
                    }
                    _ => {}
                }
            }
        } else if !self.skip_ambient_collection && self.is_alert_fn(head_name) {
            // Alert functions: first keyword argument is a translation key.
            for arg in items.iter().skip(1) {
                if let SExp::Keyword(k, span) = arg {
                    self.insert_key(format!(":{k}"), span.line);
                    break;
                }
            }
        } else if !self.skip_ambient_collection && self.is_ui_fn(head_name) {
            // UI component functions: keyword arguments are translation key references.
            for arg in items.iter().skip(1) {
                if let SExp::Keyword(k, span) = arg {
                    self.insert_key(format!(":{k}"), span.line);
                }
            }
        }
    }

    /// Extract translation keys from a single value position inside a conditional form.
    ///
    /// Handles `Keyword` (direct key), `Symbol` (resolved via scope/symbol-table),
    /// and `List` (recursively processed as a nested conditional).
    fn collect_conditional_item(&mut self, item: &SExp) {
        match item {
            SExp::Keyword(k, span) => {
                self.insert_key(format!(":{k}"), span.line);
            }
            SExp::Symbol(sym, sym_span) => {
                if let Some(keywords) = self.lookup_symbol(sym.as_str()) {
                    for kw in keywords {
                        self.insert_key(kw, sym_span.line);
                    }
                }
            }
            SExp::List(inner, _) => {
                self.collect_keys_from_conditional(inner);
            }
            _ => {}
        }
    }

    /// Extract translation keys from conditional forms nested inside a translation call.
    ///
    /// - `if`/`if-not`/`when`/`when-not`: skip condition (index 1), collect branches (index 2+)
    /// - `or`: all arguments are potential return values
    /// - `cond`: collect value positions (indices 2, 4, 6, ...); skip test positions
    /// - `case`: collect result positions (indices 3, 5, 7, ...); skip test-case values
    /// - `keyword`: synthesize key from `(keyword "ns" name)` call
    fn collect_keys_from_conditional(&mut self, items: &[SExp]) {
        if items.is_empty() {
            return;
        }
        if let SExp::Symbol(head, _) = &items[0] {
            match head.as_str() {
                "if" | "if-not" | "when" | "when-not" => {
                    for item in items.iter().skip(2) {
                        self.collect_conditional_item(item);
                    }
                }
                "or" => {
                    for item in items.iter().skip(1) {
                        self.collect_conditional_item(item);
                    }
                }
                "cond" => {
                    // Value positions: indices 2, 4, 6, ... (skip head and test positions)
                    for (i, item) in items.iter().enumerate().skip(1) {
                        if i % 2 == 0 {
                            self.collect_conditional_item(item);
                        }
                    }
                }
                "case" => {
                    // Result positions: indices 3, 5, 7, ... (skip head, expr, test-case values)
                    for (i, item) in items.iter().enumerate().skip(2) {
                        if i % 2 == 1 {
                            self.collect_conditional_item(item);
                        }
                    }
                    // Default value (last item when odd count of items after head+expr)
                    let after_head = items.len().saturating_sub(2);
                    if after_head % 2 == 1
                        && let Some(last) = items.last()
                    {
                        self.collect_conditional_item(last);
                    }
                }
                "keyword" => {
                    self.collect_keys_from_keyword_call(items);
                }
                _ => {}
            }
        }
    }

    /// Handle `(keyword "ns" expr)` calls for dynamic key construction.
    fn collect_keys_from_keyword_call(&mut self, items: &[SExp]) {
        if items.len() >= 3
            && let SExp::Str(ns, _) = &items[1]
        {
            let head_line = if let SExp::Symbol(_, span) = &items[0] {
                span.line
            } else {
                0
            };
            // Try to resolve the name part
            if let SExp::Str(name_part, _) = &items[2] {
                self.insert_key(format!(":{ns}/{name_part}"), head_line);
            }
            // For (keyword "ns" (name enum-val)) we can't resolve without more info,
            // but the always_used_key_patterns config handles this.
        }
    }

    /// Detect `:<translation-key-attr> :keyword` entries in map literals.
    fn collect_keys_from_map(&mut self, items: &[SExp]) {
        let mut i = 0;
        while i + 1 < items.len() {
            if let SExp::Keyword(key, _) = &items[i]
                && self.is_translation_key_attr(key)
            {
                match &items[i + 1] {
                    SExp::Keyword(val, span) => {
                        self.insert_key(format!(":{val}"), span.line);
                    }
                    SExp::Symbol(sym, sym_span) => {
                        if let Some(keywords) = self.lookup_symbol(sym.as_str()) {
                            for kw in keywords {
                                self.insert_key(kw, sym_span.line);
                            }
                        }
                    }
                    SExp::List(inner, _) => {
                        self.collect_keys_from_conditional(inner);
                    }
                    _ => {}
                }
            }
            i += 2;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> AppConfig {
        toml::from_str(
            r#"
i18n_functions = ["t", "tt", "i18n/t"]
[check-keys]
translation_key_attributes = ["i18n-key", "prompt-key", "title-key"]
"#,
        )
        .unwrap()
    }

    fn collect_from_source(source: &str) -> HashSet<String> {
        let config = make_config();
        let forms = parser::parse(source).unwrap();
        let mut ctx = CollectorContext::new(&config);
        for form in &forms {
            ctx.collect_def_bindings(form);
        }
        for form in &forms {
            ctx.collect_keys(form);
        }
        ctx.keys
    }

    #[test]
    fn direct_translation_call() {
        let keys = collect_from_source(r"(t :ui/save)");
        assert!(keys.contains(":ui/save"));
    }

    #[test]
    fn aliased_translation_call() {
        let keys = collect_from_source(r"(i18n/t :nav/home)");
        assert!(keys.contains(":nav/home"));
    }

    #[test]
    fn multiple_calls() {
        let keys = collect_from_source(
            r"
            (t :ui/save)
            (tt :ui/cancel)
            (i18n/t :nav/home)
        ",
        );
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(":ui/save"));
        assert!(keys.contains(":ui/cancel"));
        assert!(keys.contains(":nav/home"));
    }

    #[test]
    fn i18n_key_in_map() {
        let keys = collect_from_source(r"{:i18n-key :dialog/confirm}");
        assert!(keys.contains(":dialog/confirm"));
    }

    #[test]
    fn prompt_key_in_map() {
        let keys = collect_from_source(r"{:prompt-key :dialog/prompt}");
        assert!(keys.contains(":dialog/prompt"));
    }

    #[test]
    fn title_key_in_map() {
        let keys = collect_from_source(r"{:title-key :dialog/title}");
        assert!(keys.contains(":dialog/title"));
    }

    #[test]
    fn conditional_translation() {
        let keys = collect_from_source(r"(t (if loading? :ui/loading :ui/ready))");
        assert!(keys.contains(":ui/loading"));
        assert!(keys.contains(":ui/ready"));
    }

    #[test]
    fn or_fallback_translation() {
        let keys = collect_from_source(r"(t (or custom-key :ui/default))");
        assert!(keys.contains(":ui/default"));
    }

    #[test]
    fn i18n_key_with_if() {
        let keys = collect_from_source(r"{:i18n-key (if dark? :theme/dark :theme/light)}");
        assert!(keys.contains(":theme/dark"));
        assert!(keys.contains(":theme/light"));
    }

    #[test]
    fn symbol_resolution_via_def() {
        let _keys = collect_from_source(
            r"
            (defonce sort-options
              [[:view.table/sort-asc :asc]
               [:view.table/sort-desc :desc]])
            (for [[label _] sort-options]
              (t label))
        ",
        );
        // The symbol table should resolve `label` → keywords from the defonce
        // But since `label` in the for binding shadows the def, the simple
        // symbol-table lookup via (t symbol) works for the defonce name itself
        // In practice, `(t sort-options)` would look up the table, not `(t label)`.
        // The for destructuring is too complex for simple symbol resolution.
        // These keys are covered by static occurrence in the def value.
        // Let's test the simpler case:
        let keys2 = collect_from_source(
            r"
            (def my-key :ui/hello)
            (t my-key)
        ",
        );
        assert!(keys2.contains(":ui/hello"));
    }

    #[test]
    fn def_with_vector_of_keywords() {
        let keys = collect_from_source(
            r"
            (defonce options
              [[:view/option-a :data-a]
               [:view/option-b :data-b]])
            (t options)
        ",
        );
        // When (t options) is called, look up symbol table
        assert!(keys.contains(":view/option-a"));
        assert!(keys.contains(":view/option-b"));
        assert!(keys.contains(":data-a"));
        assert!(keys.contains(":data-b"));
    }

    #[test]
    fn nested_translation_call() {
        let keys = collect_from_source(
            r"
            (defn render []
              [:div (t :ui/title)
                [:span (t :ui/subtitle)]])
        ",
        );
        assert!(keys.contains(":ui/title"));
        assert!(keys.contains(":ui/subtitle"));
    }

    #[test]
    fn no_false_positives_from_non_i18n() {
        let keys = collect_from_source(
            r"
            (defn foo [t] (t :not-a-key))
            (log/info :some/keyword)
            {:class :css/class}
        ",
        );
        // :not-a-key is collected because `t` is in i18n_functions
        // But :some/keyword and :css/class are NOT collected since they're not
        // in i18n contexts
        assert!(!keys.contains(":some/keyword"));
        assert!(!keys.contains(":css/class"));
    }

    #[test]
    fn cond_translation() {
        let keys = collect_from_source(
            r"
            (t (cond
                 (= type :a) :msg/type-a
                 (= type :b) :msg/type-b
                 :else :msg/default))
        ",
        );
        assert!(keys.contains(":msg/type-a"));
        assert!(keys.contains(":msg/type-b"));
        assert!(keys.contains(":msg/default"));
    }

    #[test]
    fn db_ident_class_conversion() {
        assert_eq!(
            db_ident_to_i18n_key(":logseq.class/Task"),
            Some(":class.built-in/task".to_string()),
        );
        assert_eq!(
            db_ident_to_i18n_key(":logseq.class/Pdf-annotation"),
            Some(":class.built-in/pdf-annotation".to_string()),
        );
    }

    #[test]
    fn db_ident_property_conversion() {
        assert_eq!(
            db_ident_to_i18n_key(":logseq.property/status"),
            Some(":property.built-in/status".to_string()),
        );
        assert_eq!(
            db_ident_to_i18n_key(":logseq.property/hide?"),
            Some(":property.built-in/hide".to_string()),
        );
    }

    #[test]
    fn db_ident_property_with_dot() {
        assert_eq!(
            db_ident_to_i18n_key(":logseq.property/status.doing"),
            Some(":property.status/doing".to_string()),
        );
        assert_eq!(
            db_ident_to_i18n_key(":logseq.property/priority.high"),
            Some(":property.priority/high".to_string()),
        );
    }

    #[test]
    fn db_ident_property_sub_namespace() {
        assert_eq!(
            db_ident_to_i18n_key(":logseq.property.asset/type"),
            Some(":property.built-in/asset-type".to_string()),
        );
        assert_eq!(
            db_ident_to_i18n_key(":logseq.property.view/type.gallery"),
            Some(":property.view-type/gallery".to_string()),
        );
        assert_eq!(
            db_ident_to_i18n_key(":logseq.property.repeat/recur-unit.day"),
            Some(":property.repeat-recur-unit/day".to_string()),
        );
    }

    #[test]
    fn db_ident_block_conversion() {
        assert_eq!(
            db_ident_to_i18n_key(":block/alias"),
            Some(":property.built-in/alias".to_string()),
        );
        assert_eq!(
            db_ident_to_i18n_key(":block/collapsed?"),
            Some(":property.built-in/collapsed".to_string()),
        );
    }

    #[test]
    fn db_ident_unrelated_namespace() {
        assert_eq!(db_ident_to_i18n_key(":datascript/key"), None);
        assert_eq!(db_ident_to_i18n_key(":ui/save"), None);
    }

    #[test]
    fn let_bound_symbol_in_map_resolves_keys() {
        // Matches the real Logseq validate.cljs pattern:
        // (let [i18n-key (if condition :key-a :key-b)]
        //   (throw (ex-info "msg" {:payload {:i18n-key i18n-key}})))
        let keys = collect_from_source(
            r#"
            (let [i18n-key (if condition
                             :page.convert/property-value-to-page
                             :page.convert/block-parent-not-page)]
              (throw (ex-info "err" {:payload {:i18n-key i18n-key}})))
        "#,
        );
        assert!(
            keys.contains(":page.convert/property-value-to-page"),
            "should resolve let-bound symbol to :page.convert/property-value-to-page, found: {keys:?}"
        );
        assert!(
            keys.contains(":page.convert/block-parent-not-page"),
            "should resolve let-bound symbol to :page.convert/block-parent-not-page, found: {keys:?}"
        );
    }

    #[test]
    fn when_let_bound_symbol_resolves_keys() {
        let keys = collect_from_source(
            r"
            (when-let [key (if flag :notify/success :notify/failure)]
              {:i18n-key key})
        ",
        );
        assert!(
            keys.contains(":notify/success"),
            "should resolve when-let bound symbol"
        );
        assert!(
            keys.contains(":notify/failure"),
            "should resolve when-let bound symbol"
        );
    }

    // ── Conditional form handling ─────────────────────────────────────────────

    #[test]
    fn when_collects_body_key() {
        let keys = collect_from_source(r"(t (when loading? :ui/loading))");
        assert!(
            keys.contains(":ui/loading"),
            "when body should be collected"
        );
    }

    #[test]
    fn when_not_collects_body_key() {
        let keys = collect_from_source(r"(t (when-not error? :ui/ready))");
        assert!(
            keys.contains(":ui/ready"),
            "when-not body should be collected"
        );
    }

    #[test]
    fn if_branch_symbol_resolved() {
        // Symbol in if branch resolved via let scope
        let keys = collect_from_source(
            r"
            (let [k :msg/hello]
              (t (if pred k :msg/fallback)))
        ",
        );
        assert!(
            keys.contains(":msg/hello"),
            "let-bound symbol in if branch should be resolved"
        );
        assert!(keys.contains(":msg/fallback"));
    }

    #[test]
    fn or_with_nested_if_collects_all_branches() {
        let keys = collect_from_source(r"(t (or (if pred :key/a :key/b) :key/fallback))");
        assert!(keys.contains(":key/a"));
        assert!(keys.contains(":key/b"));
        assert!(keys.contains(":key/fallback"));
    }

    // ── No false positives from condition positions ───────────────────────────

    #[test]
    fn no_false_positive_from_if_condition_in_let() {
        // :not-a-i18n/key appears only in the if condition, not a translation key
        let keys = collect_from_source(
            r"
            (let [k (if (= prop :not-a-i18n/key) :real/key :other/key)]
              (t k))
        ",
        );
        assert!(
            !keys.contains(":not-a-i18n/key"),
            "condition keyword in let binding should not be collected as a translation key"
        );
        assert!(keys.contains(":real/key"));
        assert!(keys.contains(":other/key"));
    }

    #[test]
    fn no_false_positive_from_cond_test_in_let() {
        // :not-a-i18n/key appears in a cond test position, not a translation key
        let keys = collect_from_source(
            r"
            (let [k (cond
                      (= type :not-a-i18n/key) :real/key
                      :else :fallback/key)]
              (t k))
        ",
        );
        assert!(
            !keys.contains(":not-a-i18n/key"),
            "cond test keyword in let binding should not be collected"
        );
        assert!(keys.contains(":real/key"));
        assert!(keys.contains(":fallback/key"));
    }

    #[test]
    fn no_false_positive_from_case_test_in_let() {
        // :not-a-i18n/key appears as a case test value, not a translation key
        let keys = collect_from_source(
            r"
            (let [k (case prop
                      :not-a-i18n/key :real/key
                      :fallback/key)]
              (t k))
        ",
        );
        assert!(
            !keys.contains(":not-a-i18n/key"),
            "case test-value keyword in let binding should not be collected"
        );
        assert!(keys.contains(":real/key"));
        assert!(keys.contains(":fallback/key"));
    }

    #[test]
    fn no_false_positive_from_def_condition() {
        // :not-a-i18n/key appears only in a condition inside a def value
        let keys = collect_from_source(
            r"
            (def my-key (if (= x :not-a-i18n/key) :real/key :other/key))
            (t my-key)
        ",
        );
        assert!(
            !keys.contains(":not-a-i18n/key"),
            "condition keyword in def value should not be collected"
        );
        assert!(keys.contains(":real/key"));
        assert!(keys.contains(":other/key"));
    }
}
