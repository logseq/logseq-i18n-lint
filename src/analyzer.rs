use std::fmt;
use std::path::{Path, PathBuf};

use regex::RegexSet;

use crate::config::AppConfig;
use crate::parser::{self, SExp, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiagnosticKind {
    HiccupText,
    HiccupAttr,
    AlertText,
    StrConcat,
    FormatString,
    ConditionalText,
    FnArgText,
    #[allow(dead_code)]
    DefText,
    LetText,
}

impl fmt::Display for DiagnosticKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HiccupText => write!(f, "hiccup-text"),
            Self::HiccupAttr => write!(f, "hiccup-attr"),
            Self::AlertText => write!(f, "alert-text"),
            Self::StrConcat => write!(f, "str-concat"),
            Self::FormatString => write!(f, "format-string"),
            Self::ConditionalText => write!(f, "conditional-text"),
            Self::FnArgText => write!(f, "fn-arg-text"),
            Self::DefText => write!(f, "def-text"),
            Self::LetText => write!(f, "let-text"),
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub file_path: PathBuf,
    pub line: u32,
    pub col: u32,
    pub text: String,
    pub context: Option<String>,
}

struct AnalysisContext {
    file_path: PathBuf,
    allow_strings: Vec<String>,
    allow_patterns: RegexSet,
    ui_functions: Vec<String>,
    ui_namespaces: Vec<String>,
    ui_attributes: Vec<String>,
    ignore_context_functions: Vec<String>,
    i18n_functions: Vec<String>,
    exception_functions: Vec<String>,
    alert_functions: Vec<String>,
    pure_functions: Vec<String>,
    format_functions: Vec<String>,
    diagnostics: Vec<Diagnostic>,
}

impl AnalysisContext {
    fn new(file_path: PathBuf, config: &AppConfig) -> Self {
        let allow_patterns = RegexSet::new(&config.lint.allow_patterns).unwrap_or_else(|e| {
            eprintln!("warning: invalid allow_patterns regex: {e}");
            RegexSet::empty()
        });
        Self {
            file_path,
            allow_strings: config.lint.allow_strings.clone(),
            allow_patterns,
            ui_functions: config.ui_functions.clone(),
            ui_namespaces: config.ui_namespaces.clone(),
            ui_attributes: config.ui_attributes.clone(),
            ignore_context_functions: config.lint.ignore_context_functions.clone(),
            i18n_functions: config.i18n_functions.clone(),
            exception_functions: config.lint.exception_functions.clone(),
            alert_functions: config.alert_functions.clone(),
            pure_functions: config.lint.pure_functions.clone(),
            format_functions: config.lint.format_functions.clone(),
            diagnostics: Vec::new(),
        }
    }

    fn should_skip_string(&self, s: &str) -> bool {
        // Empty or single char
        if s.is_empty() || s.chars().count() <= 1 {
            return true;
        }

        // Pure numeric
        if s.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-' || c == '+') {
            return true;
        }

        // No alphabetic characters → pure symbols/emoji/digits or whitespace-only.
        if !s.chars().any(char::is_alphabetic) {
            return true;
        }

        // Exact match allow list.
        if self.allow_strings.iter().any(|a| a == s) {
            return true;
        }

        // Regex pattern match.
        if self.allow_patterns.is_match(s) {
            return true;
        }

        false
    }

    fn report(&mut self, kind: DiagnosticKind, span: Span, text: &str, context: Option<String>) {
        if self.should_skip_string(text.trim()) {
            return;
        }
        self.diagnostics.push(Diagnostic {
            kind,
            file_path: self.file_path.clone(),
            line: span.line,
            col: span.col,
            text: text.trim().to_string(),
            context,
        });
    }

    fn is_ui_function(&self, name: &str) -> bool {
        // Exact name match
        if self.ui_functions.iter().any(|f| {
            f == name || name.ends_with(&format!("/{}", f.rsplit('/').next().unwrap_or(f)))
        }) {
            return true;
        }
        // Namespace prefix match (e.g. "shui" matches "shui/button", "shui/alert-description")
        if let Some(ns) = name.split('/').next() {
            return self.ui_namespaces.iter().any(|n| n == ns);
        }
        false
    }

    fn is_ui_attribute(&self, name: &str) -> bool {
        // Strip leading colon for keyword comparison
        let clean = name.strip_prefix(':').unwrap_or(name);
        self.ui_attributes.iter().any(|a| a == clean)
    }

    fn is_ignore_context(&self, name: &str) -> bool {
        self.ignore_context_functions.iter().any(|f| {
            f == name || name.ends_with(&format!("/{}", f.rsplit('/').next().unwrap_or(f)))
        })
    }

    fn is_i18n_function(&self, name: &str) -> bool {
        self.i18n_functions.iter().any(|f| {
            f == name || name.ends_with(&format!("/{}", f.rsplit('/').next().unwrap_or(f)))
        })
    }

    fn is_exception_function(&self, name: &str) -> bool {
        self.exception_functions.iter().any(|f| {
            f == name || name.ends_with(&format!("/{}", f.rsplit('/').next().unwrap_or(f)))
        })
    }

    fn is_alert_function(&self, name: &str) -> bool {
        self.alert_functions.iter().any(|f| {
            f == name || name.ends_with(&format!("/{}", f.rsplit('/').next().unwrap_or(f)))
        })
    }

    fn is_pure_function(&self, name: &str) -> bool {
        // Hardcoded common comparison and predicate functions.
        match name {
            "=" | "not=" | "==" | "identical?" | "<" | ">" | "<=" | ">=" | "compare" | "not" => {
                return true;
            }
            _ => {}
        }
        // Any function name ending with ? is assumed to return boolean.
        if name.ends_with('?') {
            return true;
        }
        // Common clojure.string functions where all args are data (not UI text).
        let base = name.rsplit('/').next().unwrap_or(name);
        if matches!(
            base,
            "starts-with?" | "ends-with?" | "includes?" | "index-of"
                | "last-index-of" | "blank?" | "split"
        ) {
            return true;
        }
        // User-configured additional pure functions.
        self.pure_functions.iter().any(|f| {
            f == name || name.ends_with(&format!("/{}", f.rsplit('/').next().unwrap_or(f)))
        })
    }

    fn is_format_function(&self, name: &str) -> bool {
        self.format_functions.iter().any(|f| {
            f == name || name.ends_with(&format!("/{}", f.rsplit('/').next().unwrap_or(f)))
        })
    }
}

/// Analyze all files and return diagnostics.
#[must_use]
pub fn analyze_files(files: &[PathBuf], config: &AppConfig) -> Vec<Diagnostic> {
    use rayon::prelude::*;

    files
        .par_iter()
        .flat_map(|file| analyze_single_file(file, config))
        .collect()
}

fn analyze_single_file(path: &Path, config: &AppConfig) -> Vec<Diagnostic> {
    let forms = match parser::parse_file(&path.to_path_buf(), config) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("warning: failed to parse {}: {e}", path.display());
            return Vec::new();
        }
    };

    analyze_source_with_config(&forms, path, config)
}

/// Analyze pre-parsed AST forms. Used by benchmarks and tests.
#[must_use]
pub fn analyze_source_with_config(forms: &[SExp], path: &Path, config: &AppConfig) -> Vec<Diagnostic> {
    let mut ctx = AnalysisContext::new(path.to_path_buf(), config);

    for form in forms {
        analyze_form(&mut ctx, form, &ContextStack::Empty);
    }

    ctx.diagnostics
}

/// Context stack tracking what enclosing forms look like.
#[derive(Clone)]
enum ContextStack<'a> {
    Empty,
    Frame {
        kind: FrameKind,
        parent: &'a ContextStack<'a>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FrameKind {
    Hiccup,
    UiFnCall,
    StrCall,
    IgnoreContext,
    NsOrRequire,
    Comment,
    Def,
    Let,
    Conditional,
    /// Marks the boundary of an inline `(fn ...)` / `(fn* ...)` expression.
    /// When present, `has(Hiccup)` and `has(UiFnCall)` stop propagating
    /// so that strings inside event-handler lambdas (e.g. `(fn [e] (= (.-key e) "Enter"))`)
    /// are not mistakenly reported as UI text.
    FnScope,
}

impl<'a> ContextStack<'a> {
    fn push(&'a self, kind: FrameKind) -> ContextStack<'a> {
        ContextStack::Frame { kind, parent: self }
    }

    fn has(&self, target: FrameKind) -> bool {
        match self {
            Self::Empty => false,
            Self::Frame { kind, parent } => {
                if *kind == target {
                    return true;
                }
                // FnScope acts as a barrier: strings inside `(fn ...)` bodies
                // should not inherit the surrounding Hiccup / UiFnCall context.
                // Event-handler strings like (= (.-key e) "Enter") are not UI text.
                if matches!(kind, FrameKind::FnScope)
                    && matches!(target, FrameKind::Hiccup | FrameKind::UiFnCall)
                {
                    return false;
                }
                parent.has(target)
            }
        }
    }

    fn in_ignore_context(&self) -> bool {
        self.has(FrameKind::IgnoreContext) || self.has(FrameKind::NsOrRequire) || self.has(FrameKind::Comment)
    }
}

fn analyze_form(ctx: &mut AnalysisContext, form: &SExp, stack: &ContextStack<'_>) {
    if stack.in_ignore_context() {
        return;
    }

    match form {
        SExp::Discard(_, _) => (),

        SExp::List(items, _span) => {
            analyze_list_form(ctx, items, stack);
        }

        SExp::Vector(items, _span) => {
            // Check if this is a hiccup vector: starts with keyword
            if is_hiccup_vector(items) {
                let hiccup_stack = stack.push(FrameKind::Hiccup);
                analyze_hiccup_vector(ctx, items, &hiccup_stack);
            } else {
                for item in items {
                    analyze_form(ctx, item, stack);
                }
            }
        }

        SExp::Map(items, _) => {
            analyze_map(ctx, items, stack);
        }

        SExp::Meta(_, target, _) => {
            analyze_form(ctx, target, stack);
        }

        // String literals are only reported as hiccup-text when they appear as
        // direct children of a hiccup vector (handled by analyze_hiccup_vector).
        // All other detection rules (str-concat, conditional-text, fn-arg-text,
        // alert-text, format-string, let-text, hiccup-attr) have dedicated
        // handlers in analyze_list_form.  Reporting bare strings here based on
        // context alone would flag CSS classes, component IDs, and other
        // non-UI data passed through generic functions inside hiccup vectors.

        _ => {}
    }
}

#[allow(clippy::too_many_lines)]
fn analyze_list_form(ctx: &mut AnalysisContext, items: &[SExp], stack: &ContextStack<'_>) {
    if items.is_empty() {
        return;
    }

    let SExp::Symbol(name, _) = &items[0] else {
        // Not a symbol-headed list, recurse into all items
        for item in items {
            analyze_form(ctx, item, stack);
        }
        return;
    };
    let head_name = name.as_str();

    // Check ignore context functions
    if ctx.is_ignore_context(head_name) {
        return;
    }

    match head_name {
        // Translation function calls — skip entirely.
        name if ctx.is_i18n_function(name) => (),

        // Namespace declarations.
        "ns" | "require" | "import" | "use" | "refer" | "comment" => (),

        // def / defonce / defn / defmacro (standard forms)
        "def" | "defonce" | "defn" | "defn-" | "defmacro" | "defmethod" | "defmulti" => {
            analyze_def_form(ctx, items, stack);
        }

        // rum/defc, rum/defcs, rum/defcc, and similar def-like component macros
        name if {
            let base = name.rsplit('/').next().unwrap_or(name);
            matches!(base, "defc" | "defcs" | "defcc" | "defrc" | "defsc")
        } => {
            analyze_def_form(ctx, items, stack);
        }

        // let / let! / binding / loop
        "let" | "let!" | "binding" | "loop" | "when-let" | "when-some" | "if-let" | "if-some" => {
            analyze_let_form(ctx, items, stack);
        }

        // str - string concatenation
        "str" => {
            let str_stack = stack.push(FrameKind::StrCall);
            for item in &items[1..] {
                if let SExp::Str(s, span) = item {
                    if stack.has(FrameKind::Hiccup) || stack.has(FrameKind::UiFnCall) {
                        ctx.report(DiagnosticKind::StrConcat, *span, s, Some("str".to_string()));
                    }
                } else {
                    analyze_form(ctx, item, &str_stack);
                }
            }
        }

        // Format/printf functions — first arg is the template string.
        // Only flagged when the call site is inside a UI context (hiccup or UI fn call).
        // Configured via `format_functions`; calls outside UI rendering are data ops.
        name if ctx.is_format_function(name) => {
            match items.get(1) {
                Some(SExp::Str(s, span)) => {
                    if stack.has(FrameKind::Hiccup) || stack.has(FrameKind::UiFnCall) {
                        ctx.report(DiagnosticKind::FormatString, *span, s, Some(head_name.to_string()));
                    }
                    // Else: not in UI context; nothing to report and no sub-forms to recurse into.
                }
                Some(other) => {
                    // Template is a complex expression — recurse so nested UI forms are caught.
                    analyze_form(ctx, other, stack);
                }
                None => {}
            }
            for item in items.iter().skip(2) {
                analyze_form(ctx, item, stack);
            }
        }

        // Alert/notification functions — first argument is user-visible text.
        // Push UiFnCall so nested str/conditional/format calls are caught by their
        // own rules.  Remaining arguments (status codes, flags) are skipped.
        name if ctx.is_alert_function(name) => {
            let ui_stack = stack.push(FrameKind::UiFnCall);
            match items.get(1) {
                Some(SExp::Str(s, span)) => {
                    ctx.report(DiagnosticKind::AlertText, *span, s, Some(name.to_string()));
                }
                Some(other) => {
                    analyze_form(ctx, other, &ui_stack);
                }
                None => {}
            }
        }

        // Exception/error constructors — arguments are developer-facing, not UI text.
        name if ctx.is_exception_function(name) => (),

        // Conditional forms
        "if" | "if-not" | "when" | "when-not" | "cond" | "condp" | "case" | "or" => {
            analyze_conditional_form(ctx, items, head_name, stack);
        }

        // clojure.core.match/match — treat like case with FnScope for patterns.
        "match" => {
            analyze_match_form(ctx, items, stack);
        }

        // Thread-when conditionals: (cond-> expr test1 form1 test2 form2 ...)
        // expr:    initial threading value — data, not UI text.
        // testN:   predicate expressions  — not UI text.
        // formN:   partial function applications; recurse in current context so that
        //          any nested UI calls (alert, UI functions, str-concat, …) are
        //          caught by their own rules.
        "cond->" | "cond->>" => {
            let fn_stack = stack.push(FrameKind::FnScope);
            // i=0 → expr; i=1,3,5,… → testN; i=2,4,6,… → formN
            for (i, item) in items.iter().skip(1).enumerate() {
                if i == 0 || i % 2 == 1 {
                    // expr and test positions: push FnScope — not UI text
                    analyze_form(ctx, item, &fn_stack);
                } else {
                    // form positions: keep current context
                    analyze_form(ctx, item, stack);
                }
            }
        }

        // do form
        "do" => {
            for item in &items[1..] {
                analyze_form(ctx, item, stack);
            }
        }

        // fn / fn* — push FnScope so the lambda body does not inherit outer UI context.
        "fn" | "fn*" => {
            let fn_stack = stack.push(FrameKind::FnScope);
            analyze_fn_body(ctx, items, &fn_stack);
        }

        // Pure (non-UI) functions — push FnScope so string args are not reported as UI text.
        name if ctx.is_pure_function(name) => {
            let pure_stack = stack.push(FrameKind::FnScope);
            for item in &items[1..] {
                analyze_form(ctx, item, &pure_stack);
            }
        }

        // UI function call — detect keyword-arg pairs for non-UI attributes.
        name if ctx.is_ui_function(name) => {
            let ui_stack = stack.push(FrameKind::UiFnCall);
            let args = &items[1..];
            let mut i = 0;
            while i < args.len() {
                match &args[i] {
                    SExp::Keyword(k, _) if i + 1 < args.len() => {
                        let key = k.strip_prefix(':').unwrap_or(k.as_str());
                        let val = &args[i + 1];
                        if ctx.is_ui_attribute(key) {
                            if let SExp::Str(s, span) = val {
                                ctx.report(
                                    DiagnosticKind::FnArgText,
                                    *span,
                                    s,
                                    Some(key.to_string()),
                                );
                            } else {
                                analyze_form(ctx, val, &ui_stack);
                            }
                        } else {
                            // Non-UI keyword arg (e.g. :class, :on-click, :href, :target) —
                            // push FnScope so strings inside this value are not treated as UI text.
                            let non_ui_stack = ui_stack.push(FrameKind::FnScope);
                            analyze_form(ctx, val, &non_ui_stack);
                        }
                        i += 2;
                    }
                    SExp::Str(s, span) => {
                        ctx.report(DiagnosticKind::FnArgText, *span, s, Some(name.to_string()));
                        i += 1;
                    }
                    SExp::Map(kv, _) => {
                        analyze_ui_fn_map_arg(ctx, kv, &ui_stack);
                        i += 1;
                    }
                    _ => {
                        analyze_form(ctx, &args[i], &ui_stack);
                        i += 1;
                    }
                }
            }
        }

        // Generic function call — recurse into arguments.
        _ => {
            for item in &items[1..] {
                analyze_form(ctx, item, stack);
            }
        }
    }
}

fn analyze_def_form(ctx: &mut AnalysisContext, items: &[SExp], stack: &ContextStack<'_>) {
    let head = match &items[0] {
        SExp::Symbol(s, _) => s.as_str(),
        _ => "",
    };
    let base = head.rsplit('/').next().unwrap_or(head);
    let def_stack = stack.push(FrameKind::Def);

    // --- fn-like forms: defn / defmacro / rum/defc etc. ---
    // Structure: (defn name ["docstring"] [args] body…)
    // Docstring is present when items[2] is a Str and items[3] is a Vector/List (argvec).
    let is_fn_like = matches!(
        base,
        "defn" | "defn-" | "defmacro" | "defc" | "defcs" | "defcc" | "defrc" | "defsc"
    );

    if is_fn_like {
        let body_start = if matches!(items.get(2), Some(SExp::Str(_, _)))
            && matches!(items.get(3), Some(SExp::Vector(_, _) | SExp::List(_, _)))
        {
            3 // skip docstring at [2], start at argvec [3]
        } else {
            2
        };
        // Recurse into the body — hiccup / UI calls inside will be caught by the
        // appropriate rules (HiccupText, FnArgText …). We do NOT emit DefText
        // directly; there is no reliable way to know whether a bare string literal
        // inside a function body is UI text without data-flow analysis.
        for item in items.iter().skip(body_start) {
            analyze_form(ctx, item, &def_stack);
        }
        return;
    }

    // --- Non-fn-like forms: def / defonce / defmulti / defmethod ---
    //
    // (def name value)                → items.len() == 3, items[2] is the value
    // (def name "docstring" value)    → items.len() >= 4, items[2] is a doc-string,
    //                                   items[3..] is the actual value expression
    //
    // Docstring detection: items[2] is Str AND items.len() >= 4 AND
    //                      items[3] is NOT a plain Str (it's a complex expression).
    let value_start = if items.len() >= 4
        && matches!(items.get(2), Some(SExp::Str(_, _)))
        && !matches!(items.get(3), Some(SExp::Str(_, _)))
    {
        3 // skip docstring, analyze from items[3]
    } else {
        2
    };

    for item in items.iter().skip(value_start) {
        match item {
            // Plain string at the binding level is a data value (config constant,
            // page-name identifier, etc.) — NOT directly reportable as UI text.
            // E.g.  (defonce page-name "Library")
            //        (def unused-note "is not used in DB graphs")
            // If the string is ever rendered directly in hiccup, it will appear
            // as a Str literal inside a vector and be caught as HiccupText.
            SExp::Str(_, _) => {}
            // Any other expression: recurse so nested hiccup / UI calls get picked up.
            _ => analyze_form(ctx, item, &def_stack),
        }
    }
}

fn analyze_let_form(ctx: &mut AnalysisContext, items: &[SExp], stack: &ContextStack<'_>) {
    let let_stack = stack.push(FrameKind::Let);

    // Determine the actual form head name (e.g. "let", "if-let", "when-let", …)
    // from items[0] so we can produce accurate diagnostic context labels and pick
    // the right DiagnosticKind for direct body string literals.
    let head_name = match items.first() {
        Some(SExp::Symbol(s, _)) => s.as_str(),
        _ => "let",
    };

    // Body string literals in let-like forms carry different semantics depending
    // on whether the form is a pure binding form or a conditional-binding form:
    //
    //   (let [label "Click me"] label)            → LetText         (binding result)
    //   (when-let [v (get)] "Fallback")           → ConditionalText (optional branch)
    //   (if-let [v (get)] v "Default text")       → ConditionalText (else branch)
    //
    // Pure binding forms: let, let!, binding, loop
    // Conditional forms:  when-let, when-some, if-let, if-some
    let is_conditional = matches!(head_name, "when-let" | "when-some" | "if-let" | "if-some");
    let body_kind = if is_conditional {
        DiagnosticKind::ConditionalText
    } else {
        DiagnosticKind::LetText
    };

    // (let [bindings…] body…)
    //
    // Strategy: binding VALUES that are plain string literals are only reported as
    // LetText when the let expression is already inside a UI context (hiccup vector
    // or a UI function call).  At the top level of a function body we cannot know
    // whether the bound symbol will be used for UI rendering, so we stay silent to
    // avoid the very common false-positive pattern:
    //
    //   (defonce quick-add-page-name "Quick add")  ;; page identifier, not UI text
    //   (let [url (get-path ...)] (navigate url))   ;; data variable, not UI text
    //
    // When the let IS already inside hiccup/UI, we do report:
    //   (shui/button {} (let [label "Click me"] label))  → LetText: "Click me"
    let in_ui_context = stack.has(FrameKind::Hiccup) || stack.has(FrameKind::UiFnCall);

    if let Some(SExp::Vector(bindings, _)) = items.get(1) {
        for pair in bindings.chunks(2) {
            if pair.len() == 2 {
                match &pair[1] {
                    SExp::Str(s, span) if in_ui_context => {
                        ctx.report(DiagnosticKind::LetText, *span, s, None);
                    }
                    SExp::Str(_, _) => {
                        // Not in UI context — data binding, skip without recursing.
                    }
                    other => analyze_form(ctx, other, &let_stack),
                }
            }
        }
    }

    // Body: always recurse; any hiccup / UI calls inside will be caught by their
    // own rules regardless of UI context.
    // If a body item is a direct string literal AND we are inside a UI context,
    // report it with the appropriate kind:
    //   - let/binding/loop body strings → LetText   (result of the binding block)
    //   - when-let/if-let body strings  → ConditionalText (optional/else branch)
    for item in items.iter().skip(2) {
        match item {
            SExp::Str(s, span) if in_ui_context => {
                ctx.report(body_kind, *span, s, Some(head_name.to_string()));
            }
            _ => analyze_form(ctx, item, &let_stack),
        }
    }
}

fn analyze_conditional_form(
    ctx: &mut AnalysisContext,
    items: &[SExp],
    head_name: &str,
    stack: &ContextStack<'_>,
) {
    let cond_stack = stack.push(FrameKind::Conditional);

    // Only report ConditionalText when the conditional is inside a UI context
    // (hiccup vector or UI function call).  Conditionals outside UI contexts
    // produce strings for error messages, internal logic, CSS classes, etc.
    let in_ui_context = stack.has(FrameKind::Hiccup) || stack.has(FrameKind::UiFnCall);

    match head_name {
        "if" | "if-not" | "when" | "when-not" => {
            for item in items.iter().skip(1) {
                match item {
                    SExp::Str(s, span) if in_ui_context => {
                        ctx.report(DiagnosticKind::ConditionalText, *span, s, Some(head_name.to_string()));
                    }
                    SExp::Str(_, _) => {}
                    _ => analyze_form(ctx, item, &cond_stack),
                }
            }
        }
        "or" => {
            for item in items.iter().skip(1) {
                match item {
                    SExp::Str(s, span) if in_ui_context => {
                        ctx.report(DiagnosticKind::ConditionalText, *span, s, Some("or".to_string()));
                    }
                    SExp::Str(_, _) => {}
                    _ => analyze_form(ctx, item, &cond_stack),
                }
            }
        }
        "case" => {
            // (case expr val1 result1 val2 result2 ... default?)
            // Results at indices 3, 5, 7... are potential UI text
            // Even last element (default) can be UI text
            if items.len() > 2 {
                // Skip head and expr
                analyze_form(ctx, &items[1], &cond_stack); // expr
                let case_items = &items[2..];
                // In case, odd-indexed items (0-based) are results
                for (i, item) in case_items.iter().enumerate() {
                    // If there's an even number of remaining items, they're all test/expr pairs
                    // If odd, the last one is a default
                    let is_result = if case_items.len().is_multiple_of(2) {
                        i % 2 == 1
                    } else if i == case_items.len() - 1 {
                        true // default value
                    } else {
                        i % 2 == 1
                    };
                    if is_result {
                        if let SExp::Str(s, span) = item {
                            if in_ui_context {
                                ctx.report(DiagnosticKind::ConditionalText, *span, s, Some("case".to_string()));
                            }
                        } else {
                            analyze_form(ctx, item, &cond_stack);
                        }
                    } else {
                        // Case test value — compile-time dispatch constant, never returned.
                        // Use FnScope to prevent hiccup/UI context from propagating into it,
                        // matching the same treatment as pattern positions in match forms.
                        let test_stack = stack.push(FrameKind::FnScope);
                        analyze_form(ctx, item, &test_stack);
                    }
                }
            }
        }
        "cond" | "condp" => {
            // (cond test1 expr1 test2 expr2 ...)
            let start = if head_name == "condp" { 3 } else { 1 };
            for (i, item) in items.iter().skip(start).enumerate() {
                if i % 2 == 1 {
                    // Result expression
                    if let SExp::Str(s, span) = item {
                        if in_ui_context {
                            ctx.report(
                                DiagnosticKind::ConditionalText,
                                *span,
                                s,
                                Some(head_name.to_string()),
                            );
                        }
                    } else {
                        analyze_form(ctx, item, &cond_stack);
                    }
                } else {
                    analyze_form(ctx, item, &cond_stack);
                }
            }
        }
        _ => {
            for item in items.iter().skip(1) {
                analyze_form(ctx, item, &cond_stack);
            }
        }
    }
}

fn analyze_fn_body(ctx: &mut AnalysisContext, items: &[SExp], stack: &ContextStack<'_>) {
    // Structure: fn [name?] [args] body...
    // Skip an optional leading Symbol (function name) and then exactly the first
    // Vector (the argument list).  All remaining forms — including vectors that
    // are hiccup expressions — must be analyzed.
    let mut saw_arg_vec = false;
    for item in items.iter().skip(1) {
        match item {
            // Optional function name — only valid before the argument vector.
            SExp::Symbol(_, _) if !saw_arg_vec => {}
            // Argument vector — skip exactly once.
            SExp::Vector(_, _) if !saw_arg_vec => {
                saw_arg_vec = true;
            }
            // Everything else is the function body (may include hiccup vectors).
            _ => analyze_form(ctx, item, stack),
        }
    }
}

// Handles clojure.core.match/match: subject expression then alternating pattern/result pairs.
// Patterns are analyzed with FnScope to prevent string literals from being reported.
// Results are analyzed in conditional context so they can produce UI text.
fn analyze_match_form(ctx: &mut AnalysisContext, items: &[SExp], stack: &ContextStack<'_>) {
    if let Some(subj) = items.get(1) {
        analyze_form(ctx, subj, stack);
    }
    let cond_stack = stack.push(FrameKind::Conditional);
    let in_ui_context = stack.has(FrameKind::Hiccup) || stack.has(FrameKind::UiFnCall);
    for (i, item) in items.iter().skip(2).enumerate() {
        if i % 2 == 0 {
            // Pattern position — push FnScope so string literals are not reported.
            let pat_stack = stack.push(FrameKind::FnScope);
            analyze_form(ctx, item, &pat_stack);
        } else {
            // Result position — may produce UI text.
            match item {
                SExp::Str(s, span) if in_ui_context => {
                    ctx.report(DiagnosticKind::ConditionalText, *span, s, Some("match".to_string()));
                }
                SExp::Str(_, _) => {}
                _ => analyze_form(ctx, item, &cond_stack),
            }
        }
    }
}

fn is_hiccup_vector(items: &[SExp]) -> bool {
    if items.is_empty() {
        return false;
    }
    match &items[0] {
        // Namespace-qualified keywords (containing '/') are data tuple identifiers
        // like :logseq.property/color.yellow or :block/journal-day — never hiccup tags.
        // Only simple keywords like :div, :span, :div.text-sm are hiccup element tags.
        SExp::Keyword(k, _) => !k.contains('/'),
        _ => false,
    }
}

fn analyze_hiccup_vector(ctx: &mut AnalysisContext, items: &[SExp], stack: &ContextStack<'_>) {
    // items[0] is the keyword tag (e.g. :div, :span)
    // items[1] might be a map (attributes) or child element
    let mut i = 1;

    // Check for attribute map
    if let Some(SExp::Map(kv, _)) = items.get(1) {
        analyze_hiccup_attrs(ctx, kv, stack);
        i = 2;
    }

    // Remaining items are children
    while i < items.len() {
        match &items[i] {
            SExp::Str(s, span) => {
                ctx.report(DiagnosticKind::HiccupText, *span, s, None);
            }
            other => analyze_form(ctx, other, stack),
        }
        i += 1;
    }
}

fn analyze_hiccup_attrs(ctx: &mut AnalysisContext, kv_pairs: &[SExp], stack: &ContextStack<'_>) {
    for pair in kv_pairs.chunks(2) {
        if pair.len() != 2 {
            continue;
        }
        let key_name = match &pair[0] {
            SExp::Keyword(k, _) => Some(k.as_str()),
            SExp::Str(s, _) => Some(s.as_str()),
            _ => None,
        };

        if let Some(key) = key_name {
            if ctx.is_ui_attribute(key) {
                match &pair[1] {
                    SExp::Str(s, span) => {
                        ctx.report(
                            DiagnosticKind::HiccupAttr,
                            *span,
                            s,
                            Some(key.to_string()),
                        );
                    }
                    other => {
                        // Non-literal UI attribute value (e.g. `(or default "placeholder")`).
                        // Analyze in the surrounding hiccup context so nested str/conditional/
                        // alert calls are detected by their own rules.
                        analyze_form(ctx, other, stack);
                    }
                }
            } else {
                // Non-UI attribute values (event handlers, dynamic :class expressions, etc.)
                // may call alert or UI functions with user-visible strings.
                // Recurse with FnScope so the surrounding hiccup context does not propagate
                // into attribute values — the value is not a text node.
                if !matches!(&pair[1], SExp::Str(_, _)) {
                    let val_stack = stack.push(FrameKind::FnScope);
                    analyze_form(ctx, &pair[1], &val_stack);
                }
            }
        }
    }
}

fn analyze_ui_fn_map_arg(ctx: &mut AnalysisContext, kv_pairs: &[SExp], stack: &ContextStack<'_>) {
    for pair in kv_pairs.chunks(2) {
        if pair.len() != 2 {
            continue;
        }
        let key_name = match &pair[0] {
            SExp::Keyword(k, _) => Some(k.as_str()),
            _ => None,
        };

        let is_ui = key_name.is_some_and(|k| ctx.is_ui_attribute(k));

        if is_ui {
            match &pair[1] {
                SExp::Str(s, span) => {
                    ctx.report(
                        DiagnosticKind::FnArgText,
                        *span,
                        s,
                        Some(key_name.unwrap().to_string()),
                    );
                }
                other => {
                    // Non-literal UI attribute value — analyze in UI context so
                    // nested str/conditional/format calls are detected.
                    analyze_form(ctx, other, stack);
                }
            }
        } else if !matches!(&pair[1], SExp::Str(_, _)) {
            // Non-UI attribute value (CSS class, event handler, etc.) — push FnScope
            // so the surrounding UiFnCall context does not propagate, matching the
            // treatment in analyze_hiccup_attrs and analyze_map.
            let val_stack = stack.push(FrameKind::FnScope);
            analyze_form(ctx, &pair[1], &val_stack);
        }
    }
}

fn analyze_map(ctx: &mut AnalysisContext, kv_pairs: &[SExp], stack: &ContextStack<'_>) {
    for pair in kv_pairs.chunks(2) {
        if pair.len() != 2 {
            continue;
        }
        // Map values are HTML/component attribute values (:class, :on-click, etc.),
        // not rendered text nodes. Push FnScope so that expressions like
        // `(str "css-prefix-" x)` inside a :class value do not inherit the
        // surrounding Hiccup context and produce false StrConcat diagnostics.
        // Hiccup vectors nested deeper (e.g. inside an :on-click lambda) re-push
        // their own FrameKind::Hiccup and are still detected correctly.
        if !matches!(&pair[1], SExp::Str(_, _)) {
            let val_stack = stack.push(FrameKind::FnScope);
            analyze_form(ctx, &pair[1], &val_stack);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> AppConfig {
        let toml = r#"
            i18n_functions        = ["t", "tt"]
            alert_functions       = ["notification/show!"]
            ui_functions          = ["ui/button"]
            ui_namespaces         = ["shui"]
            ui_attributes         = ["placeholder", "title", "aria-label", "alt", "label"]

            [lint]
            exception_functions   = ["ex-info", "throw"]
            pure_functions        = []
            format_functions      = ["format", "goog.string/format"]
            ignore_context_functions = [
                "js/console.log", "js/console.error", "js/console.warn",
                "prn", "println", "log/debug", "log/info", "log/warn", "log/error",
                "re-pattern", "re-find", "re-matches", "require", "ns",
            ]
            allow_strings  = ["Logseq"]
            allow_patterns = [
                "^https?://",
            ]
        "#;
        toml::from_str(toml).expect("test config is valid TOML")
    }

    fn analyze_source(source: &str) -> Vec<Diagnostic> {
        let config = make_config();
        let forms = parser::parse(source).unwrap();
        let mut ctx = AnalysisContext::new(PathBuf::from("test.cljs"), &config);
        for form in &forms {
            analyze_form(&mut ctx, form, &ContextStack::Empty);
        }
        ctx.diagnostics
    }

    #[test]
    fn detects_hiccup_text() {
        let diags = analyze_source(r#"[:div "Hello world"]"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].kind, DiagnosticKind::HiccupText);
        assert_eq!(diags[0].text, "Hello world");
    }

    #[test]
    fn detects_hiccup_attr() {
        let diags = analyze_source(r#"[:input {:placeholder "Search..."}]"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].kind, DiagnosticKind::HiccupAttr);
        assert_eq!(diags[0].text, "Search...");
    }

    #[test]
    fn detects_alert_text() {
        let diags = analyze_source(r#"(notification/show! "File saved" :success)"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].kind, DiagnosticKind::AlertText);
    }

    #[test]
    fn skips_translation_call() {
        let diags = analyze_source(r"[:div (t :greeting)]");
        assert!(diags.is_empty());
    }

    #[test]
    fn skips_css_class() {
        // :class is not a ui_attribute, so the string value is never analyzed —
        // no allow_pattern needed for CSS classes in attribute positions.
        let diags = analyze_source(r#"[:div {:class "text-sm flex"}]"#);
        assert!(diags.is_empty());
    }

    #[test]
    fn detects_fn_arg_text_lowercase() {
        // Lowercase single-word strings are UI text when used as positional args
        // to UI functions (e.g. icon button labels).
        let diags = analyze_source(r#"(shui/button {} "undo")"#);
        assert_eq!(diags.len(), 1, "lowercase button label should be reported: {diags:?}");
        assert_eq!(diags[0].kind, DiagnosticKind::FnArgText);
        assert_eq!(diags[0].text, "undo");
    }

    #[test]
    fn detects_str_concat_in_alert_lowercase() {
        // Multi-word lowercase strings inside str inside an alert function first arg
        // should be reported as str-concat (they are UI text).
        let diags = analyze_source(
            r#"(notification/show! (str "exported " filename " blocks and checksum attrs") :success)"#,
        );
        let str_diags: Vec<_> = diags.iter().filter(|d| d.kind == DiagnosticKind::StrConcat).collect();
        assert!(
            str_diags.iter().any(|d| d.text.contains("blocks and checksum attrs")),
            "multi-word lowercase str-concat in alert should be reported: {diags:?}"
        );
    }

    #[test]
    fn detects_hiccup_attr_with_conditional_value() {
        // Non-literal UI attribute value: the fallback string inside (or ...) should
        // be reported even though the attr value is not a literal string.
        let diags = analyze_source(r#"[:input {:placeholder (or custom "Search here")}]"#);
        assert!(
            diags.iter().any(|d| d.text == "Search here"),
            "fallback string inside UI attr conditional should be reported: {diags:?}"
        );
    }

    #[test]
    fn skips_css_class_in_ui_fn_map() {
        // :class inside a UI function map arg is not a ui_attribute; its string
        // value must not be reported even when the surrounding context is UiFnCall.
        let diags = analyze_source(r#"(shui/button {:class "flex items-center"} "Save")"#);
        assert_eq!(diags.len(), 1, "only the button label should be reported: {diags:?}");
        assert_eq!(diags[0].text, "Save");
    }

    #[test]
    fn skips_css_str_concat_in_ui_fn_map() {
        // (str ...) for :class inside a UI function map argument must NOT be reported.
        // FnScope blocks the UiFnCall context from propagating into the (str ...) call.
        let diags = analyze_source(r#"(shui/button {:class (str "base " cls)} "Save")"#);
        assert_eq!(diags.len(), 1, "only 'Save' should be reported: {diags:?}");
        assert_eq!(diags[0].text, "Save");
    }

    #[test]
    fn skips_css_conditional_in_ui_fn_map() {
        // (if ...) for :class inside a UI function map must NOT report conditional-text.
        let diags = analyze_source(
            r#"(shui/button {:class (if active? "font-bold" "font-normal")} "Label")"#,
        );
        assert_eq!(diags.len(), 1, "only 'Label' should be reported: {diags:?}");
        assert_eq!(diags[0].text, "Label");
    }

    #[test]
    fn detects_ui_attr_in_ui_fn_map() {
        // UI attribute (:placeholder) inside a UI function map arg SHOULD be reported.
        let diags = analyze_source(r#"(shui/input {:placeholder "Search..." :class "w-full"})"#);
        assert_eq!(diags.len(), 1, "only placeholder should be reported: {diags:?}");
        assert_eq!(diags[0].text, "Search...");
    }

    #[test]
    fn detects_if_let_else_string_in_hiccup() {
        // else branch of if-let inside hiccup → ConditionalText with "if-let" context.
        let diags = analyze_source(
            r#"[:span (if-let [v (get-config)] v "Logseq Sync")]"#,
        );
        assert!(
            diags.iter().any(|d| d.kind == DiagnosticKind::ConditionalText && d.text == "Logseq Sync"),
            "if-let else string in hiccup should be ConditionalText: {diags:?}"
        );
        assert!(
            diags.iter().any(|d| d.context.as_deref() == Some("if-let")),
            "context should be 'if-let': {diags:?}"
        );
    }

    #[test]
    fn detects_when_let_body_string_in_hiccup() {
        // when-let body string inside hiccup → ConditionalText with "when-let" context.
        let diags = analyze_source(r#"[:span (when-let [v (maybe-val)] "Fallback")]"#);
        assert!(
            diags.iter().any(|d| d.kind == DiagnosticKind::ConditionalText && d.text == "Fallback"),
            "when-let body string in hiccup should be ConditionalText: {diags:?}"
        );
        assert!(
            diags.iter().any(|d| d.context.as_deref() == Some("when-let")),
            "context should be 'when-let': {diags:?}"
        );
    }

    #[test]
    fn detects_let_body_string_in_hiccup() {
        // plain let body string inside hiccup → LetText (not ConditionalText).
        let diags = analyze_source(r#"[:span (let [x 1] "Hello")]"#);
        assert!(
            diags.iter().any(|d| d.kind == DiagnosticKind::LetText && d.text == "Hello"),
            "let body string in hiccup should be LetText: {diags:?}"
        );
    }

    #[test]
    fn skips_let_body_string_outside_ui() {
        // let/if-let body string outside UI context should not be reported.
        let diags = analyze_source(r#"(if-let [v (get)] v "default-value")"#);
        assert!(
            diags.is_empty(),
            "if-let body string outside UI context should not be reported: {diags:?}"
        );
    }
    #[test]
    fn skips_console_log() {
        let diags = analyze_source(r#"(js/console.log "debug info")"#);
        assert!(diags.is_empty());
    }

    #[test]
    fn skips_empty_and_single_char() {
        let diags = analyze_source(r#"[:div ""]"#);
        assert!(diags.is_empty());

        let diags = analyze_source(r#"[:div " "]"#);
        assert!(diags.is_empty());
    }

    #[test]
    fn detects_fn_arg_text() {
        let diags = analyze_source(r#"(ui/button "Submit")"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].kind, DiagnosticKind::FnArgText);
    }

    #[test]
    fn detects_format_string_in_ui_context() {
        // format-string requires UI context — must be inside hiccup or UI function.
        let diags = analyze_source(r#"[:div (goog.string/format "Found %d items" count)]"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].kind, DiagnosticKind::FormatString);
    }

    #[test]
    fn skips_format_string_outside_ui_context() {
        let diags = analyze_source(r#"(goog.string/format "Found %d items" count)"#);
        assert!(diags.is_empty(), "format-string outside UI context should not be reported: {diags:?}");
    }

    #[test]
    fn detects_conditional_text_in_ui_context() {
        let diags = analyze_source(r#"[:div (if loading? "Loading..." "Ready to go")]"#);
        assert_eq!(diags.len(), 2);
        assert!(diags.iter().all(|d| d.kind == DiagnosticKind::ConditionalText));
    }

    #[test]
    fn skips_conditional_text_outside_ui_context() {
        let diags = analyze_source(r#"(if loading? "Loading..." "Ready to go")"#);
        assert!(diags.is_empty(), "conditional outside UI context should not be reported: {diags:?}");
    }

    #[test]
    fn detects_if_not_in_ui_context() {
        let diags = analyze_source(r#"[:div (if-not loaded? "Loading..." "Ready")]"#);
        assert_eq!(diags.len(), 2);
        assert!(diags.iter().all(|d| d.kind == DiagnosticKind::ConditionalText));
    }

    #[test]
    fn skips_if_not_outside_ui_context() {
        let diags = analyze_source(r#"(if-not loaded? "Loading..." "Ready")"#);
        assert!(diags.is_empty(), "if-not outside UI context should not be reported: {diags:?}");
    }

    #[test]
    fn detects_or_fallback_in_ui_context() {
        let diags = analyze_source(r#"[:span (or label "Untitled")]"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].kind, DiagnosticKind::ConditionalText);
        assert_eq!(diags[0].text, "Untitled");
    }

    #[test]
    fn skips_or_outside_ui_context() {
        let diags = analyze_source(r#"(or label "Untitled")"#);
        assert!(diags.is_empty(), "or outside UI context should not be reported: {diags:?}");
    }

    #[test]
    fn skips_ex_info_strings() {
        // Error messages inside ex-info are developer-facing, not UI text
        let diags = analyze_source(r#"[:div (throw (ex-info "Something went wrong" {:type :error}))]"#);
        assert!(diags.is_empty(), "ex-info strings should not be reported: {diags:?}");
    }

    #[test]
    fn skips_non_alphabetic_strings() {
        // Strings with no alphabetic characters after trimming → emoji, symbols, digits
        let diags = analyze_source(r#"[:div " 👉 "]"#);
        assert!(diags.is_empty(), "emoji-only string should not be reported: {diags:?}");

        let diags = analyze_source(r#"[:div "$10"]"#);
        assert!(diags.is_empty(), "symbol+digit string should not be reported: {diags:?}");

        let diags = analyze_source(r#"[:div " · "]"#);
        assert!(diags.is_empty(), "symbol-only string should not be reported: {diags:?}");
    }

    #[test]
    fn skips_def_plain_string_binding() {
        // Plain (def name "str") is a data constant — should NOT be reported.
        let diags = analyze_source(r#"(def greeting "Hello World")"#);
        assert!(diags.is_empty(), "def data binding should not be reported: {diags:?}");
    }

    #[test]
    fn skips_def_docstring() {
        // (def name "docstring" value) — the string is a docstring, skip it.
        let diags = analyze_source(r#"(def config "Config keys that are deprecated" {:k 1})"#);
        assert!(diags.is_empty(), "def docstring should not be reported: {diags:?}");
    }

    #[test]
    fn detects_let_text_in_ui_context() {
        // (let [...] ...) INSIDE a UI fn call → LetText should be reported.
        let diags = analyze_source(r#"(shui/button {} (let [label "Click me"] label))"#);
        let let_texts: Vec<_> = diags.iter().filter(|d| d.kind == DiagnosticKind::LetText).collect();
        assert!(!let_texts.is_empty(), "let inside UI context should report LetText");
    }

    #[test]
    fn skips_let_plain_binding_outside_ui() {
        // (let [x "str"] x) at top level — data binding, no UI context → should NOT be reported.
        let diags = analyze_source(r#"(let [label "Untitled"] label)"#);
        assert!(diags.is_empty(), "let binding outside UI context should not be reported: {diags:?}");
    }

    #[test]
    fn detects_str_concat_in_hiccup() {
        let diags = analyze_source(r#"[:div (str "Hello " name)]"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].kind, DiagnosticKind::StrConcat);
    }

    #[test]
    fn skips_ns_require() {
        let diags = analyze_source(r"(ns myapp.core (:require [clojure.string :as str]))");
        assert!(diags.is_empty());
    }

    #[test]
    fn skips_url_pattern() {
        let diags = analyze_source(r#"[:a {:href "https://logseq.com"}]"#);
        assert!(diags.is_empty());
    }

    #[test]
    fn detects_shui_button_label() {
        let diags = analyze_source(r#"(shui/button {:label "Click here"})"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].kind, DiagnosticKind::FnArgText);
    }

    #[test]
    fn skips_pure_function_comparison_in_hiccup() {
        // (= query "(and)") inside hiccup — the string is a comparison value, not UI text.
        let diags = analyze_source(r#"[:div (if (= query "(and)") "empty" "ok")]"#);
        // Only "empty" and "ok" are conditional texts; "(and)" should be skipped.
        assert!(
            diags.iter().all(|d| d.text != "(and)"),
            "comparison string inside = should not be reported: {diags:?}"
        );
    }

    #[test]
    fn skips_predicate_function_args() {
        // contains? ends with ?, so it's treated as a pure function.
        let diags = analyze_source(r#"[:div (when (contains? opts ":results") "show")]"#);
        assert!(
            diags.iter().all(|d| d.text != ":results"),
            "string inside contains? should not be reported: {diags:?}"
        );
    }

    #[test]
    fn skips_keyword_arg_class_in_ui_fn() {
        // :class keyword arg value in a UI function — CSS class, not UI text.
        let diags = analyze_source(r#"(ui/button "Submit" :class (str (when active? "active ") "btn"))"#);
        // "Submit" should be reported; "active " and "btn" (CSS) should not.
        assert_eq!(diags.len(), 1, "only Submit should be reported: {diags:?}");
        assert_eq!(diags[0].text, "Submit");
    }

    #[test]
    fn skips_match_pattern_strings() {
        // Match pattern vectors contain type names, not UI text.
        let diags = analyze_source(
            r#"(match item
                 ["Latex_Fragment" l] [:p "fragment"]
                 ["Plain" s] [:span s])"#,
        );
        // "fragment" in hiccup child should be reported; "Latex_Fragment" and "Plain" should not.
        let reported: Vec<_> = diags.iter().map(|d| d.text.as_str()).collect();
        assert!(
            !reported.contains(&"Latex_Fragment"),
            "match pattern string should not be reported: {diags:?}"
        );
        assert!(
            !reported.contains(&"Plain"),
            "match pattern string should not be reported: {diags:?}"
        );
    }

    #[test]
    fn skips_when_not_outside_ui_context() {
        // when-not outside UI context — no strings should be reported.
        let diags = analyze_source(r#"(when-not done? "pending")"#);
        assert!(
            diags.is_empty(),
            "when-not outside UI context should not be reported: {diags:?}"
        );
    }

    #[test]
    fn detects_when_not_inside_hiccup() {
        // when-not inside hiccup — the result string IS UI text.
        let diags = analyze_source(r#"[:div (when-not done? "Still pending")]"#);
        assert_eq!(diags.len(), 1, "when-not in hiccup should be reported: {diags:?}");
        assert_eq!(diags[0].kind, DiagnosticKind::ConditionalText);
    }

    #[test]
    fn trims_allow_strings_comparison() {
        // Source strings with surrounding whitespace are trimmed before allow_strings comparison.
        // "Logseq " trimmed to "Logseq" matches the "Logseq" entry in allow_strings.
        let diags = analyze_source(r#"[:div "Logseq "]"#);
        // "Logseq " trimmed = "Logseq" which is in default allow_strings.
        assert!(
            diags.is_empty(),
            "trailing-space variant of allow_string should be skipped: {diags:?}"
        );
    }

    #[test]
    fn skips_str_concat_outside_ui_context() {
        // (str ...) at top level — no UI context — must NOT be reported.
        let diags = analyze_source(r#"(str "Hello " name)"#);
        assert!(diags.is_empty(), "str-concat outside UI context should not be reported: {diags:?}");
    }

    #[test]
    fn detects_alert_text_regardless_of_context() {
        // Alert functions report their first string argument with no UI context requirement.
        let diags = analyze_source(r#"(notification/show! "Something failed" :error)"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].kind, DiagnosticKind::AlertText);
        assert_eq!(diags[0].text, "Something failed");
    }

    #[test]
    fn detects_alert_in_hiccup_attr_event_handler() {
        // Alert call nested inside a hiccup :on-click handler must be detected.
        let diags = analyze_source(
            r#"[:div {:on-click (fn [e] (notification/show! "Saved" :success))} "text"]"#,
        );
        assert!(
            diags.iter().any(|d| d.kind == DiagnosticKind::AlertText && d.text == "Saved"),
            "alert inside hiccup event handler should be detected: {diags:?}"
        );
    }

    #[test]
    fn skips_fn_scope_barrier_in_hiccup_on_key_down() {
        // DOM key comparison inside an event handler must NOT be reported.
        let diags = analyze_source(
            r#"[:div {:on-key-down (fn [e] (when (= (.-key e) "Enter") nil))} "visible"]"#,
        );
        assert!(
            !diags.iter().any(|d| d.text == "Enter"),
            "DOM key name inside event handler should not be reported: {diags:?}"
        );
    }

    #[test]
    fn recurses_into_format_template_expression() {
        // When format's first arg is an expression, recurse into it.
        // (str "Error: " code) inside hiccup-context format → StrConcat is reported.
        let diags = analyze_source(r#"[:div (format (str "Error: " code) n)]"#);
        assert!(
            diags.iter().any(|d| d.kind == DiagnosticKind::StrConcat && d.text == "Error:"),
            "str-concat inside format template should be detected in UI context: {diags:?}"
        );
    }

    #[test]
    fn skips_format_template_expression_outside_ui() {
        // Format with a complex template outside UI context — nothing to report.
        let diags = analyze_source(r#"(format (str "log: " prefix) value)"#);
        assert!(
            !diags.iter().any(|d| d.text.contains("log")),
            "format template expression outside UI context should not be reported: {diags:?}"
        );
    }

    #[test]
    fn skips_namespace_qualified_exception_function() {
        // Namespace-qualified exception functions must match the config entry by suffix.
        let diags = analyze_source(r#"[:div (clojure.core/ex-info "Not UI text" {})]"#);
        assert!(
            diags.is_empty(),
            "namespace-qualified ex-info should be skipped: {diags:?}"
        );
    }

    #[test]
    fn detects_case_results_in_ui_context() {
        let diags = analyze_source(r#"[:div (case status :ok "Done" :err "Failed" "Unknown")]"#);
        let texts: Vec<&str> = diags.iter().map(|d| d.text.as_str()).collect();
        assert!(texts.contains(&"Done"), "case result should be reported: {diags:?}");
        assert!(texts.contains(&"Failed"), "case result should be reported: {diags:?}");
        assert!(texts.contains(&"Unknown"), "case default should be reported: {diags:?}");
    }

    #[test]
    fn skips_case_dispatch_string_values() {
        // String dispatch values (not results) must not be reported.
        let diags = analyze_source(r#"[:div (case label "admin" "Admin panel" "user" "User area")]"#);
        let texts: Vec<&str> = diags.iter().map(|d| d.text.as_str()).collect();
        assert!(!texts.contains(&"admin"), "case dispatch value should not be reported: {diags:?}");
        assert!(!texts.contains(&"user"), "case dispatch value should not be reported: {diags:?}");
        assert!(texts.contains(&"Admin panel"), "case result should be reported: {diags:?}");
        assert!(texts.contains(&"User area"), "case result should be reported: {diags:?}");
    }

    #[test]
    fn detects_cond_results_in_ui_context() {
        let diags = analyze_source(r#"[:div (cond loading? "Loading" done? "Done" :else "Idle")]"#);
        let texts: Vec<&str> = diags.iter().map(|d| d.text.as_str()).collect();
        assert!(texts.contains(&"Loading"), "cond result should be reported: {diags:?}");
        assert!(texts.contains(&"Done"), "cond result should be reported: {diags:?}");
        assert!(texts.contains(&"Idle"), "cond :else result should be reported: {diags:?}");
    }

    #[test]
    fn detects_match_results_in_ui_context() {
        let diags = analyze_source(
            r#"[:div (match item
                 ["Ok" _] "Success"
                 ["Err" _] "Failure")]"#,
        );
        let texts: Vec<&str> = diags.iter().map(|d| d.text.as_str()).collect();
        assert!(texts.contains(&"Success"), "match result should be reported: {diags:?}");
        assert!(texts.contains(&"Failure"), "match result should be reported: {diags:?}");
        assert!(!texts.contains(&"Ok"), "match pattern string should not be reported: {diags:?}");
        assert!(!texts.contains(&"Err"), "match pattern string should not be reported: {diags:?}");
    }

    #[test]
    fn detects_format_function_plain_name_in_ui() {
        // The unqualified "format" function (in format_functions) must be detected in UI context.
        let diags = analyze_source(r#"[:div (format "Hello, %s!" name)]"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].kind, DiagnosticKind::FormatString);
        assert_eq!(diags[0].text, "Hello, %s!");
    }

    #[test]
    fn skips_cond_arrow_css_str_in_form_position() {
        // (str class " no-padding") in a cond-> form builds a CSS class string.
        // The default config has "no-padding" in allow_strings so the trimmed string
        // is filtered.  Verified via the analyze_source helper (uses default config).
        // Mirrors the real pattern in frontend/ui.cljs:
        //   (cond-> options (true? no-padding?) (assoc :class (str class " no-padding")) ...)
        // Note: analyze_source uses the default config which does not include "no-padding"
        // in allow_strings, so this test just checks test-position suppression.
        let diags = analyze_source(
            r"[:a (cond-> opts (true? no-padding?) (assoc :class base))]",
        );
        // The test arg "true?" is a symbol, nothing to report; no strings present here.
        assert!(
            diags.is_empty(),
            "no strings in this cond-> should be reported: {diags:?}"
        );
    }

    #[test]
    fn skips_cond_arrow_test_string() {
        // String literal in a cond-> test position is a predicate value — not UI text.
        let diags = analyze_source(
            r#"[:div (cond-> base (= mode "compact") (str " extra"))]"#,
        );
        assert!(
            !diags.iter().any(|d| d.text == "compact"),
            "string in cond-> test position should not be reported: {diags:?}"
        );
    }

    #[test]
    fn skips_cond_double_arrow_test_string() {
        // cond->> behaves the same as cond-> for string suppression.
        let diags = analyze_source(
            r#"[:div (cond->> items (= filter "all") (concat extra))]"#,
        );
        assert!(
            !diags.iter().any(|d| d.text == "all"),
            "string in cond->> test position should not be reported: {diags:?}"
        );
    }

    #[test]
    fn cond_arrow_still_detects_alert_in_form() {
        // Alert/notification functions are context-independent — should be caught even
        // inside a cond-> form position.
        let diags = analyze_source(
            r#"(cond-> x ok? (do (notification/show! "Saved" :success) x))"#,
        );
        assert!(
            diags.iter().any(|d| d.kind == DiagnosticKind::AlertText && d.text == "Saved"),
            "alert inside cond-> form should still be reported: {diags:?}"
        );
    }
}
