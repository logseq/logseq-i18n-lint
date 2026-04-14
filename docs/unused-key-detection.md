# Unused Key Detection — Design & Configuration

This document explains how the `check-keys` subcommand detects unused
translation keys and how to configure it for your project.

## Overview

The `check-keys` subcommand compares keys defined in a primary dictionary file
(EDN format) against keys actually referenced in source code. Keys that are
defined but never referenced are reported as unused. An optional `--fix` flag
removes them from all dictionary files.

## How It Works

### 1. Parse Dictionary Keys

The primary dictionary file (e.g. `src/resources/dicts/en.edn`) is parsed to
extract all top-level keyword keys. The expected format:

```edn
{
 :namespace/key-name "Translation text"
 :another.ns/key     "Another translation"
}
```

### 2. Collect Referenced Keys (Static Analysis)

Source files matching the scanner configuration (`include_dirs`, `file_extensions`,
and `[check-keys].exclude_patterns`) are parsed into ASTs. The collector uses a
two-pass analysis per file:

**Pass 1 — Symbol Table**

Top-level `def` and `defonce` bindings are scanned. Keywords found in the bound
value are recorded in a symbol table, enabling resolution when the symbol is
later passed to a translation function.

```clojure
;; Symbol table records: sort-options → [:view.table/sort-asc, :view.table/sort-desc]
(defonce sort-options
  [[:view.table/sort-asc :asc]
   [:view.table/sort-desc :desc]])
```

**Pass 2 — Key Collection**

The AST is walked recursively to find translation keys in these patterns:

| Pattern | Example | Collected Keys |
|---------|---------|----------------|
| Direct call | `(t :ui/save)` | `:ui/save` |
| Qualified call | `(i18n/t :nav/home)` | `:nav/home` |
| Symbol resolution | `(t my-key)` | keys from symbol table |
| Conditional | `(t (if x :a :b))` | `:a`, `:b` |
| Or fallback | `(t (or val :default))` | `:default` |
| Cond/case | `(t (cond ... :a ... :b))` | `:a`, `:b` |
| Map payload | `{:i18n-key :dialog/ok}` | `:dialog/ok` |
| Map conditional | `{:i18n-key (if x :a :b)}` | `:a`, `:b` |
| Let-bound symbol | `(let [k :a] {:i18n-key k})` | `:a` |
| Alert function | `(notification/show! :notify/saved ...)` | `:notify/saved` |
| UI function | `(ui/button :label/submit ...)` | `:label/submit` |

Map payload detection uses attributes from `translation_key_attributes` (e.g. `:i18n-key`, `:prompt-key`, `:title-key`) combined with `ui_attributes`.

**Let-Scope Tracking**

The collector tracks `let`, `when-let`, `if-let`, and `loop` binding scopes.
When a map attribute's value is a symbol, the collector resolves it through the
current let scope stack before falling back to the top-level symbol table:

```clojure
;; Both keys are detected even though i18n-key is a symbol in the map
(let [i18n-key (if condition
                 :page.convert/property-value-to-page
                 :page.convert/block-parent-not-page)]
  (throw (ex-info msg {:payload {:i18n-key i18n-key}})))
```

### 3. Collect DB-Ident Derived Keys

Entries in `[[check-keys.db_ident_defs]]` are parsed to extract keywords in the
`logseq.property.*`, `logseq.class/*`, and `block/*` namespaces. Each entry
scopes extraction to a specific named `def` or `defonce` form within its file,
avoiding false positives from unrelated keyword literals elsewhere in the file.
The extracted keywords are converted to their corresponding i18n keys:

| DB-Ident Keyword | I18n Key |
|------------------|----------|
| `:logseq.class/Task` | `:class.built-in/task` |
| `:logseq.property/status` | `:property.built-in/status` |
| `:logseq.property/hide?` | `:property.built-in/hide` |
| `:logseq.property/status.doing` | `:property.status/doing` |
| `:logseq.property.asset/type` | `:property.built-in/asset-type` |
| `:logseq.property.view/type.gallery` | `:property.view-type/gallery` |
| `:block/alias` | `:property.built-in/alias` |

These derived keys are added to the referenced set, so built-in property/class
keys are not falsely reported as unused.

### 4. Filter Results

Two config-driven filters reduce noise:

- **`always_used_key_patterns`** — Regex patterns matching keys that are always
  considered used. Use this for dynamically generated keys that cannot be
  detected by static analysis (e.g. keys assembled via `(keyword "ns" name)`).

- **`ignore_key_namespaces`** — Namespace prefixes whose keys are excluded from
  checking entirely. Matching uses prefix logic: `"deprecated"` also matches
  `"deprecated.old"`.

### 5. Report or Fix

Unused keys are printed in a table. With `--fix`, they are removed from all
`.edn` files in `dicts_dir`.

## Configuration Reference

Check-keys settings live in the `[check-keys]` section of the TOML config file.
Scanner settings shared with the `lint` subcommand are at the top level.

```toml
# ── Shared settings ───────────────────────────────────────────────────────────

# Translation functions — keys from their call sites are collected by both subcommands.
i18n_functions = ["t", "tt", "i18n/t"]

# Alert/notification functions — first keyword arg is a translation key reference.
alert_functions = ["notification/show!"]

# UI component functions — keyword args are translation key references.
ui_functions = ["ui/button"]

# Namespace prefixes where every function is treated as a UI component.
ui_namespaces = ["shui"]

# Hiccup/map attributes whose keyword values are translation key references.
ui_attributes = ["placeholder", "title", "aria-label"]

# ── [check-keys] section ──────────────────────────────────────────────────────

[check-keys]

# Files to skip during check-keys scanning.
# NOTE: a file can appear in [lint].exclude_patterns but NOT here, so its
# translation key references are still detected.
exclude_patterns = [
  "**/test/**",
  "**/node_modules/**",
]

# Directory containing dictionary EDN files (relative to project_root).
dicts_dir = "src/resources/dicts"

# Primary dictionary file whose keys define the translation key set.
primary_dict = "src/resources/dicts/en.edn"

# Regex patterns for keys always considered "used".
always_used_key_patterns = [
  "^:command\\.",          # dynamically assembled shortcut keys
  "^:color/",             # keys from built-in-colors vector
  "^:view\\.table/group-", # keys used in (for [...] (t k)) loops
]

# Namespace prefixes to exclude from checking.
ignore_key_namespaces = [
  "deprecated.config",
]

# Map attribute keys whose keyword values are translation key references.
# Combined with ui_attributes during analysis.
# Default: ["i18n-key", "prompt-key", "title-key"]
translation_key_attributes = ["i18n-key", "prompt-key", "title-key"]

# Built-in db-ident definition sources (relative to project_root).
# Each entry scopes extraction to the named def/defonce form in the file.
[[check-keys.db_ident_defs]]
file = "deps/db/src/logseq/db/frontend/property.cljs"
def  = "built-in-properties"

[[check-keys.db_ident_defs]]
file = "deps/db/src/logseq/db/frontend/class.cljs"
def  = "built-in-classes"
```

## Scenarios Not Covered by Static Analysis

Four categories of key usage require additional configuration:

### 1. Dynamic Key Construction

Keys assembled at runtime via `(keyword "namespace" dynamic-name)`:

```clojure
(keyword "command" (name action))  ; → :command/copy, :command/paste, etc.
```

**Solution:** Add a pattern to `always_used_key_patterns`:
```toml
always_used_key_patterns = ["^:command\\."]
```

### 2. Iterated Key References

Keys from a collection used via destructured bindings in loop constructs:

```clojure
(for [[option-key _] sort-options]
  (t option-key))  ; option-key is a symbol, not a keyword literal
```

The symbol table resolves `(t sort-options)` but not `(t option-key)` inside
a `for` destructuring. (Note: `let`-bound symbols ARE resolved — see
[Let-Scope Tracking](#let-scope-tracking) above.)

**Solution:** Add specific patterns to `always_used_key_patterns`:
```toml
always_used_key_patterns = ["^:view\\.table/group-"]
```

### 3. Keys Referenced Only in Excluded Files

If a file is excluded via `[lint].exclude_patterns` but is the only place that
references a particular translation key, the key will appear as unused.

The `[lint]` and `[check-keys]` exclude lists are separate. A file can be
suppressed from lint scanning while still being scanned for key references.

```toml
[lint]
# Excluded from hardcoded-string detection (developer tool, not user-facing)
exclude_patterns = ["**/profiler.cljs"]

[check-keys]
# NOT in this list, so profiler.cljs is scanned for translation key references
exclude_patterns = []
```

### 4. Built-in DB-Ident Derived Keys

Property and class keys derived from database identifier definitions. These
are handled automatically via `[[check-keys.db_ident_defs]]` configuration.

## CLI Usage

```bash
# Check for unused keys
logseq-i18n-lint check-keys

# Check and remove unused keys from all dictionaries
logseq-i18n-lint check-keys --fix

# With custom config
logseq-i18n-lint -c config/logseq.toml check-keys

# Verbose output (shows defined/referenced counts)
logseq-i18n-lint -v check-keys
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | All keys are used |
| 1 | Unused keys found |
| 2 | Configuration or runtime error |
