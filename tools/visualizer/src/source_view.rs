//! Renders `.intent` source as syntax-highlighted, line-anchored HTML.
//!
//! Uses the real lexer (not a regex approximation) so highlighting never
//! drifts from what the parser actually accepts. Tokens are classed by
//! kind; the gaps between tokens (whitespace, `//` comments — both trivia
//! the lexer skips) are re-inserted verbatim from the original source so
//! comments still render, just uncolored beyond their own `//` class.

use intent_lang_syntax::lexer::Token;
use logos::Logos;

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn token_class(tok: &Token) -> &'static str {
    use Token::*;
    match tok {
        Type | Enum | Intent | Safety | Theorem | Axiom | Function | Import | Require | Ensure
        | Invariant | Forall | Exists | If | Then | Else | After | As | Modifies | Reject
        | Example | Goal | Rationale | Stakeholder | Measure | RealizedBy | Coverage
        | Dimensions => "kw",
        True | False => "lit",
        IntLit(_) => "num",
        StringLit(_) => "str",
        At => "ann",
        Implies | EqEq | BangEq | LtEq | GtEq | AmpAmp | PipePipe | Plus | Minus | Star | Slash
        | Percent | Bang | Lt | Gt | Prime => "op",
        Ident(_) => "ident",
        _ => "punct",
    }
}

/// Push `text` (already known to carry a single highlight `class`, or none)
/// onto `lines`, splitting on any embedded newlines so each physical line
/// gets a self-contained, well-formed set of `<span>` tags.
fn push_text(lines: &mut Vec<String>, text: &str, class: Option<&'static str>) {
    for (i, part) in text.split('\n').enumerate() {
        if i > 0 {
            lines.push(String::new());
        }
        if part.is_empty() {
            continue;
        }
        let escaped = html_escape(part);
        let line = lines.last_mut().expect("lines always has >=1 entry");
        match class {
            Some(cls) => line.push_str(&format!("<span class=\"tok-{cls}\">{escaped}</span>")),
            None => line.push_str(&escaped),
        }
    }
}

/// Render `source` as an HTML fragment: one `<div class="src-line" id="L{n}">`
/// per line, syntax-classed spans inside. Caller wraps this in a scrollable,
/// monospace container.
pub fn render_source_html(source: &str) -> String {
    // (text, class) pieces in source order: real tokens classed by kind,
    // trivia gaps (whitespace / comments, which the lexer skips) unclassed
    // except for the `//...` portion of a comment.
    let mut pieces: Vec<(&str, Option<&'static str>)> = Vec::new();
    let mut last_end = 0usize;
    let mut lexer = Token::lexer(source);
    while let Some(tok) = lexer.next() {
        let span = lexer.span();
        if span.start > last_end {
            pieces.push((&source[last_end..span.start], None));
        }
        let class = match &tok {
            Ok(t) => Some(token_class(t)),
            Err(()) => None,
        };
        pieces.push((&source[span.start..span.end], class));
        last_end = span.end;
    }
    if last_end < source.len() {
        pieces.push((&source[last_end..], None));
    }

    let mut lines: Vec<String> = vec![String::new()];
    for (text, class) in pieces {
        if class.is_none() {
            if let Some(idx) = text.find("//") {
                push_text(&mut lines, &text[..idx], None);
                push_text(&mut lines, &text[idx..], Some("cmt"));
                continue;
            }
        }
        push_text(&mut lines, text, class);
    }
    // A trailing `\n` terminates the last real line rather than starting a
    // new (always-empty) one — drop the resulting phantom entry so line
    // counts match what an editor / `wc -l` would show.
    if source.ends_with('\n') && lines.len() > 1 {
        lines.pop();
    }

    let mut out = String::with_capacity(source.len() * 2);
    for (idx, line) in lines.iter().enumerate() {
        let n = idx + 1;
        let body = if line.is_empty() { " " } else { line.as_str() };
        out.push_str(&format!(
            "<div class=\"src-line\" id=\"L{n}\"><span class=\"ln\">{n}</span><span class=\"code\">{body}</span></div>\n"
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_each_line_with_anchor() {
        let html = render_source_html("intent Foo(a: A) {\n  ensure a.x' == 1\n}\n");
        assert!(html.contains("id=\"L1\""));
        assert!(html.contains("id=\"L2\""));
        assert!(html.contains("tok-kw"));
    }

    #[test]
    fn keeps_comment_text_visible() {
        let html = render_source_html("// hello world\nintent A() {}\n");
        assert!(html.contains("hello world"));
        assert!(html.contains("tok-cmt"));
    }

    #[test]
    fn escapes_angle_brackets() {
        let html = render_source_html("// a < b && c > d\n");
        assert!(html.contains("&lt;"));
        assert!(html.contains("&gt;"));
    }

    #[test]
    fn line_count_matches_trailing_newline() {
        let html = render_source_html("a\nb\n");
        assert!(html.contains("id=\"L1\""));
        assert!(html.contains("id=\"L2\""));
        assert!(!html.contains("id=\"L3\""));
    }
}
