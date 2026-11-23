//! Blocks (a line, a heading with its section, a list item with its children, a code block)
//! that the editor can drag around, and merging a note that changed on disk while open.
use crate::md::{self, Block};
use std::collections::HashSet;
use std::ops::Range;

fn fence_flags(lines: &[&str]) -> Vec<bool> {
    // true for every line that is a fence or inside a fenced block.
    let mut out = vec![false; lines.len()];
    let mut open = false;
    for (i, l) in lines.iter().enumerate() {
        let f = md::classify(l).block == Block::Fence;
        out[i] = f || open;
        if f {
            open = !open;
        }
    }
    out
}

fn indent_of(l: &str) -> usize {
    let mut n = 0;
    for c in l.chars() {
        match c {
            ' ' => n += 1,
            '\t' => n += 4,
            _ => break,
        }
    }
    n
}

/// The lines making up the block that line `i` belongs to.
pub fn block_at(lines: &[&str], i: usize) -> Range<usize> {
    if lines.is_empty() {
        return 0..0;
    }
    let i = i.min(lines.len() - 1);
    let code = fence_flags(lines);
    if code[i] {
        if let Some(r) = fences(lines).into_iter().find(|r| r.contains(&i)) {
            return r;
        }
    }
    let c = md::classify(lines[i]);
    match c.block {
        Block::Heading(_) => i..crate::editor::section_end(lines, i),
        Block::Bullet | Block::Number | Block::Task(_) => {
            let ind = indent_of(lines[i]);
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() || indent_of(l) <= ind || matches!(md::classify(l).block, Block::Heading(_)) {
                    break;
                }
                j += 1;
            }
            i..j
        }
        _ => i..i + 1,
    }
}

/// Fenced code blocks, from the opening fence through the closing one.
fn fences(lines: &[&str]) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut open: Option<usize> = None;
    for (i, l) in lines.iter().enumerate() {
        if md::classify(l).block == Block::Fence {
            match open.take() {
                Some(a) => out.push(a..i + 1),
                None => open = Some(i),
            }
        }
    }
    if let Some(a) = open {
        out.push(a..lines.len());
    }
    out
}

/// The other lines, grouped into the pieces that move as one while a block is dragged:
/// code blocks and folded sections stay together, everything else is a line on its own.
pub fn units(lines: &[&str], skip: &Range<usize>, hidden: &HashSet<usize>) -> Vec<Range<usize>> {
    let code = fence_flags(lines);
    let mut out: Vec<Range<usize>> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if skip.contains(&i) {
            i = skip.end;
            continue;
        }
        let mut j = i + 1;
        if code[i] {
            j = block_at(lines, i).end.max(i + 1);
        }
        while j < lines.len() && hidden.contains(&j) && !skip.contains(&j) {
            j += 1;
        }
        out.push(i..j);
        i = j;
    }
    out
}

/// Move the lines `block` so they sit before unit `gap` of `units` (the units as given by
/// [`units`]). Returns the new text, the line the block now starts on, and for each new line
/// the old line it came from.
pub fn move_block(lines: &[&str], block: &Range<usize>, units: &[Range<usize>], gap: usize) -> (String, usize, Vec<usize>) {
    let mut order: Vec<usize> = Vec::with_capacity(lines.len());
    let mut at = 0;
    for (k, u) in units.iter().enumerate() {
        if k == gap {
            at = order.len();
            order.extend(block.clone());
        }
        order.extend(u.clone());
    }
    if gap >= units.len() {
        at = order.len();
        order.extend(block.clone());
    }
    let text = order.iter().map(|&i| lines[i]).collect::<Vec<_>>().join("\n");
    (text, at, order)
}

/// The unit index the block sits before right now (so dropping there changes nothing).
pub fn home_gap(block: &Range<usize>, units: &[Range<usize>]) -> usize {
    units.iter().position(|u| u.start >= block.end).unwrap_or(units.len())
}

/// Swap the block one unit up or down. None when it is already at that end.
pub fn nudge(lines: &[&str], block: &Range<usize>, hidden: &HashSet<usize>, down: bool) -> Option<(String, usize, Vec<usize>)> {
    let us = units(lines, block, hidden);
    let home = home_gap(block, &us);
    let gap = if down {
        if home >= us.len() {
            return None;
        }
        home + 1
    } else {
        if home == 0 {
            return None;
        }
        home - 1
    };
    Some(move_block(lines, block, &us, gap))
}

/* ---------- merging a note that changed on disk ---------- */

fn common_prefix(a: &[char], b: &[char]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| x == y).count()
}
fn common_suffix(a: &[char], b: &[char], max: usize) -> usize {
    a.iter().rev().zip(b.iter().rev()).take(max).take_while(|(x, y)| x == y).count()
}

/// The changed span: chars `[p, base.len() - s)` of `base` became `[p, new.len() - s)` of `new`.
fn span(base: &[char], new: &[char]) -> (usize, usize) {
    let p = common_prefix(base, new);
    let s = common_suffix(base, new, base.len().min(new.len()) - p);
    (p, s)
}

/// Three-way merge of a note: `base` is what we last loaded or saved, `ours` the text in the
/// editor, `theirs` what is on disk now. Edits in different places both survive; when they
/// overlap, ours wins (it is saved right after). Also maps the cursor (a char index into
/// `ours`) into the result.
pub fn merge(base: &str, ours: &str, theirs: &str, cursor: usize) -> (String, usize) {
    let b: Vec<char> = base.chars().collect();
    let o: Vec<char> = ours.chars().collect();
    let t: Vec<char> = theirs.chars().collect();
    let (tp, ts) = span(&b, &t);
    let (t0, t1) = (tp, b.len() - ts); // their change in base coordinates
    let t_new: String = t[tp..t.len() - ts].iter().collect();
    let t_delta = (t.len() - ts) as isize - t1 as isize;
    if ours == base {
        let c = if cursor <= t0 {
            cursor
        } else if cursor >= t1 {
            (cursor as isize + t_delta).max(0) as usize
        } else {
            tp + t_new.chars().count()
        };
        return (theirs.to_string(), c.min(t.len()));
    }
    let (op, os) = span(&b, &o);
    let (o0, o1) = (op, b.len() - os);
    if o1 <= t0 && !(o0 == o1 && o1 == t0 && t0 == t1) {
        // Ours sits before theirs.
        let mut out: String = o[..o.len() - os].iter().collect();
        out.extend(b[o1..t0].iter());
        out.push_str(&t_new);
        out.extend(b[t1..].iter());
        let c = if cursor <= o.len() - os { cursor } else { (cursor as isize + t_delta).max(0) as usize };
        return (out, c);
    }
    if t1 <= o0 {
        // Theirs sits before ours.
        let mut out: String = b[..t0].iter().collect();
        out.push_str(&t_new);
        out.extend(b[t1..o0].iter());
        out.extend(o[op..].iter());
        let c = if cursor < op { if cursor <= t0 { cursor } else { (cursor as isize + t_delta).max(0) as usize } } else { (cursor as isize + t_delta).max(0) as usize };
        return (out, c);
    }
    // Overlapping edits: keep ours.
    (ours.to_string(), cursor)
}

/* ---------- numbered lists ---------- */

/// `(indent bytes, number, delimiter, end of the number in bytes)` for a numbered list line.
fn num_item(l: &str) -> Option<(usize, u64, char, usize)> {
    let w = l.len() - l.trim_start_matches([' ', '\t']).len();
    let rest = &l[w..];
    let d = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    if d == 0 || d > 9 {
        return None;
    }
    let delim = rest[d..].chars().next()?;
    if delim != '.' && delim != ')' {
        return None;
    }
    if !rest[d + 1..].chars().next().map(|c| c.is_whitespace()).unwrap_or(true) {
        return None;
    }
    Some((w, rest[..d].parse().ok()?, delim, w + d))
}

/// Number every ordered list again, 1, 2, 3… from its lowest number, so moving, pasting or
/// deleting items keeps it in order. A list runs on across blank lines and deeper-indented
/// lines; anything else at its indent or less ends it. Code blocks are left alone.
pub fn renumber(lines: &[&str]) -> Vec<String> {
    struct Run {
        indent: usize,
        delim: char,
        idx: Vec<usize>,
    }
    let mut runs: Vec<Run> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    let mut fence = false;
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim_start();
        let is_fence = t.starts_with("```") || t.starts_with("~~~");
        if fence {
            if is_fence {
                fence = false;
            }
            continue;
        }
        if l.trim().is_empty() {
            continue;
        }
        if is_fence {
            fence = true;
        }
        let ind = indent_of(l);
        while stack.last().map(|&r| runs[r].indent > ind).unwrap_or(false) {
            stack.pop();
        }
        let m = if is_fence { None } else { num_item(l) };
        if let Some(&top) = stack.last() {
            if runs[top].indent == ind {
                if let Some((_, _, d, _)) = m {
                    if d == runs[top].delim {
                        runs[top].idx.push(i);
                        continue;
                    }
                }
                stack.pop();
            }
        }
        if let Some((_, _, d, _)) = m {
            runs.push(Run { indent: ind, delim: d, idx: vec![i] });
            stack.push(runs.len() - 1);
        }
    }
    let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    for r in &runs {
        let mut n = r.idx.iter().filter_map(|&i| num_item(lines[i]).map(|m| m.1)).min().unwrap_or(1);
        for &i in &r.idx {
            if let Some((w, _, _, end)) = num_item(lines[i]) {
                out[i] = format!("{}{}{}", &lines[i][..w], n, &lines[i][end..]);
            }
            n += 1;
        }
    }
    out
}

/// `text` with its ordered lists renumbered, and the char offsets `sel` moved to stay on
/// the same characters.
pub fn renumbered(text: &str, sel: (usize, usize)) -> (String, (usize, usize)) {
    let old: Vec<&str> = text.split('\n').collect();
    let new = renumber(&old);
    if old.iter().zip(&new).all(|(a, b)| *a == b.as_str()) {
        return (text.to_string(), sel);
    }
    let map = |c: usize| {
        let (mut s, mut ns) = (0, 0);
        for (i, l) in old.iter().enumerate() {
            let (ol, nl) = (l.chars().count(), new[i].chars().count());
            if c <= s + ol || i == old.len() - 1 {
                let col = c.saturating_sub(s);
                let w = ol - l.trim_start().chars().count();
                let nc = if col <= w { col } else { ((col as isize + nl as isize - ol as isize).max(w as isize) as usize).min(nl) };
                return ns + nc;
            }
            s += ol + 1;
            ns += nl + 1;
        }
        c
    };
    (new.join("\n"), (map(sel.0), map(sel.1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ls(s: &str) -> Vec<&str> {
        s.split('\n').collect()
    }

    #[test]
    fn blocks() {
        let t = "# A\ntext\n- one\n  - kid\n- two\n```\ncode\n```\n## B\nx";
        let l = ls(t);
        assert_eq!(block_at(&l, 0), 0..10);
        assert_eq!(block_at(&l, 1), 1..2);
        assert_eq!(block_at(&l, 2), 2..4);
        assert_eq!(block_at(&l, 3), 3..4);
        assert_eq!(block_at(&l, 6), 5..8);
        assert_eq!(block_at(&l, 5), 5..8);
        assert_eq!(block_at(&l, 7), 5..8);
        assert_eq!(block_at(&l, 8), 8..10);
    }

    #[test]
    fn moving() {
        let t = "a\n- one\n  - kid\nb\nc";
        let l = ls(t);
        let blk = block_at(&l, 1);
        let us = units(&l, &blk, &HashSet::new());
        assert_eq!(us, vec![0..1, 3..4, 4..5]);
        assert_eq!(home_gap(&blk, &us), 1);
        assert_eq!(move_block(&l, &blk, &us, 3).0, "a\nb\nc\n- one\n  - kid");
        assert_eq!(move_block(&l, &blk, &us, 3).2, vec![0, 3, 4, 1, 2]);
        assert_eq!(move_block(&l, &blk, &us, 0).1, 0);
        assert_eq!(nudge(&l, &blk, &HashSet::new(), true).unwrap().0, "a\nb\n- one\n  - kid\nc");
        assert_eq!(nudge(&l, &blk, &HashSet::new(), false).unwrap().0, "- one\n  - kid\na\nb\nc");
    }

    #[test]
    fn merging() {
        // Remote change only: cursor after it shifts.
        assert_eq!(merge("hello world", "hello world", "hello big world", 11), ("hello big world".into(), 15));
        assert_eq!(merge("hello world", "hello world", "hello big world", 2), ("hello big world".into(), 2));
        // Both, in different places.
        assert_eq!(merge("aaa\nbbb\nccc", "aaa!\nbbb\nccc", "aaa\nbbb\nccc?", 4).0, "aaa!\nbbb\nccc?");
        assert_eq!(merge("aaa\nbbb\nccc", "aaa\nbbb\nccc?", "aaa!\nbbb\nccc", 12), ("aaa!\nbbb\nccc?".into(), 13));
        // Same place: ours wins.
        assert_eq!(merge("abc", "aXc", "aYc", 2).0, "aXc");
    }

    #[test]
    fn renumbering() {
        let r = |t: &str| renumber(&t.split('\n').collect::<Vec<_>>()).join("\n");
        assert_eq!(r("3. c\n1. a\n   - kid\n\n2. b\ntext\n1. x"), "1. c\n2. a\n   - kid\n\n3. b\ntext\n1. x");
        assert_eq!(r("6. a\n5. b\n   2. in\n   1. in2\n7. c"), "5. a\n6. b\n   1. in\n   2. in2\n7. c");
        assert_eq!(r("```\n3. no\n```"), "```\n3. no\n```");
        let (t, s) = renumbered("10. a\n2. b", (9, 9));
        assert_eq!(t, "2. a\n3. b");
        assert_eq!(s, (8, 8));
    }
}
