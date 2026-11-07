//! A small markdown reader shared by the grid previews, the reader and the editor.
//! Everything works on byte ranges into the original line so the editor can style the
//! raw text in place.
use std::ops::Range;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Block {
    Heading(u8),
    Bullet,
    Number,
    Task(bool),
    Quote,
    Rule,
    Fence,
    Plain,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub block: Block,
    /// Leading whitespace, in bytes.
    pub indent: usize,
    /// The block marker (`## `, `- `, `1. `, `- [ ] `, `> `), after the indent.
    pub marker: Range<usize>,
    /// Text after the marker.
    pub content: Range<usize>,
}

impl Line {
    /// Nesting level for lists, counting a tab or two spaces as one step.
    pub fn level(&self, s: &str) -> usize {
        let ws = &s[..self.indent];
        let mut n = 0;
        for c in ws.chars() {
            n += if c == '\t' { 2 } else { 1 }
        }
        n / 2
    }
}

pub fn classify(s: &str) -> Line {
    let indent = s.len() - s.trim_start_matches([' ', '\t']).len();
    let t = &s[indent..];
    let mk = |block, mlen: usize| Line { block, indent, marker: indent..indent + mlen, content: indent + mlen..s.len() };
    let trimmed = t.trim_end();
    if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
        return mk(Block::Fence, 0);
    }
    if indent < 4 {
        let hashes = t.bytes().take_while(|&b| b == b'#').count();
        if (1..=6).contains(&hashes) && (t.len() == hashes || t.as_bytes()[hashes] == b' ') {
            let m = if t.len() > hashes { hashes + 1 } else { hashes };
            return mk(Block::Heading(hashes as u8), m);
        }
        let rc = trimmed.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        if rc.len() >= 3
            && (rc.chars().all(|c| c == '-') || rc.chars().all(|c| c == '*') || rc.chars().all(|c| c == '_'))
        {
            return mk(Block::Rule, 0);
        }
    }
    let b = t.as_bytes();
    if b.len() >= 2 && (b[0] == b'-' || b[0] == b'*' || b[0] == b'+') && b[1] == b' ' {
        if b.len() >= 5 && b[2] == b'[' && (b[3] == b' ' || b[3] == b'x' || b[3] == b'X') && b[4] == b']' {
            let m = if b.len() > 5 && b[5] == b' ' { 6 } else { 5 };
            if b.len() == 5 || b[5] == b' ' {
                return mk(Block::Task(b[3] != b' '), m);
            }
        }
        return mk(Block::Bullet, 2);
    }
    if b.len() == 1 && (b[0] == b'-' || b[0] == b'*') {
        return mk(Block::Bullet, 1);
    }
    let digits = b.iter().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && digits < 10 && b.len() > digits && (b[digits] == b'.' || b[digits] == b')') {
        if b.len() == digits + 1 {
            return mk(Block::Number, digits + 1);
        }
        if b[digits + 1] == b' ' {
            return mk(Block::Number, digits + 2);
        }
    }
    if b.first() == Some(&b'>') {
        let m = if b.get(1) == Some(&b' ') { 2 } else { 1 };
        return mk(Block::Quote, m);
    }
    mk(Block::Plain, 0)
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    pub link: bool,
    /// Markdown syntax characters (`**`, `` ` ``, `[`, `](url)`) rather than content.
    pub marker: bool,
}

#[derive(Clone, Debug)]
pub struct Span {
    pub range: Range<usize>,
    pub style: Style,
}

/// Split `s[range]` into styled spans. Unclosed markers stay plain text.
pub fn inline(s: &str, range: Range<usize>) -> Vec<Span> {
    let mut out = Vec::new();
    parse(s, range, Style::default(), &mut out);
    out
}

fn find_close(s: &str, from: usize, end: usize, pat: &str) -> Option<usize> {
    let hay = &s[from..end];
    let mut i = 0;
    while let Some(p) = hay[i..].find(pat) {
        let at = from + i + p;
        // `*` must not be part of `**` when looking for a single star.
        if pat == "*" && s[at..end].starts_with("**") {
            i += p + 2;
            continue;
        }
        if at > from {
            // `***` closing `**`: the inner `*` closes the italic first.
            if pat == "**" && s[at..end].starts_with("***") {
                return Some(at + 1);
            }
            return Some(at);
        }
        i += p + pat.len();
    }
    None
}

fn parse(s: &str, range: Range<usize>, base: Style, out: &mut Vec<Span>) {
    let end = range.end;
    let mut i = range.start;
    let mut plain = i;
    let push = |out: &mut Vec<Span>, r: Range<usize>, st: Style| {
        if r.start < r.end {
            out.push(Span { range: r, style: st })
        }
    };
    while i < end {
        let rest = &s[i..end];
        let c = rest.as_bytes()[0];
        let mut handled = false;
        if c == b'`' {
            if let Some(close) = s[i + 1..end].find('`') {
                let close = i + 1 + close;
                if close > i + 1 {
                    push(out, plain..i, base);
                    let m = Style { marker: true, ..base };
                    push(out, i..i + 1, m);
                    push(out, i + 1..close, Style { code: true, ..base });
                    push(out, close..close + 1, m);
                    i = close + 1;
                    plain = i;
                    handled = true;
                }
            }
        } else if rest.starts_with("**") || rest.starts_with("__") || rest.starts_with("~~") {
            let pat = &rest[..2];
            if let Some(close) = find_close(s, i + 2, end, pat) {
                push(out, plain..i, base);
                let mut inner = base;
                match pat {
                    "~~" => inner.strike = true,
                    _ => inner.bold = true,
                }
                let m = Style { marker: true, ..inner };
                push(out, i..i + 2, m);
                parse(s, i + 2..close, inner, out);
                push(out, close..close + 2, m);
                i = close + 2;
                plain = i;
                handled = true;
            }
        } else if c == b'*' || (c == b'_' && (i == 0 || !s.as_bytes()[i - 1].is_ascii_alphanumeric())) {
            let pat = if c == b'*' { "*" } else { "_" };
            if rest.len() > 1 && !rest.as_bytes()[1].is_ascii_whitespace() {
                if let Some(close) = find_close(s, i + 1, end, pat) {
                    push(out, plain..i, base);
                    let inner = Style { italic: true, ..base };
                    let m = Style { marker: true, ..inner };
                    push(out, i..i + 1, m);
                    parse(s, i + 1..close, inner, out);
                    push(out, close..close + 1, m);
                    i = close + 1;
                    plain = i;
                    handled = true;
                }
            }
        } else if c == b'[' {
            if let Some(cb) = s[i + 1..end].find("](") {
                let cb = i + 1 + cb;
                if let Some(cp) = s[cb + 2..end].find(')') {
                    let cp = cb + 2 + cp;
                    push(out, plain..i, base);
                    let link = Style { link: true, ..base };
                    let m = Style { marker: true, ..link };
                    push(out, i..i + 1, m);
                    parse(s, i + 1..cb, link, out);
                    push(out, cb..cp + 1, m);
                    i = cp + 1;
                    plain = i;
                    handled = true;
                }
            }
        }
        if !handled {
            i += rest.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
        }
    }
    push(out, plain..end, base);
}

/// The URL of a `[text](url)` link span starting at `at`, if any.
pub fn link_url(s: &str, marker: Range<usize>) -> Option<String> {
    let m = &s[marker];
    m.strip_prefix("](").and_then(|x| x.strip_suffix(')')).map(|x| x.to_string())
}

/// Plain text of a line with inline markers removed, for previews and search.
pub fn plain(s: &str) -> String {
    let l = classify(s);
    inline(s, l.content.clone())
        .into_iter()
        .filter(|sp| !sp.style.marker)
        .map(|sp| &s[sp.range])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blocks() {
        assert_eq!(classify("# Hi").block, Block::Heading(1));
        assert_eq!(classify("### Hi").marker, 0..4);
        assert_eq!(classify("#Hi").block, Block::Plain);
        assert_eq!(classify("- a").block, Block::Bullet);
        assert_eq!(classify("  - [x] done").block, Block::Task(true));
        assert_eq!(classify("- [ ] todo").content, 6..10);
        assert_eq!(classify("12. x").block, Block::Number);
        assert_eq!(classify("> q").block, Block::Quote);
        assert_eq!(classify("---").block, Block::Rule);
        assert_eq!(classify("```rust").block, Block::Fence);
    }
    #[test]
    fn inlines() {
        let s = "a **b *c*** `d` [e](f) ~~g~~";
        let txt = plain(s);
        assert_eq!(txt, "a b c d e g");
        let sp = inline(s, 0..s.len());
        assert!(sp.iter().any(|x| x.style.bold && x.style.italic && &s[x.range.clone()] == "c"));
        assert!(sp.iter().any(|x| x.style.code && &s[x.range.clone()] == "d"));
        assert_eq!(plain("2 * 3 * 4"), "2 * 3 * 4");
        assert_eq!(plain("snake_case_name"), "snake_case_name");
    }
}
