# Detection Rules — Design & Limitations

This document explains how `logseq-i18n-lint` decides which strings to report,
what it intentionally skips, and known limitations with workarounds.

## Core Principle: Context-Aware Detection

The analyzer walks the AST with a **context stack** that tracks enclosing forms.
A string is only reported when it appears in a position where it would be rendered
as user-visible text. This avoids the flood of false positives that simpler
regex-based tools produce.

Two key context checks recur throughout the rules:

- **UI context** — the string is inside a hiccup vector (`[:div ...]`) or a
  recognized UI function call (`ui/button`, `shui/alert-title`, etc.)
- **Ignore context** — the string is inside a logging call, regex, `ns`/`require`,
  `comment`, `ex-info`, `throw`, or any function listed in `ignore_context_functions`

## Detection Categories

### 1. `hiccup-text`

Detects string literals that appear as **child nodes** of hiccup vectors.

```clojure
;; Reported
[:div "Hello world"]
[:span {:class "text-sm"} "Click here"]

;; NOT reported — attribute value, not a child node
[:input {:placeholder "Search..."}]

;; NOT reported — single char or empty
[:div " "]
[:div ""]
```

**How it works:** When the analyzer encounters a vector starting with a keyword
(`:div`, `:span`, etc.), it enters Hiccup context. String children after the
optional attribute map are reported as `hiccup-text`.

### 2. `hiccup-attr`

Detects strings in hiccup attribute maps for **UI-facing attributes** only.

```clojure
;; Reported — placeholder is a UI attribute
[:input {:placeholder "Search pages..."}]

;; NOT reported — class is not a UI attribute
[:div {:class "flex items-center"}]
```

**Configured via:** `ui_attributes` in the config file. Default:
`placeholder`, `title`, `aria-label`, `alt`, `label`.

### 3. `fn-arg-text`

Detects string arguments passed to **recognized UI functions**.

```clojure
;; Reported
(ui/button "Submit")
(shui/alert-title "Warning")

;; NOT reported — not a UI function
(js/console.log "debug info")
```

**Configured via:** `ui_functions` and `ui_namespaces`. Any function whose
namespace appears in `ui_namespaces` (e.g. `"shui"`) is treated as a UI function.

### 4. `str-concat` (Requires UI Context)

Detects string literals inside `(str ...)` calls, but **only when the `str` call
is inside a hiccup vector or UI function call**.

```clojure
;; Reported — inside hiccup
[:div (str "Hello " name)]

;; NOT reported — not in UI context
(defn make-key [x] (str "key-" x))
```

**Rationale:** `(str ...)` is used pervasively for non-UI purposes (building keys,
paths, queries). Only those inside UI rendering contexts are likely to need i18n.

### 5. `conditional-text` (Requires UI Context)

Detects string literals in `if`/`when`/`case`/`cond`/`condp` branch positions,
but **only when the conditional is inside a UI context**.

```clojure
;; Reported — inside hiccup
[:span (if loading? "Loading..." "Ready")]

;; NOT reported — not in UI context
(if mac? "⌘+V" "Ctrl+V")       ; keyboard shortcut mapping
(throw (ex-info (if x "A" "B") {}))  ; error message
```

**Rationale:** Conditionals appear everywhere — error handling, data transformation,
keyboard shortcut mapping, CSS class toggling. Only those producing strings for UI
rendering need i18n.

### 6. `format-string` (Requires UI Context)

Detects the format string (first argument) of functions listed in `format_functions`,
but **only when the call is inside a hiccup vector or UI function call**.

```clojure
;; Reported — inside hiccup
[:div (goog.string/format "Found %d items" count)]

;; NOT reported — not in UI context
(defn make-label [n] (goog.string/format "Item %d" n))

;; NOT reported — template is a translation call
[:div (format (t :items-count) count)]
```

**Configured via:** `format_functions`. Default: `["format", "goog.string/format"]`.

### 7. `let-text` (Requires UI Context)

Detects string values in `let` bindings, but **only when the `let` is inside a
hiccup vector or UI function call**.

```clojure
;; Reported — inside UI function
(shui/button {} (let [label "Click me"] label))

;; NOT reported — not in UI context
(let [path "~/.logseq/config.edn"] (read-file path))
```

### 8. `alert-text`

Detects the first string argument to **alert/notification functions**.

```clojure
;; Reported
(notification/show! "File saved" :success)

;; NOT reported — uses translation
(notification/show! (t :file-saved) :success)
```

**Configured via:** `alert_functions`. Default: `["notification/show!"]`.

## Automatic Skip Rules

Before any context check, strings are filtered by `should_skip_string`:

| Rule | Example Skipped |
|------|-----------------|
| Empty or single character | `""`, `" "`, `"x"` |
| Pure numeric | `"42"`, `"3.14"`, `"-1"` |
| No alphabetic chars (after trim) | `" · "`, `"$10"`, `"< 0.01"`, `"⌘+V"`, `"🎉 "`, `" →"` |
| Whitespace-only (2+ chars) | `"  "`, `"\n  "` |
| Exact match in `allow_strings` | `"Logseq"`, `"Contents"` |
| Regex match in `allow_patterns` | URLs, CSS classes, hex colors, LaTeX commands |

The **no-alphabetic-chars** rule is the most impactful automatic filter. It skips
any string that, after trimming whitespace, contains zero Unicode alphabetic characters.
This eliminates emoji decorations, mathematical symbols, currency + digits,
keyboard shortcut symbols, and punctuation separators — none of which are
translatable natural language.

## Ignored Contexts

The following contexts suppress string detection for all nested content:

| Context | Why | Config key |
|---------|-----|------------|
| `(ns ...)`, `(require ...)` | Namespace declarations | — |
| `(comment ...)` | Developer comments | — |
| Translation function calls | Already translated | `i18n_functions` |
| Error constructor calls | Developer-facing messages | `exception_functions` |
| Logging, regex, and non-UI utilities | Not user-visible output | `ignore_context_functions` |

### `fn` / `fn*` Scope Barrier

Anonymous functions (`fn`, `fn*`) create a **scope barrier** that prevents the
surrounding hiccup/UI context from leaking into the lambda body.

```clojure
;; "Enter" is NOT reported — fn scope barrier
[:div {:on-key-down (fn [e] (when (= (.-key e) "Enter") ...))}
  "Press enter"]  ;; ← this IS reported (direct hiccup child)
```

**Rationale:** Event handler lambdas compare against key names, DOM properties,
and other internal values. These are not UI text even though they're syntactically
inside a hiccup vector.

### UI Function Keyword-Argument Values

When a UI function is called with keyword-argument pairs, values for keywords not
listed in `ui_attributes` are analyzed under a FnScope barrier:

```clojure
(ui/button "Submit"                   ; reported as fn-arg-text
  :class (str "btn " active-class)    ; NOT reported — :class not in ui_attributes
  :target "_blank"                    ; NOT reported — :target not in ui_attributes
  :aria-label "Close dialog")         ; reported as fn-arg-text — in ui_attributes
```

CSS class fragments, link targets, element IDs, and other non-text keyword-arg
values are suppressed automatically without any `allow_strings` entries.

## `def` / `defonce` Handling

Plain string values in `def`/`defonce` are **not reported**:

```clojure
;; NOT reported — data constant
(def page-name "Library")
(defonce quick-add "Quick add")
```

**Rationale:** Without data-flow analysis, we cannot know whether the bound symbol
is used in UI rendering or in internal logic. Reporting all `def` strings would
produce hundreds of false positives for page identifiers, config keys, and internal
constants. If the string is actually rendered, it will be caught when used inside a
hiccup vector or UI function.

## Known Limitations

### 1. Dynamic string usage (`def` → hiccup)

```clojure
(def error-fallback "Something went wrong")

;; Later in another file:
[:div error-fallback]  ;; ← the string isn't visible here
```

The string at the `def` site is not reported. The usage at the hiccup site references
a symbol, which the analyzer cannot resolve without cross-file data-flow tracking.

**Mitigation:** Not currently solvable without cross-file data-flow analysis.
Use translation calls at the definition site rather than binding raw strings.

### 2. `str` used for non-UI purposes inside UI context

```clojure
;; The :class str call is correctly skipped (attribute value).
;; But if str appears as a direct child, it's reported:
[:div (str "Count: " n)]  ;; Reported ✓

;; Edge case: str building a value for JS interop inside hiccup
[:div {:on-click #(js/alert (str "ID: " id))}]  ;; NOT reported (fn barrier)
```

The `fn` scope barrier handles most of these cases.

### 3. Macro-expanded code

The analyzer works on surface syntax. Macros that expand into hiccup or UI function
calls at compile time are invisible:

```clojure
(my-custom-macro "This might render as UI text")  ;; Not detected
```

**Workaround:** Add the macro to `ui_functions` in the config if it produces UI output.

## Configuration Tips

### When to use `allow_strings`

- Internal identifiers that look like words but are not translatable: `"Contents"`, `"Label"`, `"Cancelled"`
- Syntax tokens or paths displayed verbatim: `":END:"`, `"~/.logseq"`
- Brand names that are not translated in any locale
- Note: CSS class fragments in `:class` keyword-arg values and leading/trailing whitespace
  variants are handled automatically — whitespace is stripped before pattern matching, so
  `"active "` is matched as `"active"` by the single-token CSS class pattern.

### When to use `allow_patterns`

- URL schemes: `^https?://`
- CSS class patterns: `^[-!]?[a-z][a-z0-9!/:_\[\].%+*~-]*$` (single token)
- Format/template strings: `^[^A-Za-z ]*%`
- LaTeX commands: `^\\\\`

### When to use `pure_functions`

- AST dispatch functions where string args are node type names: `markup-element-cp`, `inline`
- String utility functions whose args are data substrings: `text-util/cut-by`
- Any non-UI function called inside hiccup that produces data, not display text

### When to use `exclude_patterns`

- Test and dev files: `**/test/**`
- Generated code: `**/target/**`
- Files that are purely data (no UI): keyboard shortcut config, device model lists, Malli schemas

### When to use `ignore_context_functions`

- Logging/debugging: `js/console.log`, `log/debug`
- CSS utilities: `shui/cn`, `util/classnames`
- Dialog/popup API calls that take IDs, not text: `shui/popup-show!`

---

## Best Practices

### Avoiding False Negatives (missed detections)

False negatives occur when a hardcoded UI string is not detected.

**Common causes and remedies:**

1. **UI function not in config.** If a function renders text to the user but isn't
   listed in `ui_functions` or `ui_namespaces`, its string args are missed.
   *Fix:* Add it to `ui_functions`, or its namespace prefix to `ui_namespaces`.

2. **String stored in `def`, used in UI.** The analyzer cannot trace data flow across
   bindings. A `(def label "Click me")` followed by `[:button label]` will not be
   caught at the def site.
   *Mitigation:* Code review discipline — use `(t :key)` at the point of definition.

3. **String inside a macro.** Macros that expand to hiccup are invisible to the
   surface-syntax analyzer.
   *Fix:* Add the macro to `ui_functions` to flag its positional string args.

4. **Excluded file.** If the file is in `exclude_patterns`, no strings inside it
   are ever checked.
   *Fix:* Review `exclude_patterns` — exclude only genuinely non-UI files (tests,
   generated code, pure-data schemas).

5. **Bare string in a lambda body.** `(fn ...)` creates a scope barrier that resets
   the surrounding UI context. A string that is a direct child of the lambda (not
   wrapped in its own hiccup vector) will not be detected. Nested hiccup vectors
   inside the lambda body **are** analyzed normally.
   *Mitigation:* This is intentional — most lambda bodies contain event handlers and
   comparisons, not UI text. Ensure translated wrappers are used at the call site.

### Avoiding False Positives (incorrect detections)

False positives occur when a non-translatable string is incorrectly flagged.

**Common causes and remedies:**

1. **Data string passed to a UI function.** A function used in a UI context may take
   data arguments (e.g. AST type names, CSS classes, element IDs).
   *Fix:* Add the function to `pure_functions` if all its string args are data, or
   to `ignore_context_functions` if it has no UI output.

2. **Predicate/comparison expression in UI context.** `(= query "(and)")` inside a
   hiccup branch — the string is not rendered, it's compared.
   *Fix:* These are handled automatically for common predicates (`=`, `not=`,
   `contains?`, etc. and any function ending in `?`). For project-specific
   comparison functions, add them to `pure_functions`.

3. **Schema file with Malli/spec literals.** `[:= "TypeName"]` uses string literals
   as schema validators, not UI text.
   *Fix:* Exclude schema files via `exclude_patterns`.

4. **Syntax tokens or paths displayed verbatim.** Strings like `":END:"`,
   `":results"`, or `"~/.logseq"` are not translatable (markup syntax, filesystem paths).
   *Fix:* Add them to `allow_strings`.

5. **Brand or product names that should not be translated.** Some names are identical
   in all locales.
   *Fix:* Add to `allow_strings`; surrounding whitespace is stripped before comparison,
   so `"Logseq "` is covered by a `"Logseq"` entry.

6. **`match` pattern arms.** `(match item ["TypeName" ...] ...)` — the pattern vector
   `["TypeName" ...]` is not UI text.
   *Fix:* Handled automatically. The `match` form handler applies FnScope to all
   pattern positions.
