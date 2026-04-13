use std::fmt;
use std::path::PathBuf;

use crate::config::AppConfig;

/// Represents a position in source code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line: u32,
    pub col: u32,
    pub offset: u32,
}

/// S-expression AST node.
#[derive(Debug, Clone, PartialEq)]
pub enum SExp {
    List(Vec<SExp>, Span),
    Vector(Vec<SExp>, Span),
    Map(Vec<SExp>, Span),
    Set(Vec<SExp>, Span),
    Symbol(String, Span),
    Keyword(String, Span),
    Str(String, Span),
    Number(String, Span),
    Regex(String, Span),
    Char(char, Span),
    Bool(bool, Span),
    Nil(Span),
    Quote(Box<SExp>, Span),
    SyntaxQuote(Box<SExp>, Span),
    Unquote(Box<SExp>, Span),
    UnquoteSplicing(Box<SExp>, Span),
    Deref(Box<SExp>, Span),
    Meta(Box<SExp>, Box<SExp>, Span),
    Discard(Box<SExp>, Span),
    VarQuote(Box<SExp>, Span),
    AnonFn(Vec<SExp>, Span),
    ReaderConditional(Vec<SExp>, Span),
    ReaderConditionalSplicing(Vec<SExp>, Span),
    TaggedLiteral(String, Box<SExp>, Span),
}

impl SExp {
    #[allow(dead_code)]
    pub fn span(&self) -> Span {
        match self {
            Self::List(_, s)
            | Self::Vector(_, s)
            | Self::Map(_, s)
            | Self::Set(_, s)
            | Self::Symbol(_, s)
            | Self::Keyword(_, s)
            | Self::Str(_, s)
            | Self::Number(_, s)
            | Self::Regex(_, s)
            | Self::Char(_, s)
            | Self::Bool(_, s)
            | Self::Nil(s)
            | Self::Quote(_, s)
            | Self::SyntaxQuote(_, s)
            | Self::Unquote(_, s)
            | Self::UnquoteSplicing(_, s)
            | Self::Deref(_, s)
            | Self::Meta(_, _, s)
            | Self::Discard(_, s)
            | Self::VarQuote(_, s)
            | Self::AnonFn(_, s)
            | Self::ReaderConditional(_, s)
            | Self::ReaderConditionalSplicing(_, s)
            | Self::TaggedLiteral(_, _, s) => *s,
        }
    }
}

/// Parse errors.
#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.span.line, self.span.col, self.message)
    }
}

impl std::error::Error for ParseError {}

/// Parse all top-level forms from a Clojure source string.
/// `file_hint` is shown in parse warnings and may be an empty string.
pub fn parse_with_hint(source: &str, file_hint: &str) -> Result<Vec<SExp>, ParseError> {
    let mut reader = Reader::new(source);
    let mut forms = Vec::new();

    loop {
        reader.skip_whitespace_and_comments();
        if reader.is_eof() {
            break;
        }
        match reader.read_form() {
            Ok(Some(form)) => forms.push(form),
            Ok(None) => break,
            Err(e) => {
                // Error recovery: skip to next non-whitespace and try again
                reader.advance();
                if file_hint.is_empty() {
                    eprintln!("parse warning: {e}");
                } else {
                    eprintln!("parse warning: {file_hint}: {e}");
                }
            }
        }
    }

    Ok(forms)
}

/// Parse all top-level forms from Clojure source code.
#[allow(dead_code)]
pub fn parse(source: &str) -> Result<Vec<SExp>, ParseError> {
    parse_with_hint(source, "")
}

/// Parse a single file, returning AST and file path info.
pub fn parse_file(path: &PathBuf, _config: &AppConfig) -> Result<Vec<SExp>, Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(path)?;
    let hint = path.to_string_lossy();
    let forms = parse_with_hint(&source, &hint)?;
    Ok(forms)
}

const MAX_DEPTH: usize = 256;

struct Reader<'src> {
    source: &'src str,
    bytes: &'src [u8],
    pos: usize,
    line: u32,
    col: u32,
    depth: usize,
}

impl<'src> Reader<'src> {
    fn new(source: &'src str) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            pos: 0,
            line: 1,
            col: 1,
            depth: 0,
        }
    }

    fn span(&self) -> Span {
        Span {
            line: self.line,
            col: self.col,
            offset: self.pos as u32,
        }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }


    fn advance(&mut self) -> Option<u8> {
        if self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            self.pos += 1;
            if b == b'\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
            Some(b)
        } else {
            None
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(b) if b.is_ascii_whitespace() || b == b',' => {
                    self.advance();
                }
                Some(b';') => {
                    while let Some(b) = self.peek() {
                        if b == b'\n' {
                            break;
                        }
                        self.advance();
                    }
                }
                _ => break,
            }
        }
    }

    fn error(&self, message: impl Into<String>) -> ParseError {
        ParseError {
            message: message.into(),
            span: self.span(),
        }
    }

    fn read_form(&mut self) -> Result<Option<SExp>, ParseError> {
        self.skip_whitespace_and_comments();
        if self.is_eof() {
            return Ok(None);
        }

        if self.depth > MAX_DEPTH {
            return Err(self.error("maximum nesting depth exceeded"));
        }

        let span = self.span();

        match self.peek() {
            Some(b'(') => self.read_list(),
            Some(b'[') => self.read_vector(),
            Some(b'{') => self.read_map(),
            Some(b'"') => self.read_string(),
            Some(b':') => self.read_keyword(),
            Some(b'\\') => self.read_char_literal(),
            Some(b'\'') => {
                self.advance();
                let inner = self.read_form()?.ok_or_else(|| self.error("expected form after '"))?;
                Ok(Some(SExp::Quote(Box::new(inner), span)))
            }
            Some(b'`') => {
                self.advance();
                let inner = self.read_form()?.ok_or_else(|| self.error("expected form after `"))?;
                Ok(Some(SExp::SyntaxQuote(Box::new(inner), span)))
            }
            Some(b'~') => {
                self.advance();
                if self.peek() == Some(b'@') {
                    self.advance();
                    let inner = self.read_form()?.ok_or_else(|| self.error("expected form after ~@"))?;
                    Ok(Some(SExp::UnquoteSplicing(Box::new(inner), span)))
                } else {
                    let inner = self.read_form()?.ok_or_else(|| self.error("expected form after ~"))?;
                    Ok(Some(SExp::Unquote(Box::new(inner), span)))
                }
            }
            Some(b'@') => {
                self.advance();
                let inner = self.read_form()?.ok_or_else(|| self.error("expected form after @"))?;
                Ok(Some(SExp::Deref(Box::new(inner), span)))
            }
            Some(b'^') => self.read_metadata(span),
            Some(b'#') => self.read_dispatch(),
            Some(b')' | b']' | b'}') => Err(self.error("unexpected closing delimiter")),
            Some(_) => Ok(self.read_symbol_or_number()),
            None => Ok(None),
        }
    }

    fn read_list(&mut self) -> Result<Option<SExp>, ParseError> {
        let span = self.span();
        self.advance(); // consume '('
        self.depth += 1;
        let items = self.read_delimited(b')')?;
        self.depth -= 1;
        Ok(Some(SExp::List(items, span)))
    }

    fn read_vector(&mut self) -> Result<Option<SExp>, ParseError> {
        let span = self.span();
        self.advance(); // consume '['
        self.depth += 1;
        let items = self.read_delimited(b']')?;
        self.depth -= 1;
        Ok(Some(SExp::Vector(items, span)))
    }

    fn read_map(&mut self) -> Result<Option<SExp>, ParseError> {
        let span = self.span();
        self.advance(); // consume '{'
        self.depth += 1;
        let items = self.read_delimited(b'}')?;
        self.depth -= 1;
        Ok(Some(SExp::Map(items, span)))
    }

    fn read_delimited(&mut self, closing: u8) -> Result<Vec<SExp>, ParseError> {
        let mut items = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            match self.peek() {
                Some(b) if b == closing => {
                    self.advance();
                    return Ok(items);
                }
                None => return Err(self.error(format!("unexpected EOF, expected '{}'", closing as char))),
                _ => {
                    if let Some(form) = self.read_form()? {
                        items.push(form);
                    }
                }
            }
        }
    }

    fn read_string(&mut self) -> Result<Option<SExp>, ParseError> {
        let span = self.span();
        self.advance(); // consume opening '"'
        let mut s = String::new();

        loop {
            match self.advance() {
                Some(b'"') => return Ok(Some(SExp::Str(s, span))),
                Some(b'\\') => {
                    match self.advance() {
                        Some(b'n') => s.push('\n'),
                        Some(b't') => s.push('\t'),
                        Some(b'r') => s.push('\r'),
                        Some(b'\\') => s.push('\\'),
                        Some(b'"') => s.push('"'),
                        Some(b'u') => {
                            let hex = self.read_n_chars(4)?;
                            let code = u32::from_str_radix(&hex, 16)
                                .map_err(|_| self.error(format!("invalid unicode escape: \\u{hex}")))?;

                            // Handle JavaScript/ClojureScript surrogate pairs: \uD800-\uDBFF
                            if (0xD800..=0xDBFF).contains(&code) {
                                // High surrogate — look ahead for a low surrogate \uDC00-\uDFFF
                                if self.bytes.get(self.pos) == Some(&b'\\')
                                    && self.bytes.get(self.pos + 1) == Some(&b'u')
                                {
                                    self.advance(); // consume '\\'
                                    self.advance(); // consume 'u'
                                    let hex2 = self.read_n_chars(4)?;
                                    if let Ok(low) = u32::from_str_radix(&hex2, 16) {
                                        if (0xDC00..=0xDFFF).contains(&low) {
                                            // Decode surrogate pair to Unicode scalar
                                            let scalar = 0x10000
                                                + (code - 0xD800) * 0x400
                                                + (low - 0xDC00);
                                            s.push(char::from_u32(scalar).unwrap_or('\u{FFFD}'));
                                        } else {
                                            s.push('\u{FFFD}');
                                        }
                                    } else {
                                        s.push('\u{FFFD}');
                                    }
                                } else {
                                    s.push('\u{FFFD}');
                                }
                            } else {
                                let ch = char::from_u32(code)
                                    .ok_or_else(|| self.error(format!("invalid unicode code point: {code}")))?;
                                s.push(ch);
                            }
                        }
                        Some(c) => {
                            s.push('\\');
                            s.push(c as char);
                        }
                        None => return Err(self.error("unexpected EOF in string escape")),
                    }
                }
                Some(b) if b < 0x80 => s.push(b as char), // ASCII fast path
                Some(b) => {
                    // Multi-byte UTF-8: the source is valid UTF-8, so we can read
                    // the full char from the string slice (we already consumed the
                    // first byte via advance(), so step back one to the char boundary).
                    let char_start = self.pos - 1;
                    if let Some(ch) = self.source[char_start..].chars().next() {
                        s.push(ch);
                        // advance past the remaining bytes of this char
                        let extra = ch.len_utf8().saturating_sub(1);
                        for _ in 0..extra {
                            self.advance();
                        }
                    } else {
                        // Invalid UTF-8: consume as replacement char
                        let _ = b;
                        s.push('\u{FFFD}');
                    }
                }
                None => return Err(self.error("unexpected EOF in string literal")),
            }
        }
    }

    fn read_n_chars(&mut self, n: usize) -> Result<String, ParseError> {
        let mut s = String::with_capacity(n);
        for _ in 0..n {
            match self.advance() {
                Some(b) => s.push(b as char),
                None => return Err(self.error("unexpected EOF reading escape sequence")),
            }
        }
        Ok(s)
    }

    fn read_keyword(&mut self) -> Result<Option<SExp>, ParseError> {
        let span = self.span();
        self.advance(); // consume ':'
        // Handle ::namespaced-keyword
        if self.peek() == Some(b':') {
            self.advance();
        }
        let name = self.read_token();
        if name.is_empty() {
            return Err(self.error("empty keyword"));
        }
        Ok(Some(SExp::Keyword(name, span)))
    }

    fn read_char_literal(&mut self) -> Result<Option<SExp>, ParseError> {
        let span = self.span();
        self.advance(); // consume '\'

        let start = self.pos;
        // Special-case: delimiter chars, comma (Clojure whitespace), and ASCII
        // whitespace are all valid single-char literals: \( \, \ etc.
        // They must be handled BEFORE the token loop so the loop doesn't break
        // immediately and produce an empty token.
        match self.peek() {
            None => return Err(self.error("unexpected EOF in char literal")),
            Some(b) if is_delimiter(b) || b == b',' || b.is_ascii_whitespace() => {
                self.advance();
                return Ok(Some(SExp::Char(b as char, span)));
            }
            _ => {}
        }

        // Read until delimiter or whitespace
        while let Some(b) = self.peek() {
            if b.is_ascii_whitespace() || is_delimiter(b) || b == b',' {
                break;
            }
            self.advance();
        }

        let token = &self.source[start..self.pos];
        let ch = match token {
            "newline" => '\n',
            "return" => '\r',
            "space" => ' ',
            "tab" => '\t',
            "backspace" => '\u{0008}',
            "formfeed" => '\u{000C}',
            s if s.starts_with('u') && s.len() == 5 => {
                let code = u32::from_str_radix(&s[1..], 16)
                    .map_err(|_| self.error(format!("invalid char unicode: \\{s}")))?;
                char::from_u32(code).ok_or_else(|| self.error(format!("invalid char code: {code}")))?
            }
            s if s.len() == 1 => s.chars().next().expect("non-empty single-char token"),
            other => return Err(self.error(format!("invalid char literal: \\{other}"))),
        };

        Ok(Some(SExp::Char(ch, span)))
    }

    fn read_dispatch(&mut self) -> Result<Option<SExp>, ParseError> {
        let span = self.span();
        self.advance(); // consume '#'

        match self.peek() {
            Some(b'{') => {
                // Set literal #{...}
                self.advance();
                self.depth += 1;
                let items = self.read_delimited(b'}')?;
                self.depth -= 1;
                Ok(Some(SExp::Set(items, span)))
            }
            Some(b'(') => {
                // Anonymous function #(...)
                self.advance();
                self.depth += 1;
                let items = self.read_delimited(b')')?;
                self.depth -= 1;
                Ok(Some(SExp::AnonFn(items, span)))
            }
            Some(b'"') => {
                // Regex #"pattern"
                self.advance(); // consume opening '"'
                let mut s = String::new();
                loop {
                    match self.advance() {
                        Some(b'"') => return Ok(Some(SExp::Regex(s, span))),
                        Some(b'\\') => {
                            s.push('\\');
                            if let Some(next) = self.advance() {
                                s.push(next as char);
                            }
                        }
                        Some(b) => s.push(b as char),
                        None => return Err(self.error("unexpected EOF in regex literal")),
                    }
                }
            }
            Some(b'\'') => {
                // Var quote #'symbol
                self.advance();
                let inner = self.read_form()?.ok_or_else(|| self.error("expected form after #'"))?;
                Ok(Some(SExp::VarQuote(Box::new(inner), span)))
            }
            Some(b'_') => {
                // Discard #_ form
                self.advance();
                let inner = self.read_form()?.ok_or_else(|| self.error("expected form after #_"))?;
                Ok(Some(SExp::Discard(Box::new(inner), span)))
            }
            Some(b'^') => {
                // #^metadata (old-style)
                self.advance();
                self.read_metadata(span)
            }
            Some(b'?') => {
                // Reader conditional #?(...) or #?@(...)
                self.advance();
                if self.peek() == Some(b'@') {
                    self.advance();
                    if self.peek() == Some(b'(') {
                        self.advance();
                        self.depth += 1;
                        let items = self.read_delimited(b')')?;
                        self.depth -= 1;
                        Ok(Some(SExp::ReaderConditionalSplicing(items, span)))
                    } else {
                        Err(self.error("expected '(' after #?@"))
                    }
                } else if self.peek() == Some(b'(') {
                    self.advance();
                    self.depth += 1;
                    let items = self.read_delimited(b')')?;
                    self.depth -= 1;
                    Ok(Some(SExp::ReaderConditional(items, span)))
                } else {
                    Err(self.error("expected '(' after #?"))
                }
            }
            Some(_) => {
                // Tagged literal, e.g. #inst "2023-..."
                let tag = self.read_token();
                if tag.is_empty() {
                    return Err(self.error("empty tagged literal"));
                }
                self.skip_whitespace_and_comments();
                let value = self.read_form()?.ok_or_else(|| self.error("expected form after tagged literal"))?;
                Ok(Some(SExp::TaggedLiteral(tag, Box::new(value), span)))
            }
            None => Err(self.error("unexpected EOF after #")),
        }
    }

    fn read_metadata(&mut self, span: Span) -> Result<Option<SExp>, ParseError> {
        self.advance(); // consume '^'
        let meta = self.read_form()?.ok_or_else(|| self.error("expected metadata form"))?;
        let target = self.read_form()?.ok_or_else(|| self.error("expected form after metadata"))?;
        Ok(Some(SExp::Meta(Box::new(meta), Box::new(target), span)))
    }

    fn read_symbol_or_number(&mut self) -> Option<SExp> {
        let span = self.span();
        let token = self.read_token();

        if token.is_empty() {
            return None;
        }

        match token.as_str() {
            "nil" => Some(SExp::Nil(span)),
            "true" => Some(SExp::Bool(true, span)),
            "false" => Some(SExp::Bool(false, span)),
            _ => {
                if is_number(&token) {
                    Some(SExp::Number(token, span))
                } else {
                    Some(SExp::Symbol(token, span))
                }
            }
        }
    }

    fn read_token(&mut self) -> String {
        let start = self.pos;
        while let Some(b) = self.peek() {
            if b.is_ascii_whitespace() || is_delimiter(b) || b == b',' {
                break;
            }
            self.advance();
        }
        self.source[start..self.pos].to_string()
    }
}

fn is_delimiter(b: u8) -> bool {
    matches!(b, b'(' | b')' | b'[' | b']' | b'{' | b'}' | b'"' | b';')
}

fn is_number(token: &str) -> bool {
    let bytes = token.as_bytes();
    if bytes.is_empty() {
        return false;
    }

    let start = if bytes[0] == b'+' || bytes[0] == b'-' {
        if bytes.len() == 1 {
            return false;
        }
        1
    } else {
        0
    };

    // Check if first char after optional sign is a digit
    if !bytes[start].is_ascii_digit() {
        return false;
    }

    // Accept anything that starts with a digit (integers, floats, ratios, hex, etc.)
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_list() {
        let forms = parse("(a b c)").unwrap();
        assert_eq!(forms.len(), 1);
        if let SExp::List(items, _) = &forms[0] {
            assert_eq!(items.len(), 3);
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn parse_string_literal() {
        let forms = parse(r#""hello world""#).unwrap();
        assert_eq!(forms.len(), 1);
        if let SExp::Str(s, _) = &forms[0] {
            assert_eq!(s, "hello world");
        } else {
            panic!("expected string");
        }
    }

    #[test]
    fn parse_string_escapes() {
        let forms = parse(r#""line1\nline2\t\"quoted\"""#).unwrap();
        if let SExp::Str(s, _) = &forms[0] {
            assert_eq!(s, "line1\nline2\t\"quoted\"");
        } else {
            panic!("expected string");
        }
    }

    #[test]
    fn parse_keyword() {
        let forms = parse(":foo").unwrap();
        if let SExp::Keyword(k, _) = &forms[0] {
            assert_eq!(k, "foo");
        } else {
            panic!("expected keyword");
        }
    }

    #[test]
    fn parse_vector() {
        let forms = parse("[1 2 3]").unwrap();
        if let SExp::Vector(items, _) = &forms[0] {
            assert_eq!(items.len(), 3);
        } else {
            panic!("expected vector");
        }
    }

    #[test]
    fn parse_map() {
        let forms = parse("{:a 1 :b 2}").unwrap();
        if let SExp::Map(items, _) = &forms[0] {
            assert_eq!(items.len(), 4);
        } else {
            panic!("expected map");
        }
    }

    #[test]
    fn parse_set() {
        let forms = parse("#{1 2 3}").unwrap();
        if let SExp::Set(items, _) = &forms[0] {
            assert_eq!(items.len(), 3);
        } else {
            panic!("expected set");
        }
    }

    #[test]
    fn parse_regex() {
        let forms = parse(r#"#"[a-z]+""#).unwrap();
        if let SExp::Regex(r, _) = &forms[0] {
            assert_eq!(r, "[a-z]+");
        } else {
            panic!("expected regex");
        }
    }

    #[test]
    fn parse_quote() {
        let forms = parse("'foo").unwrap();
        if let SExp::Quote(inner, _) = &forms[0] {
            if let SExp::Symbol(s, _) = inner.as_ref() {
                assert_eq!(s, "foo");
            } else {
                panic!("expected symbol in quote");
            }
        } else {
            panic!("expected quote");
        }
    }

    #[test]
    fn parse_deref() {
        let forms = parse("@atom").unwrap();
        if let SExp::Deref(inner, _) = &forms[0] {
            if let SExp::Symbol(s, _) = inner.as_ref() {
                assert_eq!(s, "atom");
            } else {
                panic!("expected symbol in deref");
            }
        } else {
            panic!("expected deref");
        }
    }

    #[test]
    fn parse_discard() {
        let forms = parse("#_ foo bar").unwrap();
        // #_ discards foo, but we still store it as Discard node
        // bar is the next form
        assert_eq!(forms.len(), 2);
        assert!(matches!(&forms[0], SExp::Discard(_, _)));
        assert!(matches!(&forms[1], SExp::Symbol(_, _)));
    }

    #[test]
    fn parse_anon_fn() {
        let forms = parse("#(+ %1 %2)").unwrap();
        if let SExp::AnonFn(items, _) = &forms[0] {
            assert_eq!(items.len(), 3);
        } else {
            panic!("expected anon fn");
        }
    }

    #[test]
    fn parse_nested_hiccup() {
        let forms = parse(r#"[:div {:class "foo"} [:span "hello"]]"#).unwrap();
        if let SExp::Vector(items, _) = &forms[0] {
            assert_eq!(items.len(), 3);
            if let SExp::Vector(inner, _) = &items[2] {
                assert_eq!(inner.len(), 2);
            } else {
                panic!("expected nested vector");
            }
        } else {
            panic!("expected vector");
        }
    }

    #[test]
    fn parse_line_tracking() {
        let forms = parse("(foo\n  bar\n  baz)").unwrap();
        if let SExp::List(items, span) = &forms[0] {
            assert_eq!(span.line, 1);
            assert_eq!(span.col, 1);
            if let SExp::Symbol(_, s) = &items[1] {
                assert_eq!(s.line, 2);
            }
            if let SExp::Symbol(_, s) = &items[2] {
                assert_eq!(s.line, 3);
            }
        }
    }

    #[test]
    fn parse_nil_true_false() {
        let forms = parse("nil true false").unwrap();
        assert_eq!(forms.len(), 3);
        assert!(matches!(&forms[0], SExp::Nil(_)));
        assert!(matches!(&forms[1], SExp::Bool(true, _)));
        assert!(matches!(&forms[2], SExp::Bool(false, _)));
    }

    #[test]
    fn parse_comment_line() {
        let forms = parse("; this is a comment\nfoo").unwrap();
        assert_eq!(forms.len(), 1);
        assert!(matches!(&forms[0], SExp::Symbol(_, _)));
    }

    #[test]
    fn parse_metadata() {
        let forms = parse("^:private foo").unwrap();
        assert!(matches!(&forms[0], SExp::Meta(_, _, _)));
    }

    #[test]
    fn parse_reader_conditional() {
        let forms = parse("#?(:clj 1 :cljs 2)").unwrap();
        if let SExp::ReaderConditional(items, _) = &forms[0] {
            assert_eq!(items.len(), 4);
        } else {
            panic!("expected reader conditional, got {:?}", forms[0]);
        }
    }

    #[test]
    fn parse_unicode_escape() {
        let forms = parse(r#""\u0041""#).unwrap();
        if let SExp::Str(s, _) = &forms[0] {
            assert_eq!(s, "A");
        } else {
            panic!("expected string");
        }
    }
}
