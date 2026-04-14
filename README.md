# logseq-i18n-lint

AST-level detection of hardcoded UI strings in Clojure/ClojureScript source code.

## Overview

`logseq-i18n-lint` analyzes Clojure/ClojureScript source files at the AST level to find hardcoded UI strings that should be internationalized. Unlike regex-based approaches, it understands the code structure — hiccup vectors, function calls, attribute maps — to accurately detect strings that are displayed to users.

## Features

- **AST-level analysis** — Custom Clojure/ClojureScript parser, not regex matching
- **8 detection categories** — hiccup-text, hiccup-attr, alert-text, str-concat, format-string, conditional-text, fn-arg-text, let-text
- **Unused key detection** — Find translation keys defined in dictionaries but never referenced in code
- **Auto-fix** — Remove unused keys from all dictionary files with `--fix`
- **DB-ident key derivation** — Automatically resolves built-in property/class keys from db-ident definitions
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
# Run lint on the project root
logseq-i18n-lint lint

# Use a custom config
logseq-i18n-lint -c .i18n-lint.toml lint

# Check only git-changed files
logseq-i18n-lint lint --git-changed

# Compact output for CI
logseq-i18n-lint lint -f compact

# Warn only (exit 0 even if issues found)
logseq-i18n-lint lint --warn-only

# Check for unused translation keys
logseq-i18n-lint check-keys

# Remove unused keys from all dictionaries
logseq-i18n-lint check-keys --fix
```

> **Note:** The configuration flag `-c` is a global flag and must come **before** the subcommand:
> `logseq-i18n-lint -c .i18n-lint.toml lint`

## Configuration

Create `.i18n-lint.toml` in your project root. If not present, built-in defaults are used.

```toml
# Path to the project root, relative to the directory that contains the
# executable.  Resolution is always based on the executable location, so
# the result is the same no matter which directory you run the binary from.
# Leave empty when the executable is placed at the project root;
# set to ".." when it lives in a subdirectory such as bin/.
project_root = ""

# Shared settings used by both subcommands
include_dirs    = ["src"]
file_extensions = ["clj", "cljs", "cljc"]

# Translation functions — calls to these are never flagged
i18n_functions = ["t", "i18n/t"]

# Alert/notification functions — first keyword arg is a translation key
alert_functions = ["notification/show!"]

# UI component functions — keyword args are translation keys
ui_functions   = []
ui_namespaces  = []

# Hiccup attribute names whose string values are flagged
ui_attributes  = ["placeholder", "title", "aria-label", "alt", "label"]

# ── lint settings ──────────────────────────────────────────────────────────────
[lint]
exclude_patterns    = ["**/test/**", "**/node_modules/**"]
text_preview_length = 60
allow_strings       = ["Logseq"]
allow_patterns      = ["^https?://"]
exception_functions = ["throw"]
pure_functions      = []
format_functions    = ["format", "goog.string/format"]
ignore_context_functions = [
  "js/console.log",
  "log/debug",
  "prn",
  "re-pattern",
  "ns",
]

# ── check-keys settings ────────────────────────────────────────────────────────
[check-keys]
dicts_dir                  = "src/resources/dicts"
primary_dict               = "src/resources/dicts/en.edn"
always_used_key_patterns   = ["^:command\\."]
ignore_key_namespaces      = []
translation_key_attributes = ["i18n-key", "prompt-key", "title-key"]

# Built-in db-ident definitions (one entry per source file)
[[check-keys.db_ident_defs]]
file = "deps/db/src/logseq/db/frontend/property.cljs"
def  = "built-in-properties"

[[check-keys.db_ident_defs]]
file = "deps/db/src/logseq/db/frontend/class.cljs"
def  = "built-in-classes"
```

## CLI Reference

```
logseq-i18n-lint [GLOBAL_OPTIONS] <COMMAND> [COMMAND_OPTIONS]

Commands:
  lint        Detect hardcoded UI strings
  check-keys  Check for unused translation keys

Global Options:
  -c, --config <PATH>    Configuration file path [default: .i18n-lint.toml]
  -v, --verbose          Verbose output
  -h, --help             Print help
  -V, --version          Print version

lint Options:
  -f, --format <FORMAT>  Output format: table|compact [default: table]
  -w, --warn-only        Warn only, do not exit with error code
  -g, --git-changed      Only check git changed files

check-keys Options:
  --fix                  Remove unused keys from all dictionary files
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

> **Deep dive:** See [docs/hardcoded-string-detection.md](docs/hardcoded-string-detection.md) for how each
> rule works, automatic skip filters, known limitations, and configuration tips.
>
> For unused key detection, see [docs/unused-key-detection.md](docs/unused-key-detection.md).

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
