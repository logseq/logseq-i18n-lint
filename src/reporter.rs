use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use clap::ValueEnum;
use colored::Colorize;
use unicode_width::UnicodeWidthStr;

use crate::analyzer::{Diagnostic, DiagnosticKind};
use crate::config::AppConfig;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Table,
    Compact,
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Table => write!(f, "table"),
            Self::Compact => write!(f, "compact"),
        }
    }
}

pub fn report(diagnostics: &[Diagnostic], format: OutputFormat, config: &AppConfig, file_count: usize, base_dir: &Path) {
    match format {
        OutputFormat::Table => report_table(diagnostics, config, base_dir),
        OutputFormat::Compact => report_compact(diagnostics, config, base_dir),
    }

    println!();
    print_summary(diagnostics, file_count);
}

fn truncate_text(text: &str, max_len: usize) -> String {
    // Split on \n or \r to get first line
    let has_multiple_lines = text.contains('\n') || text.contains('\r');
    let first_line = text
        .split(['\n', '\r'])
        .next()
        .unwrap_or(text);

    let char_count = first_line.chars().count();
    if char_count > max_len {
        // Truncate to max_len characters, respecting char boundaries
        let cut = first_line
            .char_indices()
            .nth(max_len)
            .map_or(first_line.len(), |(i, _)| i);
        let truncated = &first_line[..cut];
        format!("{truncated}...")
    } else if has_multiple_lines {
        // Multiple lines: always append ... even if first line fits
        format!("{first_line}...")
    } else {
        first_line.to_string()
    }
}

fn kind_color(kind: DiagnosticKind) -> colored::Color {
    match kind {
        DiagnosticKind::HiccupText => colored::Color::Yellow,
        DiagnosticKind::HiccupAttr => colored::Color::Cyan,
        DiagnosticKind::AlertText => colored::Color::Red,
        DiagnosticKind::StrConcat => colored::Color::Magenta,
        DiagnosticKind::FormatString => colored::Color::Blue,
        DiagnosticKind::ConditionalText => colored::Color::Green,
        DiagnosticKind::FnArgText => colored::Color::White,
        DiagnosticKind::DefText => colored::Color::BrightBlack,
        DiagnosticKind::LetText => colored::Color::BrightBlue,
    }
}

fn report_compact(diagnostics: &[Diagnostic], config: &AppConfig, base_dir: &Path) {
    for diag in diagnostics {
        let rel_path = diag
            .file_path
            .strip_prefix(base_dir)
            .unwrap_or(&diag.file_path);
        let preview = truncate_text(&diag.text, config.lint.text_preview_length);
        let kind_str = format!("[{}]", diag.kind);

        println!(
            "{} {}:{} \"{}\"",
            kind_str.color(kind_color(diag.kind)),
            rel_path.display(),
            diag.line,
            preview,
        );
    }
}

struct Row {
    kind: DiagnosticKind,
    kind_str: String,
    file_str: String,
    line_str: String,
    text_str: String,
}

fn report_table(diagnostics: &[Diagnostic], config: &AppConfig, base_dir: &Path) {

    // Compute column widths using unicode-width for CJK support
    let type_header = "Type";
    let file_header = "File";
    let line_header = "Line";
    let text_header = "Text";

    let mut max_type_w = UnicodeWidthStr::width(type_header);
    let mut max_file_w = UnicodeWidthStr::width(file_header);
    let mut max_line_w = UnicodeWidthStr::width(line_header);
    let mut max_text_w = UnicodeWidthStr::width(text_header);

    let mut rows = Vec::with_capacity(diagnostics.len());

    for diag in diagnostics {
        let rel_path = diag
            .file_path
            .strip_prefix(base_dir)
            .unwrap_or(&diag.file_path);
        let kind_str = diag.kind.to_string();
        let file_str = rel_path.display().to_string().replace('\\', "/");
        let line_str = diag.line.to_string();
        let text_str = format!("\"{}\"", truncate_text(&diag.text, config.lint.text_preview_length));

        max_type_w = max_type_w.max(UnicodeWidthStr::width(kind_str.as_str()));
        max_file_w = max_file_w.max(UnicodeWidthStr::width(file_str.as_str()));
        max_line_w = max_line_w.max(UnicodeWidthStr::width(line_str.as_str()));
        max_text_w = max_text_w.max(UnicodeWidthStr::width(text_str.as_str()));

        rows.push(Row {
            kind: diag.kind,
            kind_str,
            file_str,
            line_str,
            text_str,
        });
    }

    // Print table
    let sep_top = format!(
        "┌{}┬{}┬{}┬{}┐",
        "─".repeat(max_type_w + 2),
        "─".repeat(max_file_w + 2),
        "─".repeat(max_line_w + 2),
        "─".repeat(max_text_w + 2),
    );
    let sep_mid = format!(
        "├{}┼{}┼{}┼{}┤",
        "─".repeat(max_type_w + 2),
        "─".repeat(max_file_w + 2),
        "─".repeat(max_line_w + 2),
        "─".repeat(max_text_w + 2),
    );
    let sep_bot = format!(
        "└{}┴{}┴{}┴{}┘",
        "─".repeat(max_type_w + 2),
        "─".repeat(max_file_w + 2),
        "─".repeat(max_line_w + 2),
        "─".repeat(max_text_w + 2),
    );

    println!("{sep_top}");
    println!(
        "│ {}{} │ {}{} │ {}{} │ {}{} │",
        type_header.bold(),
        " ".repeat(max_type_w - UnicodeWidthStr::width(type_header)),
        file_header.bold(),
        " ".repeat(max_file_w - UnicodeWidthStr::width(file_header)),
        line_header.bold(),
        " ".repeat(max_line_w - UnicodeWidthStr::width(line_header)),
        text_header.bold(),
        " ".repeat(max_text_w - UnicodeWidthStr::width(text_header)),
    );
    println!("{sep_mid}");

    for row in &rows {
        let type_pad = max_type_w - UnicodeWidthStr::width(row.kind_str.as_str());
        let file_pad = max_file_w - UnicodeWidthStr::width(row.file_str.as_str());
        let line_pad = max_line_w - UnicodeWidthStr::width(row.line_str.as_str());
        let text_pad = max_text_w - UnicodeWidthStr::width(row.text_str.as_str());

        println!(
            "│ {}{} │ {}{} │ {}{} │ {}{} │",
            row.kind_str.color(kind_color(row.kind)),
            " ".repeat(type_pad),
            row.file_str,
            " ".repeat(file_pad),
            " ".repeat(line_pad),
            row.line_str,
            row.text_str,
            " ".repeat(text_pad),
        );
    }

    println!("{sep_bot}");
}

fn print_summary(diagnostics: &[Diagnostic], file_count: usize) {
    let mut counts: BTreeMap<DiagnosticKind, usize> = BTreeMap::new();
    for diag in diagnostics {
        *counts.entry(diag.kind).or_insert(0) += 1;
    }

    let detail: Vec<String> = counts
        .iter()
        .map(|(kind, count)| {
            format!(
                "{}: {}",
                kind.to_string().color(kind_color(*kind)),
                count
            )
        })
        .collect();

    println!(
        "Found {file_count} hardcoded strings in {count} files ({detail})",
        file_count = diagnostics.len().to_string().yellow().bold(),
        count = file_count.to_string().cyan().bold(),
        detail = detail.join(", "),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_within_limit() {
        assert_eq!(truncate_text("hello", 10), "hello");
    }

    #[test]
    fn truncate_over_limit() {
        let result = truncate_text("hello world", 5);
        assert_eq!(result, "hello...");
    }

    #[test]
    fn truncate_unicode_boundary() {
        // 6 CJK chars with max_len=4 → truncated at char boundary with "..."
        let result = truncate_text("你好世界啊哦", 4);
        assert!(result.ends_with("..."));
        // 4 CJK chars with max_len=6 → fits, no truncation
        let no_trunc = truncate_text("你好世界", 6);
        assert_eq!(no_trunc, "你好世界");
    }

    #[test]
    fn truncate_multiline_short_first_line() {
        // First line short but there are more lines → always "..."
        let result = truncate_text("hi\nsecond line", 60);
        assert_eq!(result, "hi...");
    }

    #[test]
    fn truncate_multiline_long_first_line() {
        // First line also exceeds max_len → truncated AND "..."
        let result = truncate_text("hello world\nsecond", 5);
        assert_eq!(result, "hello...");
    }

    #[test]
    fn truncate_crlf() {
        let result = truncate_text("line one\r\nline two", 60);
        assert_eq!(result, "line one...");
    }

    #[test]
    fn output_format_display() {
        assert_eq!(OutputFormat::Table.to_string(), "table");
        assert_eq!(OutputFormat::Compact.to_string(), "compact");
    }
}
