use std::path::PathBuf;

use logseq_i18n_lint::analyzer::{self, DiagnosticKind};
use logseq_i18n_lint::config::AppConfig;
use logseq_i18n_lint::parser;

fn fixture_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(relative)
}

fn test_config() -> AppConfig {
    let toml = r#"
        i18n_functions      = ["t", "tr", "translate"]
        exception_functions = ["ex-info", "throw"]
        alert_functions     = ["notification/show!"]
        format_functions    = ["format", "goog.string/format"]
        ui_functions        = ["ui/button"]
        ui_namespaces       = ["shui"]
        ui_attributes       = ["placeholder", "title", "aria-label", "alt", "label"]
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
