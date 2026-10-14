//! Block model → CommonMark / GFM text.

use crate::model::{Block, Inline, Marker, Style};

/// Render blocks to Markdown. Consecutive list items form one tight list;
/// every other block is separated by a blank line.
pub fn render(blocks: &[Block]) -> String {
    let mut out = String::with_capacity(blocks.len() * 128);
    let mut prev_was_item = false;
    for block in blocks {
        let is_item = matches!(block, Block::ListItem { .. });
        if !out.is_empty() {
            out.push_str(if is_item && prev_was_item {
                "\n"
            } else {
                "\n\n"
            });
        }
        match block {
            Block::Heading { level, inlines } => {
                for _ in 0..(*level).clamp(1, 6) {
                    out.push('#');
                }
                out.push(' ');
                push_inlines(&mut out, inlines, Ctx::Heading);
                // A trailing `#` run would be read as a closing sequence and dropped.
                if out.ends_with('#') {
                    out.insert(out.len() - 1, '\\');
                }
            }
            Block::Paragraph { inlines } => push_inlines(&mut out, inlines, Ctx::Paragraph),
            Block::ListItem {
                depth,
                marker,
                inlines,
            } => {
                // Nested content must start at the parent's content column:
                // 2 columns under "- ", 3 under "1. " (wider numbers are rare
                // and CommonMark tolerates up to 3 extra spaces).
                for _ in 0..*depth {
                    out.push_str(match marker {
                        Marker::Bullet => "  ",
                        Marker::Ordered(_) | Marker::OrderedParen(_) => "   ",
                    });
                }
                match marker {
                    Marker::Bullet => out.push_str("- "),
                    Marker::Ordered(n) => {
                        out.push_str(&n.to_string());
                        out.push_str(". ");
                    }
                    Marker::OrderedParen(n) => {
                        out.push_str(&n.to_string());
                        out.push_str(") ");
                    }
                }
                push_inlines(&mut out, inlines, Ctx::ListItem);
            }
            Block::Code { lines } => {
                let fence = code_fence(lines);
                out.push_str(&fence);
                out.push('\n');
                for l in lines {
                    out.push_str(l.trim_end());
                    out.push('\n');
                }
                out.push_str(&fence);
            }
            Block::Table { rows } => push_table(&mut out, rows),
        }
        prev_was_item = is_item;
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ctx {
    Paragraph,
    Heading,
    ListItem,
    TableCell,
}

/// Pick a fence longer than any backtick run inside the code.
fn code_fence(lines: &[String]) -> String {
    let longest = lines.iter().map(|l| longest_run(l, '`')).max().unwrap_or(0);
    "`".repeat(longest.max(2) + 1)
}

fn longest_run(s: &str, ch: char) -> usize {
    let (mut best, mut cur) = (0, 0);
    for c in s.chars() {
        if c == ch {
            cur += 1;
            best = best.max(cur);
        } else {
            cur = 0;
        }
    }
    best
}

fn push_table(out: &mut String, rows: &[Vec<Vec<Inline>>]) {
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    if cols == 0 {
        return;
    }
    for (i, row) in rows.iter().enumerate() {
        out.push('|');
        for c in 0..cols {
            out.push(' ');
            if let Some(cell) = row.get(c) {
                push_inlines(out, cell, Ctx::TableCell);
            }
            out.push_str(" |");
        }
        out.push('\n');
        if i == 0 {
            out.push('|');
            for _ in 0..cols {
                out.push_str(" --- |");
            }
            out.push('\n');
        }
    }
    out.pop(); // trailing newline; block separator adds its own
}

/// Merge adjacent inlines with identical style and link, then render each
/// group with its emphasis markers and link syntax.
fn push_inlines(out: &mut String, inlines: &[Inline], ctx: Ctx) {
    let merged = merge(inlines);
    let start = out.len();
    for (i, inl) in merged.iter().enumerate() {
        let at_start = i == 0;
        push_inline(out, inl, ctx, at_start);
    }
    // Paragraph text starting with what looks like a block marker must be
    // escaped so it stays a paragraph.
    if matches!(ctx, Ctx::Paragraph | Ctx::ListItem) {
        escape_block_start(out, start);
    }
}

fn merge(inlines: &[Inline]) -> Vec<Inline> {
    let mut merged: Vec<Inline> = Vec::with_capacity(inlines.len());
    for inl in inlines {
        if inl.text.is_empty() {
            continue;
        }
        match merged.last_mut() {
            Some(last)
                if last.link == inl.link
                    && (last.style == inl.style || inl.text.trim().is_empty()) =>
            {
                last.text.push_str(&inl.text)
            }
            Some(last)
                if last.link == inl.link && last.text.trim().is_empty() && last.link.is_none() =>
            {
                // Leading whitespace adopts the following style.
                let mut ws = std::mem::take(&mut last.text);
                ws.push_str(&inl.text);
                last.text = ws;
                last.style = inl.style;
            }
            _ => merged.push(inl.clone()),
        }
    }
    merged
}

fn push_inline(out: &mut String, inl: &Inline, ctx: Ctx, _at_start: bool) {
    let text = inl.text.as_str();
    let core = text.trim();
    if core.is_empty() {
        out.push_str(if text.is_empty() { "" } else { " " });
        return;
    }
    let lead = text.len() - text.trim_start().len();
    let trail = text.len() - text.trim_end().len();
    if lead > 0 {
        out.push(' ');
    }
    if inl.link.is_some() {
        out.push('[');
    }
    let Style { bold, italic, mono } = inl.style;
    // In a table cell `\|` escapes the pipe even inside a code span, so a
    // backslash before a pipe cannot be kept literal there: fall back to
    // escaped plain text, which keeps every character.
    let mono = mono && !(ctx == Ctx::TableCell && core.contains("\\|"));
    if mono {
        let ticks = "`".repeat(longest_run(core, '`') + 1);
        out.push_str(&ticks);
        if core.starts_with('`') || core.ends_with('`') {
            out.push(' ');
        }
        out.push_str(&if ctx == Ctx::TableCell {
            core.replace('|', "\\|")
        } else {
            core.to_owned()
        });
        if core.starts_with('`') || core.ends_with('`') {
            out.push(' ');
        }
        out.push_str(&ticks);
    } else {
        let marker = match (bold, italic) {
            (true, true) => "***",
            (true, false) => "**",
            (false, true) => "*",
            (false, false) => "",
        };
        out.push_str(marker);
        escape_into(out, core, ctx);
        out.push_str(marker);
    }
    if let Some(uri) = &inl.link {
        out.push_str("](");
        out.push_str(&escape_url(uri));
        out.push(')');
    }
    if trail > 0 {
        out.push(' ');
    }
}

/// Percent-encode every character that could end the link destination or
/// change the surrounding structure (whitespace, controls, brackets,
/// backslash, and `|`, which splits GFM table cells before inline parsing).
fn escape_url(uri: &str) -> String {
    let mut s = String::with_capacity(uri.len());
    for c in uri.chars() {
        match c {
            c if (c as u32) <= 0x20 || c == '\u{7f}' => s.push_str(&format!("%{:02X}", c as u32)),
            '(' | ')' | '<' | '>' | '|' | '\\' | '`' => s.push_str(&format!("%{:02X}", c as u32)),
            c => s.push(c),
        }
    }
    s
}

/// Escape characters with inline Markdown meaning.
fn escape_into(out: &mut String, text: &str, ctx: Ctx) {
    let chars: Vec<char> = text.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        match c {
            '\\' | '*' | '`' | '[' | ']' | '<' | '>' | '~' => {
                out.push('\\');
                out.push(c);
            }
            '_' => {
                // Intraword underscores never open emphasis (snake_case).
                let prev_word = i > 0 && chars[i - 1].is_alphanumeric();
                let next_word = chars.get(i + 1).is_some_and(|n| n.is_alphanumeric());
                if !(prev_word && next_word) {
                    out.push('\\');
                }
                out.push('_');
            }
            '|' if ctx == Ctx::TableCell => out.push_str("\\|"),
            c => out.push(c),
        }
    }
}

/// Escape a paragraph that would otherwise parse as a heading, list item,
/// thematic break or block quote.
fn escape_block_start(out: &mut String, start: usize) {
    let rest = &out[start..];
    let first = match rest.chars().next() {
        Some(c) => c,
        None => return,
    };
    let needs = match first {
        '#' | '=' => true,
        '-' | '+' => {
            rest[1..].starts_with(' ') || rest[1..].is_empty() || rest.chars().all(|c| c == first)
        }
        d if d.is_ascii_digit() => {
            let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
            let after = &rest[digits..];
            digits <= 9
                && (after.starts_with(". ")
                    || after.starts_with(") ")
                    || after == "."
                    || after == ")")
        }
        _ => false,
    };
    if !needs {
        return;
    }
    if first.is_ascii_digit() {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        out.insert(start + digits, '\\');
    } else {
        out.insert(start, '\\');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn plain(s: &str) -> Inline {
        Inline::plain(s)
    }
    fn styled(s: &str, bold: bool, italic: bool) -> Inline {
        Inline {
            text: s.into(),
            style: Style {
                bold,
                italic,
                mono: false,
            },
            link: None,
        }
    }
    fn para(inlines: Vec<Inline>) -> Block {
        Block::Paragraph { inlines }
    }

    #[test]
    fn heading_and_paragraph_separated_by_blank_line() {
        let md = render(&[
            Block::Heading {
                level: 2,
                inlines: vec![plain("Market overview")],
            },
            para(vec![plain("Widgets sell.")]),
        ]);
        assert_eq!(md, "## Market overview\n\nWidgets sell.\n");
    }

    #[test]
    fn emphasis_wraps_trimmed_text() {
        let md = render(&[para(vec![
            plain("This is "),
            styled("bold ", true, false),
            plain("and "),
            styled("italic", false, true),
            plain(" and "),
            styled("both", true, true),
            plain("."),
        ])]);
        assert_eq!(md, "This is **bold** and *italic* and ***both***.\n");
    }

    #[test]
    fn adjacent_same_style_runs_merge() {
        let md = render(&[para(vec![
            styled("Hello", true, false),
            styled(" world", true, false),
        ])]);
        assert_eq!(md, "**Hello world**\n");
    }

    #[test]
    fn lists_are_tight_and_nested() {
        let md = render(&[
            para(vec![plain("Intro")]),
            Block::ListItem {
                depth: 0,
                marker: Marker::Bullet,
                inlines: vec![plain("one")],
            },
            Block::ListItem {
                depth: 1,
                marker: Marker::Bullet,
                inlines: vec![plain("nested")],
            },
            Block::ListItem {
                depth: 0,
                marker: Marker::Ordered(3),
                inlines: vec![plain("three")],
            },
            Block::ListItem {
                depth: 0,
                marker: Marker::OrderedParen(1),
                inlines: vec![plain("paren")],
            },
            para(vec![plain("After")]),
        ]);
        assert_eq!(
            md,
            "Intro\n\n- one\n  - nested\n3. three\n1) paren\n\nAfter\n"
        );
    }

    #[test]
    fn special_characters_escaped() {
        let md = render(&[para(vec![plain(
            "a*b [c] `d` <e> ~f snake_case _x_ back\\slash",
        )])]);
        assert_eq!(
            md,
            "a\\*b \\[c\\] \\`d\\` \\<e\\> \\~f snake_case \\_x\\_ back\\\\slash\n"
        );
    }

    #[test]
    fn block_starts_escaped() {
        assert_eq!(
            render(&[para(vec![plain("# not a heading")])]),
            "\\# not a heading\n"
        );
        assert_eq!(
            render(&[para(vec![plain("1984. That year")])]),
            "1984\\. That year\n"
        );
        assert_eq!(
            render(&[para(vec![plain("- not a list")])]),
            "\\- not a list\n"
        );
        assert_eq!(render(&[para(vec![plain("---")])]), "\\---\n");
        assert_eq!(render(&[para(vec![plain("-5 degrees")])]), "-5 degrees\n");
        assert_eq!(render(&[para(vec![plain("3.5 percent")])]), "3.5 percent\n");
    }

    #[test]
    fn links_render_with_text() {
        let link = Some(Arc::<str>::from("https://example.com/a b"));
        let md = render(&[para(vec![
            plain("Visit "),
            Inline {
                text: "example.com".into(),
                style: Style::default(),
                link,
            },
            plain(" now."),
        ])]);
        assert_eq!(md, "Visit [example.com](https://example.com/a%20b) now.\n");
    }

    #[test]
    fn link_destinations_are_structure_safe() {
        let link = Some(Arc::<str>::from("https://x.test/a|b c\\d"));
        let md = render(&[Block::Table {
            rows: vec![
                vec![vec![plain("H")]],
                vec![vec![Inline {
                    text: "go".into(),
                    style: Style::default(),
                    link,
                }]],
            ],
        }]);
        assert!(md.contains("[go](https://x.test/a%7Cb%20c%5Cd)"), "{md}");
    }

    #[test]
    fn list_item_text_that_looks_like_a_marker_is_escaped() {
        let md = render(&[Block::ListItem {
            depth: 0,
            marker: Marker::Bullet,
            inlines: vec![plain("1. not nested")],
        }]);
        assert_eq!(md, "- 1\\. not nested\n");
    }

    #[test]
    fn table_with_header_and_pipe_escape() {
        let md = render(&[Block::Table {
            rows: vec![
                vec![vec![plain("Name")], vec![plain("Value")]],
                vec![vec![plain("a|b")], vec![plain("1")]],
                vec![vec![plain("short")]],
            ],
        }]);
        assert_eq!(
            md,
            "| Name | Value |\n| --- | --- |\n| a\\|b | 1 |\n| short |  |\n"
        );
    }

    #[test]
    fn code_block_fence_outgrows_backticks() {
        let md = render(&[Block::Code {
            lines: vec!["let a = 1;".into(), "  ```inner".into()],
        }]);
        assert_eq!(md, "````\nlet a = 1;\n  ```inner\n````\n");
    }

    #[test]
    fn inline_mono_uses_code_span() {
        let md = render(&[para(vec![
            plain("Run "),
            Inline {
                text: "cargo *test*".into(),
                style: Style {
                    mono: true,
                    ..Style::default()
                },
                link: None,
            },
        ])]);
        assert_eq!(md, "Run `cargo *test*`\n");
    }

    #[test]
    fn heading_trailing_hash_is_escaped() {
        let h = |t: &str| {
            render(&[Block::Heading {
                level: 2,
                inlines: vec![plain(t)],
            }])
        };
        assert_eq!(h("Issue #"), "## Issue \\#\n");
        assert_eq!(h("C#"), "## C\\#\n");
        assert_eq!(h("#"), "## \\#\n");
        assert_eq!(h("#1 result"), "## #1 result\n");
    }

    #[test]
    fn table_code_with_backslash_pipe_falls_back_to_text() {
        let mono = |t: &str| Inline {
            text: t.into(),
            style: Style {
                mono: true,
                ..Style::default()
            },
            link: None,
        };
        let md = render(&[Block::Table {
            rows: vec![vec![vec![plain("h")]], vec![vec![mono("a\\|b")]]],
        }]);
        assert_eq!(md, "| h |\n| --- |\n| a\\\\\\|b |\n");
        let md = render(&[Block::Table {
            rows: vec![vec![vec![plain("h")]], vec![vec![mono("a|b")]]],
        }]);
        assert_eq!(md, "| h |\n| --- |\n| `a\\|b` |\n");
    }

    #[test]
    fn empty_input_renders_empty() {
        assert_eq!(render(&[]), "");
    }
}
