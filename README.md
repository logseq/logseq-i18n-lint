# logseq-i18n-lint

AST-level detection of hardcoded UI strings in Clojure/ClojureScript source code.

## Overview

`logseq-i18n-lint` analyzes Clojure/ClojureScript source files at the AST level to find hardcoded UI strings that should be internationalized. Unlike regex-based approaches, it understands the code structure — hiccup vectors, function calls, attribute maps — to accurately detect strings that are displayed to users.

## Features

- **AST-level analysis** — Custom Clojure/ClojureScript parser, not regex matching
- **8 detection categories** — hiccup-text, hiccup-attr, alert-text, str-concat, format-string, conditional-text, fn-arg-text, let-text
- **Configurable exclusions** — Allow lists, regex patterns, ignore context functions
- **Git integration** — Check only changed files with `--git-changed`
- **Parallel processing** — Uses rayon for fast multi-file analysis
- **Cross-platform** — Prebuilt binaries for Windows, macOS, Linux (x64 & ARM64)
- **Two output formats** — Aligned table with Unicode support, or compact for CI

## Installation

### Download prebuilt binary

Download from [Releases](https://github.com/logseq/logseq-i18n-lint/releases/latest) for your platform.

### Build from source

```bash
cargo install --git https://github.com/logseq/logseq-i18n-lint
```

Or clone and build:

```bash
git clone https://github.com/logseq/logseq-i18n-lint
cd logseq-i18n-lint
cargo build --release
```

## Quick Start

```bash
# Run in the logseq project root
logseq-i18n-lint

# Use a custom config
logseq-i18n-lint -c .i18n-lint.toml

# Check only git-changed files
logseq-i18n-lint --git-changed

# Compact output for CI
logseq-i18n-lint -f compact

# Warn only (exit 0 even if issues found)
logseq-i18n-lint --warn-only
```

## Configuration

Create `.i18n-lint.toml` in your project root. If not present, built-in defaults are used.

```toml
# Root of the project to analyse, relative to the directory where you run the binary
# Leave empty to use the current working directory
project_root = ""

# Directories to scan
include_dirs = [
  "src",
]

# Exclude patterns (glob)
exclude_patterns = [
  "**/test/**",
  "**/node_modules/**",
]

# File extensions
file_extensions = ["clj", "cljs", "cljc"]

# Max text preview length in output
text_preview_length = 60

# Strings to allow (exact match)
allow_strings = ["Logseq"]

# Patterns to allow (regex)
allow_patterns = ["^https?://"]

# Translation functions — strings passed to these are not flagged
i18n_functions = ["t", "i18n/t"]

# Alert/notification functions — first string arg is flagged as alert-text
alert_functions = ["notification/show!"]

# Exception functions — first string is a developer message, not UI text
exception_functions = ["throw"]

# Pure/comparison functions — args are data values, not UI text
pure_functions = []

# Format/printf functions — first arg (template) is flagged only inside UI context
format_functions = ["format", "goog.string/format"]

# UI function names (string args are flagged)
ui_functions = ["ui/button"]

# Namespace prefixes whose every function is treated as a UI function
ui_namespaces = []

# UI attribute names (string values are flagged)
ui_attributes = [
  "placeholder",
  "title",
  "aria-label",
  "alt",
  "label",
]

# Functions whose args are NOT checked
ignore_context_functions = [
  "js/console.log",
  "log/debug",
  "prn",
  "re-pattern",
  "ns",
]
```

## CLI Reference

```
logseq-i18n-lint [OPTIONS]

Options:
  -c, --config <PATH>    Configuration file path [default: .i18n-lint.toml]
  -f, --format <FORMAT>  Output format: table|compact [default: table]
  -w, --warn-only        Warn only, do not exit with error code
  -g, --git-changed      Only check git changed files
  -v, --verbose          Verbose output
  -h, --help             Print help
  -V, --version          Print version
```

## Detection Categories

| Type | Description | Example |
|------|-------------|---------|
| `hiccup-text` | Text nodes in hiccup vectors | `[:div "Hello"]` |
| `hiccup-attr` | UI text in hiccup attributes | `{:placeholder "Search..."}` |
| `fn-arg-text` | UI function string arguments | `(ui/button "Submit")` |
| `str-concat` | String concatenation in UI context | `(str "Error: " msg)` |
| `conditional-text` | Text in conditionals in UI context | `(if x "Yes" "No")` |
| `format-string` | Format strings in UI context | `(goog.string/format "Found %d")` |
| `let-text` | let binding in UI context | `(let [x "Untitled"] [:div x])` |
| `alert-text` | Alert/notification messages | `(notification/show! "Done")` |

> **Deep dive:** See [docs/detection-rules.md](docs/detection-rules.md) for how each
> rule works, automatic skip filters, known limitations, and configuration tips.

## Output Formats

### Table mode (default)

```
┌──────────────────┬─────────────────────────┬──────┬────────────────────────────┐
│ Type             │ File                    │ Line │ Text                       │
├──────────────────┼─────────────────────────┼──────┼────────────────────────────┤
│ hiccup-text      │ src/frontend/editor.cljs│   48 │ "No matched commands"      │
│ hiccup-attr      │ src/frontend/search.cljs│   92 │ "Search pages..."          │
└──────────────────┴─────────────────────────┴──────┴────────────────────────────┘

Found 2 hardcoded strings in 317 files (hiccup-text: 1, hiccup-attr: 1)
```

### Compact mode

```
[hiccup-text] src/frontend/editor.cljs:48 "No matched commands"
[hiccup-attr] src/frontend/search.cljs:92 "Search pages..."

Found 2 hardcoded strings in 317 files (hiccup-text: 1, hiccup-attr: 1)
```

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# Run clippy
cargo clippy

# Run benchmarks
cargo bench

# Build release
cargo build --release
```

## License

MIT
