use std::path::PathBuf;

use logseq_i18n_lint::analyzer::{self, DiagnosticKind};
use logseq_i18n_lint::checker;
use logseq_i18n_lint::config::AppConfig;
use logseq_i18n_lint::edn;
use logseq_i18n_lint::key_collector;
use logseq_i18n_lint::parser;

fn fixture_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(relative)
}

fn test_config() -> AppConfig {
    let toml = r#"
        i18n_functions      = ["t", "tr", "translate"]
        alert_functions     = ["notification/show!"]
        ui_functions        = ["ui/button"]
        ui_namespaces       = ["shui"]
        ui_attributes       = ["placeholder", "title", "aria-label", "alt", "label"]

        [lint]
        exception_functions = ["ex-info", "throw"]
        format_functions    = ["format", "goog.string/format"]
        ignore_context_functions = [
            "js/console.log", "js/console.error", "js/console.warn",
            "prn", "println", "log/debug", "log/info", "log/warn", "log/error",
            "re-pattern", "re-find", "re-matches", "require", "ns",
        ]
        allow_strings       = ["Logseq"]
        allow_patterns      = [
            "^https?://", "^#[0-9a-fA-F]{3,8}$", "^\\.[a-z]",
            "^[a-z][a-z0-9-]*$", "^[A-Z][A-Z0-9_]+$",
        ]
        pure_functions      = []
    "#;
    toml::from_str(toml).expect("test config is valid TOML")
}

fn analyze_fixture(relative: &str) -> Vec<analyzer::Diagnostic> {
    let path = fixture_path(relative);
    let config = test_config();
    let source = std::fs::read_to_string(&path).unwrap();
    let forms = parser::parse(&source).unwrap();
    analyzer::analyze_source_with_config(&forms, &path, &config)
}

// ─── Parser fixture tests ───

#[test]
fn parse_basic_sexp_fixture() {
    let path = fixture_path("parser/basic_sexp.cljs");
    let source = std::fs::read_to_string(&path).unwrap();
    let forms = parser::parse(&source).unwrap();
    assert!(!forms.is_empty(), "should parse basic s-expressions");
}

#[test]
fn parse_string_escape_fixture() {
    let path = fixture_path("parser/string_escape.cljs");
    let source = std::fs::read_to_string(&path).unwrap();
    let forms = parser::parse(&source).unwrap();
    assert!(!forms.is_empty(), "should parse string escapes");
}

#[test]
fn parse_nested_deep_fixture() {
    let path = fixture_path("parser/nested_deep.cljs");
    let source = std::fs::read_to_string(&path).unwrap();
    let forms = parser::parse(&source).unwrap();
    assert!(!forms.is_empty(), "should parse deeply nested structures");
}

#[test]
fn parse_reader_macros_fixture() {
    let path = fixture_path("parser/reader_macros.cljs");
    let source = std::fs::read_to_string(&path).unwrap();
    let forms = parser::parse(&source).unwrap();
    assert!(!forms.is_empty(), "should parse reader macros");
}

#[test]
fn parse_regex_literals_fixture() {
    let path = fixture_path("parser/regex_literals.cljs");
    let source = std::fs::read_to_string(&path).unwrap();
    let forms = parser::parse(&source).unwrap();
    assert!(!forms.is_empty(), "should parse regex literals");
}

#[test]
fn parse_comments_fixture() {
    let path = fixture_path("parser/comments.cljs");
    let source = std::fs::read_to_string(&path).unwrap();
    let forms = parser::parse(&source).unwrap();
    assert!(!forms.is_empty(), "should parse comments");
}

#[test]
fn parse_edge_cases_fixture() {
    let path = fixture_path("parser/edge_cases.cljs");
    let source = std::fs::read_to_string(&path).unwrap();
    let forms = parser::parse(&source).unwrap();
    assert!(!forms.is_empty(), "should parse edge cases");
}

// ─── Analyzer fixture tests ───

#[test]
fn analyze_hiccup_text_fixture() {
    let diags = analyze_fixture("analyzer/hiccup_text.cljs");
    let hiccup_texts: Vec<_> = diags
        .iter()
        .filter(|d| d.kind == DiagnosticKind::HiccupText)
        .collect();
    assert!(
        hiccup_texts.len() >= 4,
        "should detect hiccup text nodes, found: {}",
        hiccup_texts.len()
    );
}

#[test]
fn analyze_hiccup_attr_fixture() {
    let diags = analyze_fixture("analyzer/hiccup_attr.cljs");
    let attrs: Vec<_> = diags
        .iter()
        .filter(|d| d.kind == DiagnosticKind::HiccupAttr)
        .collect();
    assert!(
        attrs.len() >= 3,
        "should detect hiccup attribute values, found: {}",
        attrs.len()
    );
}

#[test]
fn analyze_fn_arg_fixture() {
    let diags = analyze_fixture("analyzer/fn_arg.cljs");
    let fn_args: Vec<_> = diags
        .iter()
        .filter(|d| d.kind == DiagnosticKind::FnArgText)
        .collect();
    assert!(
        fn_args.len() >= 2,
        "should detect UI function arguments, found: {}",
        fn_args.len()
    );
}

#[test]
fn analyze_conditional_fixture() {
    let diags = analyze_fixture("analyzer/conditional.cljs");
    let conds: Vec<_> = diags
        .iter()
        .filter(|d| d.kind == DiagnosticKind::ConditionalText)
        .collect();
    assert!(
        conds.len() >= 3,
        "should detect conditional text, found: {}",
        conds.len()
    );
}

#[test]
fn analyze_notification_fixture() {
    let diags = analyze_fixture("analyzer/notification.cljs");
    let notifs: Vec<_> = diags
        .iter()
        .filter(|d| d.kind == DiagnosticKind::AlertText)
        .collect();
    assert!(
        notifs.len() >= 3,
        "should detect notification messages, found: {}",
        notifs.len()
    );
}

#[test]
fn analyze_str_concat_fixture() {
    let diags = analyze_fixture("analyzer/str_concat.cljs");
    let concats: Vec<_> = diags
        .iter()
        .filter(|d| d.kind == DiagnosticKind::StrConcat)
        .collect();
    assert!(
        concats.len() >= 2,
        "should detect str concat in hiccup, found: {}",
        concats.len()
    );
}

#[test]
fn analyze_allow_strings_fixture() {
    let diags = analyze_fixture("analyzer/allow_strings.cljs");
    assert!(
        diags.is_empty(),
        "should not report any allowed strings, but found: {diags:?}",
    );
}

#[test]
fn analyze_ignore_context_fixture() {
    let diags = analyze_fixture("analyzer/ignore_context.cljs");
    assert!(
        diags.is_empty(),
        "should not report any strings in ignore context, but found: {diags:?}",
    );
}

// ─── Integration test ───

#[test]
fn integration_mixed_patterns() {
    let diags = analyze_fixture("integration/mixed_patterns.cljs");

    // Should find multiple types
    let kinds: Vec<_> = diags.iter().map(|d| d.kind).collect();

    assert!(
        kinds.contains(&DiagnosticKind::HiccupText),
        "should find hiccup-text in mixed patterns"
    );
    assert!(
        kinds.contains(&DiagnosticKind::HiccupAttr),
        "should find hiccup-attr in mixed patterns"
    );
    assert!(
        kinds.contains(&DiagnosticKind::AlertText),
        "should find alert-text in mixed patterns"
    );
    assert!(
        kinds.contains(&DiagnosticKind::ConditionalText),
        "should find conditional-text in mixed patterns"
    );

    // Should NOT find any strings from ignored contexts
    for diag in &diags {
        assert!(
            !diag.text.contains("render called"),
            "should not report console.log text"
        );
        assert!(
            !diag.text.contains("Component mounted"),
            "should not report log/debug text"
        );
    }
}

#[test]
fn analyze_fn_hiccup_return_fixture() {
    let diags = analyze_fixture("analyzer/fn_hiccup_return.cljs");

    // Hiccup text inside fn body must be reported.
    let texts: Vec<&str> = diags.iter().map(|d| d.text.as_str()).collect();
    assert!(
        texts.contains(&"Hidden in fn"),
        "should detect hiccup-text inside anonymous fn body, found: {texts:?}"
    );
    assert!(
        texts.contains(&"Multi-body return"),
        "should detect hiccup-text in multi-form fn body, found: {texts:?}"
    );

    // Hiccup attr inside fn body must also be reported.
    let attr_texts: Vec<&str> = diags
        .iter()
        .filter(|d| d.kind == DiagnosticKind::HiccupAttr)
        .map(|d| d.text.as_str())
        .collect();
    assert!(
        attr_texts.contains(&"Hidden label"),
        "should detect hiccup-attr inside anonymous fn body, found: {attr_texts:?}"
    );

    // DOM key comparison inside event handler must NOT appear.
    assert!(
        !texts.contains(&"Enter"),
        "should NOT report DOM key comparison 'Enter' inside event handler"
    );
}

#[test]
fn analyze_format_string_fixture() {
    let diags = analyze_fixture("analyzer/format_string.cljs");
    let fmt_strs: Vec<_> = diags
        .iter()
        .filter(|d| d.kind == DiagnosticKind::FormatString)
        .collect();
    assert!(
        fmt_strs.len() >= 2,
        "should detect format strings inside hiccup, found: {}",
        fmt_strs.len()
    );
    // Format calls outside UI context must not appear.
    assert!(
        !diags.iter().any(|d| d.text.contains("log:")),
        "format string outside UI context should not be reported"
    );
    assert!(
        !diags.iter().any(|d| d.text.contains("key-")),
        "format string outside UI context should not be reported"
    );
    assert!(
        !diags.iter().any(|d| d.text.contains("item-")),
        "format string in non-UI function body should not be reported"
    );
}

// ─── check-keys tests ───

fn checker_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/checker")
}

fn checker_config() -> AppConfig {
    let toml = r#"
        include_dirs = ["src"]
        file_extensions = ["clj", "cljs", "cljc"]
        i18n_functions = ["t", "tt", "i18n/t"]

        [check-keys]
        dicts_dir = "dicts"
        primary_dict = "dicts/en.edn"
        translation_key_attributes = ["i18n-key", "prompt-key", "title-key"]
        always_used_key_patterns = [
            "^:shortcut\\.",
            "^:color\\.",
        ]
        ignore_key_namespaces = [
            "deprecated.config",
        ]

        [[check-keys.db_ident_defs]]
        file = "db-idents/property.cljs"
        def  = "built-in-properties"

        [[check-keys.db_ident_defs]]
        file = "db-idents/class.cljs"
        def  = "built-in-classes"
    "#;
    toml::from_str(toml).expect("checker test config is valid TOML")
}

#[test]
fn edn_parse_dict_keys() {
    let dict_path = checker_fixture_path().join("dicts/en.edn");
    let keys = edn::parse_dict_keys(&dict_path).unwrap();
    assert!(keys.contains(":ui/save"));
    assert!(keys.contains(":unused/orphan"));
    assert!(keys.contains(":shortcut.editor/copy"));
    assert!(keys.contains(":deprecated.config/old-setting"));
    assert!(
        keys.len() >= 26,
        "should parse all keys, found {}",
        keys.len()
    );
}

#[test]
fn key_collector_finds_direct_calls() {
    let config = checker_config();
    let src = checker_fixture_path().join("src/app.cljs");
    let keys = key_collector::collect_referenced_keys(&[src], &config);
    assert!(
        keys.contains(":ui/save"),
        "should find (t :ui/save), found: {keys:?}"
    );
    assert!(keys.contains(":ui/cancel"), "should find (t :ui/cancel)");
    assert!(keys.contains(":nav/home"), "should find (tt :nav/home)");
}

#[test]
fn key_collector_finds_conditional_keys() {
    let config = checker_config();
    let src = checker_fixture_path().join("src/app.cljs");
    let keys = key_collector::collect_referenced_keys(&[src], &config);
    assert!(
        keys.contains(":ui/loading"),
        "should find :ui/loading from (if ...)"
    );
    assert!(
        keys.contains(":ui/ready"),
        "should find :ui/ready from (if ...)"
    );
}

#[test]
fn key_collector_finds_map_payload_keys() {
    let config = checker_config();
    let src = checker_fixture_path().join("src/app.cljs");
    let keys = key_collector::collect_referenced_keys(&[src], &config);
    assert!(
        keys.contains(":dialog/confirm"),
        "should find :i18n-key :dialog/confirm"
    );
    assert!(
        keys.contains(":dialog/title"),
        "should find :title-key :dialog/title"
    );
    assert!(
        keys.contains(":dialog/prompt"),
        "should find :prompt-key :dialog/prompt"
    );
}

#[test]
fn key_collector_finds_i18n_key_with_if() {
    let config = checker_config();
    let src = checker_fixture_path().join("src/app.cljs");
    let keys = key_collector::collect_referenced_keys(&[src], &config);
    assert!(
        keys.contains(":theme/dark"),
        "should find :theme/dark from :i18n-key (if ...)"
    );
    assert!(
        keys.contains(":theme/light"),
        "should find :theme/light from :i18n-key (if ...)"
    );
}

#[test]
fn key_collector_finds_cond_keys() {
    let config = checker_config();
    let src = checker_fixture_path().join("src/app.cljs");
    let keys = key_collector::collect_referenced_keys(&[src], &config);
    assert!(
        keys.contains(":msg/type-a"),
        "should find :msg/type-a from cond"
    );
    assert!(
        keys.contains(":msg/type-b"),
        "should find :msg/type-b from cond"
    );
    assert!(
        keys.contains(":msg/default"),
        "should find :msg/default from cond"
    );
}

#[test]
fn key_collector_symbol_resolution() {
    let config = checker_config();
    let src = checker_fixture_path().join("src/app.cljs");
    let keys = key_collector::collect_referenced_keys(&[src], &config);
    assert!(
        keys.contains(":view/option-a"),
        "should resolve :view/option-a from defonce via (t view-options)"
    );
    assert!(
        keys.contains(":view/option-b"),
        "should resolve :view/option-b from defonce via (t view-options)"
    );
}

#[test]
fn checker_finds_unused_keys() {
    let config = checker_config();
    let base_dir = checker_fixture_path();
    let result = checker::check_unused_keys(&config, &base_dir).unwrap();

    assert!(
        result.unused_keys.contains(&":unused/orphan".to_string()),
        "should detect :unused/orphan as unused, found: {:?}",
        result.unused_keys
    );
    assert!(
        result.unused_keys.contains(&":unused/stale".to_string()),
        "should detect :unused/stale as unused"
    );
}

#[test]
fn checker_always_used_filters() {
    let config = checker_config();
    let base_dir = checker_fixture_path();
    let result = checker::check_unused_keys(&config, &base_dir).unwrap();

    // :shortcut.* and :color.* should NOT appear as unused
    assert!(
        !result
            .unused_keys
            .iter()
            .any(|k| k.starts_with(":shortcut.")),
        "shortcut keys should be filtered by always_used_key_patterns"
    );
    assert!(
        !result.unused_keys.iter().any(|k| k.starts_with(":color.")),
        "color keys should be filtered by always_used_key_patterns"
    );
}

#[test]
fn checker_ignore_namespace_filters() {
    let config = checker_config();
    let base_dir = checker_fixture_path();
    let result = checker::check_unused_keys(&config, &base_dir).unwrap();

    assert!(
        !result
            .unused_keys
            .iter()
            .any(|k| k.starts_with(":deprecated.config")),
        "deprecated.config keys should be filtered by ignore_key_namespaces"
    );
}

#[test]
fn checker_fix_removes_keys() {
    // Create a temporary copy of the fixture to test fix
    let temp_dir = tempfile::tempdir().unwrap();
    let dicts_dir = temp_dir.path().join("dicts");
    let src_dir = temp_dir.path().join("src");
    let db_idents_dir = temp_dir.path().join("db-idents");
    std::fs::create_dir_all(&dicts_dir).unwrap();
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::create_dir_all(&db_idents_dir).unwrap();

    // Copy fixtures
    std::fs::copy(
        checker_fixture_path().join("dicts/en.edn"),
        dicts_dir.join("en.edn"),
    )
    .unwrap();
    std::fs::copy(
        checker_fixture_path().join("dicts/es.edn"),
        dicts_dir.join("es.edn"),
    )
    .unwrap();
    std::fs::copy(
        checker_fixture_path().join("src/app.cljs"),
        src_dir.join("app.cljs"),
    )
    .unwrap();
    std::fs::copy(
        checker_fixture_path().join("db-idents/property.cljs"),
        db_idents_dir.join("property.cljs"),
    )
    .unwrap();
    std::fs::copy(
        checker_fixture_path().join("db-idents/class.cljs"),
        db_idents_dir.join("class.cljs"),
    )
    .unwrap();

    let config = checker_config();
    let result = checker::check_unused_keys(&config, temp_dir.path()).unwrap();
    assert!(
        !result.unused_keys.is_empty(),
        "should find unused keys to fix"
    );

    // Fix
    checker::fix_unused_keys(&config, temp_dir.path(), &result.unused_keys).unwrap();

    // Verify keys are removed from en.edn
    let en_dict = std::fs::read_to_string(dicts_dir.join("en.edn")).unwrap();
    assert!(
        !en_dict.contains(":unused/orphan"),
        "en.edn should not contain :unused/orphan after fix"
    );
    assert!(
        !en_dict.contains(":unused/stale"),
        "en.edn should not contain :unused/stale after fix"
    );
    assert!(
        en_dict.contains(":ui/save"),
        "en.edn should still contain :ui/save"
    );

    // Verify keys are removed from es.edn too
    let spanish = std::fs::read_to_string(dicts_dir.join("es.edn")).unwrap();
    assert!(
        !spanish.contains(":unused/orphan"),
        "es.edn should not contain :unused/orphan after fix"
    );
    assert!(
        spanish.contains(":ui/save"),
        "es.edn should still contain :ui/save"
    );

    // Re-check: should find no unused keys
    let result2 = checker::check_unused_keys(&config, temp_dir.path()).unwrap();
    assert!(
        result2.unused_keys.is_empty(),
        "after fix, no unused keys should remain, found: {:?}",
        result2.unused_keys
    );
}

#[test]
fn checker_no_false_positives_on_unused() {
    let config = checker_config();
    let base_dir = checker_fixture_path();
    let result = checker::check_unused_keys(&config, &base_dir).unwrap();

    // Used keys should NOT appear in returned unused list
    let used_keys = [
        ":ui/save",
        ":ui/cancel",
        ":ui/loading",
        ":ui/ready",
        ":nav/home",
        ":dialog/confirm",
        ":dialog/title",
        ":dialog/prompt",
        ":theme/dark",
        ":theme/light",
        ":msg/type-a",
        ":msg/type-b",
        ":msg/default",
        ":view/option-a",
        ":view/option-b",
    ];
    for key in &used_keys {
        assert!(
            !result.unused_keys.contains(&key.to_string()),
            "{key} should NOT be reported as unused"
        );
    }
}

#[test]
fn checker_db_ident_keys_are_referenced() {
    let config = checker_config();
    let base_dir = checker_fixture_path();
    let result = checker::check_unused_keys(&config, &base_dir).unwrap();

    // :property.built-in/status (from :logseq.property/status) should NOT be unused
    assert!(
        !result
            .unused_keys
            .contains(&":property.built-in/status".to_string()),
        "property.built-in/status should be referenced via db-ident conversion"
    );
    // :property.built-in/alias (from :block/alias) should NOT be unused
    assert!(
        !result
            .unused_keys
            .contains(&":property.built-in/alias".to_string()),
        "property.built-in/alias should be referenced via db-ident conversion"
    );
    // :class.built-in/task (from :logseq.class/Task) should NOT be unused
    assert!(
        !result
            .unused_keys
            .contains(&":class.built-in/task".to_string()),
        "class.built-in/task should be referenced via db-ident conversion"
    );
}

#[test]
fn checker_db_ident_phantom_is_unused() {
    let config = checker_config();
    let base_dir = checker_fixture_path();
    let result = checker::check_unused_keys(&config, &base_dir).unwrap();

    // :property.built-in/phantom has no db-ident source, should be unused
    assert!(
        result
            .unused_keys
            .contains(&":property.built-in/phantom".to_string()),
        "property.built-in/phantom should be reported as unused (no db-ident source), found: {:?}",
        result.unused_keys
    );
}

// ─── check-missing tests ───

fn check_missing_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/check_missing")
}

fn check_missing_config() -> AppConfig {
    let toml = r#"
        include_dirs = ["src"]
        file_extensions = ["clj", "cljs", "cljc"]
        i18n_functions = ["t", "tt", "i18n/t"]

        [check-keys]
        dicts_dir = "dicts"
        primary_dict = "dicts/en.edn"
        always_used_key_patterns = [
            "^:shortcut/",
        ]
        ignore_key_namespaces = [
            "deprecated",
        ]
    "#;
    toml::from_str(toml).expect("check_missing test config is valid TOML")
}

#[test]
fn check_missing_finds_undefined_keys() {
    let config = check_missing_config();
    let base_dir = check_missing_fixture_path();
    let result = checker::check_missing_keys(&config, &base_dir).unwrap();

    assert!(
        result
            .missing_keys
            .iter()
            .any(|e| e.key == ":sidebar/title"),
        "should detect :sidebar/title as missing, found: {:?}",
        result
            .missing_keys
            .iter()
            .map(|e| &e.key)
            .collect::<Vec<_>>()
    );
    assert!(
        result
            .missing_keys
            .iter()
            .any(|e| e.key == ":dialog/confirm-delete"),
        "should detect :dialog/confirm-delete as missing, found: {:?}",
        result
            .missing_keys
            .iter()
            .map(|e| &e.key)
            .collect::<Vec<_>>()
    );
}

#[test]
fn check_missing_no_false_positives() {
    let config = check_missing_config();
    let base_dir = check_missing_fixture_path();
    let result = checker::check_missing_keys(&config, &base_dir).unwrap();

    for key in &[":ui/save", ":ui/cancel", ":nav/home"] {
        assert!(
            !result.missing_keys.iter().any(|e| e.key == *key),
            "{key} is defined and should not be reported as missing, found: {:?}",
            result
                .missing_keys
                .iter()
                .map(|e| &e.key)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn check_missing_always_used_filters() {
    let config = check_missing_config();
    let base_dir = check_missing_fixture_path();
    let result = checker::check_missing_keys(&config, &base_dir).unwrap();

    assert!(
        !result
            .missing_keys
            .iter()
            .any(|e| e.key.starts_with(":shortcut/")),
        "shortcut keys should be filtered by always_used_key_patterns, found: {:?}",
        result
            .missing_keys
            .iter()
            .map(|e| &e.key)
            .collect::<Vec<_>>()
    );
}

#[test]
fn check_missing_ignore_namespace_filters() {
    let config = check_missing_config();
    let base_dir = check_missing_fixture_path();
    let result = checker::check_missing_keys(&config, &base_dir).unwrap();

    assert!(
        !result
            .missing_keys
            .iter()
            .any(|e| e.key == ":deprecated/extra-key"),
        "deprecated/extra-key should be filtered by ignore_key_namespaces, found: {:?}",
        result
            .missing_keys
            .iter()
            .map(|e| &e.key)
            .collect::<Vec<_>>()
    );
}

#[test]
fn check_missing_all_defined_returns_empty() {
    let temp_dir = tempfile::tempdir().unwrap();
    let dicts_dir = temp_dir.path().join("dicts");
    let src_dir = temp_dir.path().join("src");
    std::fs::create_dir_all(&dicts_dir).unwrap();
    std::fs::create_dir_all(&src_dir).unwrap();

    std::fs::write(
        dicts_dir.join("en.edn"),
        "{\n :ui/save \"Save\"\n :ui/cancel \"Cancel\"\n}\n",
    )
    .unwrap();

    std::fs::write(
        src_dir.join("app.cljs"),
        "(ns test.app)\n(defn render [] (t :ui/save) (t :ui/cancel))\n",
    )
    .unwrap();

    let config: AppConfig = toml::from_str(
        r#"
        include_dirs = ["src"]
        i18n_functions = ["t"]
        [check-keys]
        dicts_dir = "dicts"
        primary_dict = "dicts/en.edn"
    "#,
    )
    .unwrap();

    let result = checker::check_missing_keys(&config, temp_dir.path()).unwrap();
    assert!(
        result.missing_keys.is_empty(),
        "all keys defined, should have no missing keys, found: {:?}",
        result
            .missing_keys
            .iter()
            .map(|e| &e.key)
            .collect::<Vec<_>>()
    );
}
