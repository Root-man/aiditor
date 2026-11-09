//! Markdown as text properties: the markup stays in the text and is only hidden, so search
//! matches it and copying yields markdown. Tables are the exception: they're drawn as text.

use std::ops::Range;

use tree_sitter::Node;
use tree_sitter_md::MarkdownParser;

use crate::buffer::{Decoration, FaceRef, Prop};
use crate::settings::FaceName;

/// Where a rendered text's last top-level block starts: a byte of the source and a char of
/// the rendered text. Later text can change only how that block and what follows it render.
pub(crate) type Stable = (usize, usize);

/// The text that shows `text` as rendered markdown and its properties, in char offsets of that
/// text, sorted by start then end. Faces and hidden markup never overlap, and code blocks
/// decorate whole lines, so starts and ends both ascend. Words `is_file` accepts and bare web
/// URLs are links. In a tool call's `body` a fence line takes no row: it hides with the newline
/// joining it to its block. The text is `text` itself but for `tables`, where each top-level
/// pipe table is replaced by its drawn lines. Last comes where its last top-level block starts.
pub(crate) fn render(
    text: &str,
    thought: bool,
    body: bool,
    tables: bool,
    is_file: impl Fn(&str) -> bool,
) -> (String, Vec<(Range<usize>, Prop)>, Stable) {
    let mut parser = MarkdownParser::default();
    let parsed = parse(&mut parser, text, thought, body, Some(tables), &is_file);
    let stable = text[..parsed.stable].chars().count();
    if parsed.tables.is_empty() {
        return (text.into(), parsed.props, (parsed.stable, stable));
    }
    let mut shown = String::new();
    let mut drawn_props = Vec::new();
    // Each table's char range in `text` and the chars its drawing adds.
    let mut regions: Vec<(Range<usize>, isize)> = Vec::new();
    // Where the last table ended, in bytes and chars of `text` and chars of `shown`.
    let (mut from, mut source_at, mut shown_at) = (0, 0, 0);
    for table in &parsed.tables {
        let before = text[from..table.range.start].chars().count();
        shown.push_str(&text[from..table.range.start]);
        let (start, at) = (source_at + before, shown_at + before);
        let (drawn, props) = draw(&mut parser, text, table, &is_file);
        drawn_props.extend(
            props
                .into_iter()
                .map(|(r, p)| (at + r.start..at + r.end, p)),
        );
        let range = start..start + text[table.range.clone()].chars().count();
        let drawn_len = drawn.chars().count();
        (source_at, shown_at) = (range.end, at + drawn_len);
        regions.push((range.clone(), drawn_len as isize - range.len() as isize));
        shown.push_str(&drawn);
        from = table.range.end;
    }
    shown.push_str(&text[from..]);
    // Props come sorted by start, so the tables before each are counted once in passing.
    let (mut passed, mut shift) = (0, 0);
    let mut props: Vec<_> = (parsed.props.into_iter())
        .filter_map(|(r, p)| {
            while let Some((region, grew)) = regions.get(passed)
                && region.end <= r.start
            {
                (passed, shift) = (passed + 1, shift + grew);
            }
            if regions
                .get(passed)
                .is_some_and(|(region, _)| region.start < r.end)
            {
                return None;
            }
            let at = |i: usize| (i as isize + shift) as usize;
            Some((at(r.start)..at(r.end), p))
        })
        .chain(drawn_props)
        .collect();
    props.sort_by_key(|(r, _)| (r.start, r.end));
    let before = regions.iter().filter(|(region, _)| region.end <= stable);
    let shown_stable = (stable as isize + before.map(|(_, grew)| grew).sum::<isize>()) as usize;
    (shown, props, (parsed.stable, shown_stable))
}

/// A top-level pipe table's byte range and its header, delimiter and body rows'.
struct Table {
    range: Range<usize>,
    rows: Vec<Range<usize>>,
    delimiter: Option<Range<usize>>,
}

/// `text`'s properties, its inline links' labels in chars with their destinations, and, with
/// `blocks` of `Some(true)`, its top-level pipe tables. With `None` only inline markup renders,
/// as in a table cell, where `#` or a fence is text.
struct Parsed {
    props: Vec<(Range<usize>, Prop)>,
    links: Vec<(Range<usize>, String)>,
    tables: Vec<Table>,
    /// The byte its last top-level block's line starts at, the block found inside sections.
    stable: usize,
}

fn parse(
    parser: &mut MarkdownParser,
    text: &str,
    thought: bool,
    body: bool,
    blocks: Option<bool>,
    is_file: &impl Fn(&str) -> bool,
) -> Parsed {
    let mut found = Vec::new();
    let mut links = Vec::new();
    let Some(tree) = parser.parse(text.as_bytes(), None) else {
        return Parsed {
            props: Vec::new(),
            links,
            tables: found,
            stable: 0,
        };
    };
    let title = &[FaceRef::Syntax("text.title"), FaceRef::Bold][..];
    let mut faces: Vec<(Range<usize>, &[FaceRef])> = Vec::new();
    let mut hidden = Vec::new();
    let mut code = Vec::new();
    if thought {
        faces.push((0..text.len(), &[FaceRef::Face(FaceName::SessionThought)]));
    }
    let (blocks, tables) = (blocks.is_some(), blocks == Some(true));
    walk(
        tree.block_tree().root_node(),
        &mut |node| match node.kind() {
            _ if !blocks => {}
            "atx_heading" => {
                faces.push((node.byte_range(), title));
                // The `#`s and the space after them, up to the heading's text.
                if let Some(marker) = node.child(0) {
                    let end = (node.child(1)).map_or(marker.end_byte(), |text| text.start_byte());
                    hidden.push(marker.start_byte()..end);
                }
            }
            "setext_heading" => {
                for child in children(node) {
                    match child.kind() {
                        "paragraph" => faces.push((child.byte_range(), title)),
                        _ => hidden.push(child.byte_range()),
                    }
                }
            }
            "fenced_code_block" | "indented_code_block" => {
                let mut start = node.start_byte();
                while start < node.end_byte() {
                    let end = (text[start..node.end_byte()].find('\n'))
                        .map_or(node.end_byte(), |i| start + i + 1);
                    code.push(start..end);
                    start = end;
                }
                // A fence line shows as an empty line of the block, or in a body as none.
                let delimiters =
                    children(node).filter(|c| c.kind() == "fenced_code_block_delimiter");
                for (i, child) in delimiters.enumerate() {
                    let (start, line) = (child.start_byte(), &text[child.start_byte()..]);
                    let end = start + line.find('\n').unwrap_or(line.len());
                    hidden.push(match (body, i) {
                        (false, _) => start..end,
                        (true, 0) => start..(end + 1).min(text.len()),
                        (true, _) if text[..start].ends_with('\n') => start - 1..end,
                        (true, _) => start..end,
                    });
                }
            }
            "pipe_table"
                if tables
                    && (node.parent())
                        .is_some_and(|p| matches!(p.kind(), "section" | "document")) =>
            {
                found.push(Table {
                    range: node.byte_range(),
                    rows: children(node)
                        .filter(|r| matches!(r.kind(), "pipe_table_header" | "pipe_table_row"))
                        .map(|r| r.byte_range())
                        .collect(),
                    delimiter: children(node)
                        .find(|r| r.kind() == "pipe_table_delimiter_row")
                        .map(|r| r.byte_range()),
                });
            }
            _ => {}
        },
    );
    let link = &[FaceRef::Face(FaceName::Link), FaceRef::Underline][..];
    let mut spans = Vec::new();
    for inline in tree.inline_trees() {
        walk(inline.root_node(), &mut |node| {
            if node.kind() == "code_span" {
                spans.push(node.byte_range());
            }
            let (face, markup) = match node.kind() {
                "emphasis" => (&[FaceRef::Italic][..], "emphasis_delimiter"),
                "strong_emphasis" => (&[FaceRef::Bold][..], "emphasis_delimiter"),
                "code_span" => (
                    &[FaceRef::Face(FaceName::SessionCode)][..],
                    "code_span_delimiter",
                ),
                "uri_autolink" => {
                    faces.push((node.byte_range(), link));
                    hidden.push(node.start_byte()..node.start_byte() + 1);
                    hidden.push(node.end_byte() - 1..node.end_byte());
                    return;
                }
                "inline_link" => {
                    let Some(label) = children(node).find(|c| c.kind() == "link_text") else {
                        return;
                    };
                    faces.push((label.byte_range(), link));
                    if let Some(to) = children(node).find(|c| c.kind() == "link_destination") {
                        links.push((label.byte_range(), text[to.byte_range()].to_string()));
                    }
                    hidden.push(node.start_byte()..label.start_byte());
                    hidden.push(label.end_byte()..node.end_byte());
                    return;
                }
                _ => return,
            };
            faces.push((node.byte_range(), face));
            hidden.extend((children(node).filter(|c| c.kind() == markup)).map(|c| c.byte_range()));
        });
    }
    // Last, so a link's face wins over the code span or emphasis around it.
    faces.extend(
        (words(text).into_iter())
            .filter(|w| is_file(&text[w.clone()]))
            .map(|w| (w, link)),
    );
    // Code shows a URL as written, as GFM doesn't autolink there; file paths in code still link.
    faces.extend(
        (web_urls(text).into_iter())
            .filter(|u| !code.iter().chain(&spans).any(|c| c.contains(&u.start)))
            .map(|u| (u, link)),
    );
    // The old per-piece `covers` test never matched an empty range.
    faces.retain(|(r, _)| !r.is_empty());
    hidden.retain(|r| !r.is_empty());
    let mut cuts: Vec<usize> = (faces.iter().map(|(r, _)| r))
        .chain(&hidden)
        .chain(&code)
        .flat_map(|r| [r.start, r.end])
        .collect();
    cuts.sort_unstable();
    cuts.dedup();
    // Every range boundary is a cut, so the ranges covering a piece are those open at its start.
    let at = |pos: usize| cuts.binary_search(&pos).expect("every boundary is a cut");
    let (mut opened, mut closed) = (vec![Vec::new(); cuts.len()], vec![Vec::new(); cuts.len()]);
    for (i, (r, _)) in faces.iter().enumerate() {
        opened[at(r.start)].push(i);
        closed[at(r.end)].push(i);
    }
    let mut hiding = vec![0isize; cuts.len()];
    for r in &hidden {
        hiding[at(r.start)] += 1;
        hiding[at(r.end)] -= 1;
    }
    let mut line_starts = vec![false; cuts.len()];
    for r in &code {
        line_starts[at(r.start)] = true;
    }
    // Keyed by push index: inner faces come later, so folding in key order lets them win.
    let mut open = std::collections::BTreeMap::new();
    let mut hidden_open = 0;
    let mut props: Vec<(Range<usize>, Prop)> = Vec::new();
    for (k, piece) in cuts.windows(2).map(|w| w[0]..w[1]).enumerate() {
        for i in &closed[k] {
            open.remove(i);
        }
        open.extend(opened[k].iter().map(|&i| (i, faces[i].1)));
        hidden_open += hiding[k];
        let prop = match hidden_open > 0 {
            true => Prop::Invisible(String::new()),
            false => Prop::Named(open.values().copied().flatten().copied().collect()),
        };
        match props.last_mut() {
            // Not joined across a code line's start, so a face never ends inside a later line's decoration.
            Some((last, p)) if last.end == piece.start && *p == prop && !line_starts[k] => {
                last.end = piece.end
            }
            _ if prop != Prop::Named(Vec::new()) => props.push((piece, prop)),
            _ => {}
        }
    }
    let line = Prop::Line(Decoration::Background(FaceName::SessionBlock));
    props.extend(code.into_iter().map(|r| (r, line.clone())));
    // Byte offsets to char offsets.
    let mut chars = vec![0; text.len() + 1];
    for (n, (i, c)) in text.char_indices().enumerate() {
        chars[i..i + c.len_utf8()].fill(n);
        chars[i + c.len_utf8()] = n + 1;
    }
    let mut props: Vec<_> = (props.into_iter())
        .map(|(r, p)| (chars[r.start]..chars[r.end], p))
        .collect();
    props.sort_by_key(|(r, _)| (r.start, r.end));
    let links = (links.into_iter())
        .map(|(r, url)| (chars[r.start]..chars[r.end], url))
        .collect();
    let mut node = tree.block_tree().root_node();
    let mut stable = 0;
    while let Some(last) =
        (node.named_child_count().checked_sub(1)).and_then(|i| node.named_child(i as u32))
    {
        if last.kind() != "section" {
            stable = text[..last.start_byte()].rfind('\n').map_or(0, |i| i + 1);
            break;
        }
        node = last;
    }
    Parsed {
        props,
        links,
        tables: found,
        stable,
    }
}

/// A table cell as shown: its markup hidden, with its faces and links in its chars.
struct Cell {
    text: Vec<char>,
    faces: Vec<(Range<usize>, Vec<FaceRef>)>,
    links: Vec<(Range<usize>, String)>,
}

fn cell(parser: &mut MarkdownParser, source: &str, is_file: &impl Fn(&str) -> bool) -> Cell {
    let chars: Vec<char> = source.chars().collect();
    // Most cells are plain words, whose only markup can be file links: they skip the parse.
    if !source.contains(['*', '_', '`', '[', '<', '\\', '~', '&']) && !source.contains("http") {
        let link = vec![FaceRef::Face(FaceName::Link), FaceRef::Underline];
        let at = |byte: usize| source[..byte].chars().count();
        let faces = (words(source).into_iter())
            .filter(|w| is_file(&source[w.clone()]))
            .map(|w| (at(w.start)..at(w.end), link.clone()))
            .collect();
        return Cell {
            text: chars,
            faces,
            links: Vec::new(),
        };
    }
    let parsed = parse(parser, source, false, false, None, is_file);
    let mut hidden = vec![false; chars.len()];
    for (r, p) in &parsed.props {
        if matches!(p, Prop::Invisible(_)) {
            hidden[r.clone()].fill(true);
        }
    }
    // Each source char's index among the shown ones.
    let mut at = Vec::with_capacity(chars.len() + 1);
    let mut n = 0;
    for &h in &hidden {
        at.push(n);
        n += usize::from(!h);
    }
    at.push(n);
    let shown = |r: &Range<usize>| at[r.start]..at[r.end];
    Cell {
        text: (chars.iter().zip(&hidden))
            .filter(|(_, h)| !**h)
            .map(|(c, _)| *c)
            .collect(),
        faces: (parsed.props.iter())
            .filter_map(|(r, p)| match p {
                Prop::Named(refs) => Some((shown(r), refs.clone())),
                _ => None,
            })
            .collect(),
        links: (parsed.links.iter())
            .map(|(r, url)| (shown(r), url.clone()))
            .collect(),
    }
}

/// A table row's cells, split at unescaped `|`s, an escaped one kept as `|`, trimmed.
fn cells(row: &str) -> Vec<String> {
    let row = row.trim();
    let mut cells = vec![String::new()];
    let mut chars = row.chars().peekable();
    while let Some(c) = chars.next() {
        let last = cells.last_mut().expect("never empty");
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                last.push('|');
                chars.next();
            }
            '|' => cells.push(String::new()),
            c => last.push(c),
        }
    }
    if row.starts_with('|') {
        cells.remove(0);
    }
    if row.len() > 1 && row.ends_with('|') && !row.ends_with("\\|") {
        cells.pop();
    }
    cells.iter().map(|c| c.trim().to_string()).collect()
}

/// `table` drawn: a line per header and body row, each cell a space, its text padded to its
/// column's width as the delimiter row aligns it and a space, with the properties in its chars.
// shortcut: widths count chars, so wide characters misalign columns; measure display width if
// CJK or emoji tables show up.
fn draw(
    parser: &mut MarkdownParser,
    text: &str,
    table: &Table,
    is_file: &impl Fn(&str) -> bool,
) -> (String, Vec<(Range<usize>, Prop)>) {
    let rows: Vec<Vec<Cell>> = (table.rows.iter())
        .map(|r| {
            (cells(&text[r.clone()]).iter())
                .map(|c| cell(parser, c, is_file))
                .collect()
        })
        .collect();
    let aligns: Vec<(bool, bool)> = table.delimiter.as_ref().map_or(Vec::new(), |r| {
        (cells(&text[r.clone()]).iter())
            .map(|c| (c.starts_with(':'), c.ends_with(':')))
            .collect()
    });
    let columns = rows.first().map_or(0, Vec::len);
    let widths: Vec<usize> = (0..columns)
        .map(|c| {
            (rows.iter())
                .filter_map(|r| r.get(c))
                .map(|c| c.text.len())
                .max()
                .unwrap_or(0)
        })
        .collect();
    let mut edges = vec![0];
    for w in &widths {
        edges.push(edges.last().expect("starts with 0") + *w as isize + 2);
    }
    let (mut drawn, mut props) = (String::new(), Vec::new());
    let mut start = 0;
    for (i, row) in rows.iter().enumerate() {
        let decoration = Decoration::Table {
            edges: edges.clone(),
            first: i == 0,
        };
        props.push((start..start + 1, Prop::Line(decoration)));
        let base = match i {
            0 => vec![FaceRef::Face(FaceName::SessionBlock), FaceRef::Bold],
            _ => Vec::new(),
        };
        // Each char's faces and link, merged into runs below.
        let mut line: Vec<(char, Vec<FaceRef>, Option<&str>)> = Vec::new();
        let pad = |line: &mut Vec<_>, n| line.extend((0..n).map(|_| (' ', base.clone(), None)));
        for (c, &width) in widths.iter().enumerate() {
            let cell = row.get(c);
            let gap = width - cell.map_or(0, |c| c.text.len());
            let left = match aligns.get(c) {
                Some((true, true)) => gap / 2,
                Some((false, true)) => gap,
                _ => 0,
            };
            pad(&mut line, 1 + left);
            for (k, &ch) in cell.map_or(&[][..], |c| &c.text).iter().enumerate() {
                let cell = cell.expect("has text");
                let mut refs = base.clone();
                for (r, f) in &cell.faces {
                    if r.contains(&k) {
                        refs.extend(f);
                    }
                }
                let url = (cell.links.iter()).find(|(r, _)| r.contains(&k));
                line.push((ch, refs, url.map(|(_, u)| u.as_str())));
            }
            pad(&mut line, gap - left + 1);
        }
        let mut k = 0;
        while k < line.len() {
            let run = (line[k..].iter())
                .take_while(|(_, f, u)| (f, u) == (&line[k].1, &line[k].2))
                .count();
            let range = start + k..start + k + run;
            if !line[k].1.is_empty() {
                props.push((range.clone(), Prop::Named(line[k].1.clone())));
            }
            if let Some(url) = line[k].2 {
                props.push((range, Prop::Url(url.into())));
            }
            k += run;
        }
        drawn.extend(line.iter().map(|(c, ..)| c));
        start += line.len();
        if i + 1 < rows.len() || text[table.range.clone()].ends_with('\n') {
            drawn.push('\n');
            start += 1;
        }
    }
    (drawn, props)
}

/// The byte ranges of `text`'s words that could name a file: the runs between whitespace,
/// quotes, backticks, brackets, `*`, `,` and `;`, without a trailing `.`, `:`, `!` or `?`.
// shortcut: a path holding a bracket or `*` (Next.js `app/(group)/page.tsx`) never links;
// match whole indexed paths if agents name such files.
pub(crate) fn words(text: &str) -> Vec<Range<usize>> {
    let mut words = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices().chain([(text.len(), ' ')]) {
        match (c.is_whitespace() || "`'\"()[]<>{}*,;".contains(c), start) {
            (true, Some(s)) => {
                let word = text[s..i].trim_end_matches(['.', ':', '!', '?']);
                if !word.is_empty() {
                    words.push(s..s + word.len());
                }
                start = None;
            }
            (false, None) => start = Some(i),
            _ => {}
        }
    }
    words
}

/// Whether `url` is an `http` or `https` URL, the only kind a link opens.
pub(crate) fn is_web(url: &str) -> bool {
    let Some((scheme, rest)) = url.split_once("://") else {
        return false;
    };
    matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https") && !rest.is_empty()
}

/// The byte ranges of `text`'s bare web URLs, ended as GFM's extended autolinks are: at
/// whitespace, quotes, backticks, angle or square brackets and braces, without trailing
/// punctuation or a `)` that opens nothing in the URL, so `(…/Emacs_(text_editor))` keeps one.
pub(crate) fn web_urls(text: &str) -> Vec<Range<usize>> {
    let mut urls = Vec::new();
    let mut from = 0;
    while let Some(start) = text[from..].find("http").map(|i| from + i) {
        let stop = |c: char| c.is_whitespace() || "<>\"'`[]{}".contains(c);
        let end = text[start..].find(stop).map_or(text.len(), |i| start + i);
        from = end.max(start + 4);
        let mut url = &text[start..end];
        loop {
            let trimmed = url.trim_end_matches(['.', ',', ':', ';', '!', '?', '*', '_', '~']);
            let trimmed = match trimmed.strip_suffix(')') {
                Some(t) if trimmed.matches(')').count() > trimmed.matches('(').count() => t,
                _ => trimmed,
            };
            if trimmed.len() == url.len() {
                break;
            }
            url = trimmed;
        }
        let word_start = !text[..start].ends_with(|c: char| c.is_alphanumeric());
        if word_start && is_web(url) {
            urls.push(start..start + url.len());
        }
    }
    urls
}

/// The URL of the link holding byte `at` of `line`: an inline link's destination, an
/// autolink's text, else a bare web URL word.
// shortcut: parses one line, so a link whose label wraps opens only from its destination's
// line; parse the whole block if that shows up.
pub(crate) fn url_at(line: &str, at: usize) -> Option<String> {
    let tree = MarkdownParser::default().parse(line.as_bytes(), None)?;
    let mut url = None;
    for inline in tree.inline_trees() {
        walk(inline.root_node(), &mut |node| {
            if url.is_some() || !node.byte_range().contains(&at) {
                return;
            }
            url = match node.kind() {
                "uri_autolink" => Some(line[node.start_byte() + 1..node.end_byte() - 1].into()),
                "inline_link" => (children(node).find(|c| c.kind() == "link_destination"))
                    .map(|d| line[d.byte_range()].into()),
                _ => None,
            };
        });
    }
    url.or_else(|| {
        let found = web_urls(line).into_iter().find(|u| u.contains(&at));
        found.map(|u| line[u].into())
    })
}

fn children<'a>(node: Node<'a>) -> impl Iterator<Item = Node<'a>> {
    (0..node.child_count()).filter_map(move |i| node.child(i))
}

/// Calls `f` on `node` and every node below it, parents first.
fn walk<'a>(node: Node<'a>, f: &mut impl FnMut(Node<'a>)) {
    f(node);
    for child in children(node) {
        walk(child, f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Face;
    use crate::settings::Settings;
    use crate::syntax::face as syntax_face;

    /// What the text shows with its hidden ranges dropped, and each visible run with its face,
    /// for faces that aren't the default.
    fn shown(text: &str, thought: bool) -> (String, Vec<(String, Face)>) {
        let theme = Settings::default().theme;
        let props = render(text, thought, false, false, |w| w.starts_with("src/")).1;
        let chars: Vec<char> = text.chars().collect();
        let hidden = |i: usize| {
            (props.iter()).any(|(r, p)| matches!(p, Prop::Invisible(_)) && r.contains(&i))
        };
        let shown = (0..chars.len())
            .filter(|&i| !hidden(i))
            .map(|i| chars[i])
            .collect();
        let faces = (props.iter())
            .filter_map(|(r, p)| match p {
                Prop::Named(refs) => Some((chars[r.clone()].iter().collect(), theme.resolve(refs))),
                _ => None,
            })
            .collect();
        (shown, faces)
    }

    fn face(fg: Option<u32>, bold: bool, italic: bool, underline: bool) -> Face {
        Face {
            fg,
            bg: None,
            bold,
            italic,
            underline,
            strike: false,
            boxed: None,
        }
    }

    #[test]
    fn inline_markup_is_hidden_and_faced() {
        let theme = Settings::default().theme;
        let (text, faces) = shown(
            "Some **bold**, *it*, `code` and [docs](https://x.dev).",
            false,
        );
        assert_eq!(text, "Some bold, it, code and docs.");
        let link = theme.face(FaceName::Link).fg;
        assert_eq!(
            faces,
            [
                ("bold".into(), face(None, true, false, false)),
                ("it".into(), face(None, false, true, false)),
                ("code".into(), theme.face(FaceName::SessionCode)),
                ("docs".into(), face(link, false, false, true)),
            ]
        );
    }

    #[test]
    fn headings_and_autolinks() {
        let title = syntax_face(&Settings::default().theme.syntax, "text.title");
        let title = title.and_then(|f| f.fg);
        let (text, faces) = shown("# Title\n\nSub\n===\n\nsee <https://x.dev>\n", false);
        assert_eq!(text, "Title\n\nSub\n\n\nsee https://x.dev\n");
        assert_eq!(
            faces[0],
            ("Title\n".into(), face(title, true, false, false))
        );
        assert_eq!(faces[1].0, "Sub\n");
        assert_eq!(faces[2].0, "https://x.dev");
    }

    #[test]
    fn code_blocks_decorate_their_lines_and_hide_fences() {
        let text = "```rust\nfn main() {}\n```\nafter\n\n    indented\n";
        let props = render(text, false, false, false, |_| false).1;
        let lines: Vec<_> = (props.iter())
            .filter(|(_, p)| matches!(p, Prop::Line(_)))
            .map(|(r, _)| text.chars().skip(r.start).take(r.len()).collect::<String>())
            .collect();
        assert_eq!(
            lines,
            ["```rust\n", "fn main() {}\n", "```\n", "    indented\n"]
        );
        assert_eq!(
            shown(text, false).0,
            "\nfn main() {}\n\nafter\n\n    indented\n"
        );
    }

    #[test]
    fn a_thought_is_dim_under_other_faces() {
        let dim = Settings::default().theme.face(FaceName::SessionThought).fg;
        let (_, faces) = shown("I *think*", true);
        assert_eq!(
            faces,
            [
                ("I ".into(), face(dim, false, false, false)),
                ("think".into(), face(dim, false, true, false)),
            ]
        );
    }

    #[test]
    fn inner_faces_win_and_code_lines_stay_apart() {
        let theme = Settings::default().theme;
        let dim = theme.face(FaceName::SessionThought).fg;
        let title = syntax_face(&theme.syntax, "text.title").and_then(|f| f.fg);
        let uri = theme.face(FaceName::Link).fg;
        let text = "# T **s [k](https://x.dev) e**\n\n```\nx\ny\n```\n";
        let (_, faces) = shown(text, true);
        let title = face(title, true, false, false);
        assert_eq!(
            faces,
            [
                ("T ".into(), title),
                ("s ".into(), title),
                ("k".into(), face(uri, true, false, true)),
                (" e".into(), title),
                ("\n".into(), title),
                ("\n".into(), face(dim, false, false, false)),
                ("\n".into(), face(dim, false, false, false)),
                ("x\n".into(), face(dim, false, false, false)),
                ("y\n".into(), face(dim, false, false, false)),
                ("\n".into(), face(dim, false, false, false)),
            ]
        );
    }

    #[test]
    fn words_that_name_files_are_links() {
        assert_eq!(
            words("see `src/a.rs:12`, (x.rs). é src/b.rs: c.rs?!"),
            [0..3, 5..16, 20..24, 27..29, 30..38, 40..44]
        );
        let theme = Settings::default().theme;
        let link = face(theme.face(FaceName::Link).fg, false, false, true);
        let (text, faces) = shown("In `src/a.rs:3` and *src/b.rs*, not gone.rs.", false);
        assert_eq!(text, "In src/a.rs:3 and src/b.rs, not gone.rs.");
        assert_eq!(
            faces,
            [
                ("src/a.rs:3".into(), link),
                (
                    "src/b.rs".into(),
                    Face {
                        italic: true,
                        ..link
                    }
                )
            ]
        );
    }

    #[test]
    fn bare_web_urls_are_links() {
        let link = face(
            Settings::default().theme.face(FaceName::Link).fg,
            false,
            false,
            true,
        );
        let (_, faces) = shown("See https://x.dev/docs. Not ftp://x.dev/file", false);
        assert_eq!(faces, [("https://x.dev/docs".into(), link)]);
        let wiki = "https://en.wikipedia.org/wiki/Emacs_(text_editor)";
        let (_, faces) = shown(&format!("({wiki}), `https://x.dev/code`"), false);
        assert_eq!(faces[0], (wiki.into(), link));
        assert!(
            !faces.iter().any(|(t, f)| t.contains("code") && f.underline),
            "not in code"
        );
        let (_, faces) = shown("```\nhttps://x.dev/block\n```\n", false);
        assert!(
            !faces.iter().any(|(_, f)| f.underline),
            "nor in a code block"
        );
        assert_eq!(
            web_urls("xhttps://a.b [https://c.d] https://e.f/(g)h). https://"),
            [14..25, 27..43]
        );
    }

    #[test]
    fn url_at_finds_link_destinations_autolinks_and_bare_urls() {
        let line =
            "a [docs](https://x.dev) <http://y.dev/a> https://z.dev/b. [f](file:///bin/sh) c";
        let at = |s: &str| line.find(s).unwrap();
        assert_eq!(url_at(line, at("docs")).as_deref(), Some("https://x.dev"));
        assert_eq!(url_at(line, at("y.dev")).as_deref(), Some("http://y.dev/a"));
        assert_eq!(
            url_at(line, at("z.dev")).as_deref(),
            Some("https://z.dev/b")
        );
        assert_eq!(url_at(line, at("f]")).as_deref(), Some("file:///bin/sh"));
        assert_eq!(url_at(line, 0), None);
        assert_eq!(url_at(line, line.len() - 1), None);
        assert!(is_web("HTTPS://x.dev") && !is_web("file:///bin/sh") && !is_web("src/a.rs"));
    }

    /// `text` rendered with tables drawn: its shown text and each table row's decoration.
    fn drawn(text: &str) -> (String, Vec<(Range<usize>, Prop)>) {
        let (text, props, _) = render(text, false, false, true, |w| w.starts_with("src/"));
        (text, props)
    }

    /// The text of `shown`'s chars in `r`.
    fn at(shown: &str, r: &Range<usize>) -> String {
        shown.chars().skip(r.start).take(r.len()).collect()
    }

    fn rows(props: &[(Range<usize>, Prop)]) -> Vec<(usize, Decoration)> {
        (props.iter())
            .filter_map(|(r, p)| match p {
                Prop::Line(d @ Decoration::Table { .. }) => Some((r.start, d.clone())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn tables_draw_aligned_columns() {
        let (shown, props) = drawn("| a | b |\n|---|--:|\n| x | 10 |\n");
        assert_eq!(shown, " a   b \n x  10 \n");
        let row = |first| Decoration::Table {
            edges: vec![0, 3, 7],
            first,
        };
        assert_eq!(rows(&props), [(0, row(true)), (8, row(false))]);
        let (shown, _) = drawn("| a | b |\n|:-:|:--|\n| xyz | 1 |\n");
        assert_eq!(shown, "  a   b \n xyz  1 \n", "centred, then left");
        let (shown, _) = drawn("| a | b | c |\n|---|---|---|\n| 1 | 2 |\n| 1 | 2 | 3 | 4 |\n");
        assert_eq!(shown, " a  b  c \n 1  2    \n 1  2  3 \n", "ragged rows");
        let (shown, _) = drawn("| a \\| b | c |\n|---|---|\n");
        assert_eq!(shown, " a | b  c \n", "an escaped pipe");
        let (shown, _) = drawn("| # | h |\n|---|---|\n| # *x* | ``` |\n| - y | 1. z |\n");
        assert_eq!(
            shown, " #    h    \n # x  ```  \n - y  1. z \n",
            "cells render inline markup only, so block markers stay"
        );
    }

    #[test]
    fn a_streaming_table_widens_as_rows_arrive() {
        let (shown, _) = drawn("| a | b |\n|---|---|\n");
        assert_eq!(shown, " a  b \n");
        let (shown, _) = drawn("| a | b |\n|---|---|\n| long | x |");
        assert_eq!(shown, " a     b \n long  x ");
    }

    #[test]
    fn table_cells_render_their_markup() {
        let (shown, props) = drawn("| k |\n|---|\n| **hot** path |\n| [docs](https://x.dev) |\n");
        assert_eq!(shown, " k        \n hot path \n docs     \n");
        let named = |name: &str| {
            (props.iter())
                .filter(|(r, p)| at(&shown, r) == name && matches!(p, Prop::Named(_)))
                .map(|(_, p)| p.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(named("hot"), [Prop::Named(vec![FaceRef::Bold])]);
        let header = vec![FaceRef::Face(FaceName::SessionBlock), FaceRef::Bold];
        assert_eq!(
            named(" k        "),
            [Prop::Named(header)],
            "the header's whole width"
        );
        let link = vec![FaceRef::Face(FaceName::Link), FaceRef::Underline];
        assert_eq!(named("docs"), [Prop::Named(link)]);
        let url = (props.iter()).find(|(_, p)| matches!(p, Prop::Url(_)));
        let (r, p) = url.expect("the link's destination");
        assert_eq!(
            (at(&shown, r), p),
            ("docs".into(), &Prop::Url("https://x.dev".into()))
        );
        let ascend =
            |w: &[(Range<usize>, Prop)]| w[0].0.start <= w[1].0.start && w[0].0.end <= w[1].0.end;
        assert!(props.windows(2).all(ascend), "{props:?}");
    }

    #[test]
    fn text_around_a_table_keeps_its_rendering() {
        let text = "**a**\n\n| x |\n|---|\n| y |\n\n**b** src/b.rs\n";
        let (shown, props) = drawn(text);
        assert_eq!(shown, "**a**\n\n x \n y \n\n**b** src/b.rs\n");
        let faces: Vec<_> = (props.iter())
            .filter(|(_, p)| !matches!(p, Prop::Line(_)))
            .map(|(r, p)| (at(&shown, r), p.clone()))
            .collect();
        let hidden = || Prop::Invisible(String::new());
        let link = Prop::Named(vec![FaceRef::Face(FaceName::Link), FaceRef::Underline]);
        assert_eq!(
            faces,
            [
                ("**".into(), hidden()),
                ("a".into(), Prop::Named(vec![FaceRef::Bold])),
                ("**".into(), hidden()),
                (
                    " x ".into(),
                    Prop::Named(vec![FaceRef::Face(FaceName::SessionBlock), FaceRef::Bold])
                ),
                ("**".into(), hidden()),
                ("b".into(), Prop::Named(vec![FaceRef::Bold])),
                ("**".into(), hidden()),
                ("src/b.rs".into(), link),
            ]
        );
        let quoted = "> | a |\n> |---|\n";
        assert_eq!(drawn(quoted).0, quoted, "a table inside a quote stays text");
        let listed = "- item:\n\n  | a |\n  |---|\n  | b |\n";
        assert_eq!(drawn(listed).0, listed, "and inside a list item");
        let table = "| a |\n|---|\n";
        assert_eq!(
            render(table, false, false, false, |_| false).0,
            table,
            "tables off"
        );
    }

    #[test]
    fn offsets_are_chars_and_props_ascend() {
        let text = "é **ünï** `ç`\n\n```\nß\nx\n```\n";
        let (shown, _) = shown(text, false);
        assert_eq!(shown, "é ünï ç\n\n\nß\nx\n\n");
        // A thought's dim face runs through the code lines too.
        for thought in [false, true] {
            let props = render(text, thought, false, false, |_| false).1;
            let ascend = |w: &[(Range<usize>, Prop)]| {
                w[0].0.start <= w[1].0.start && w[0].0.end <= w[1].0.end
            };
            assert!(props.windows(2).all(ascend), "{props:?}");
        }
    }

    /// Where `text`'s last top-level block starts: its source text and rendered text from there.
    fn stable(text: &str) -> (String, String) {
        let (shown, _, (byte, char)) = render(text, false, false, true, |_| false);
        (text[byte..].into(), shown.chars().skip(char).collect())
    }

    #[test]
    fn the_stable_point_is_the_last_top_level_blocks_start() {
        let same = |tail: &str| (tail.to_string(), tail.to_string());
        assert_eq!(
            stable("one\n\ntwo é\nmore"),
            same("two é\nmore"),
            "a paragraph"
        );
        assert_eq!(stable("one"), same("one"), "the only block");
        assert_eq!(stable(""), same(""));
        // Inside the sections headings open, down to the last.
        assert_eq!(stable("# A\n\na\n\n## B\n\nlast"), same("last"));
        assert_eq!(stable("# A\n\na\n\n## B\n"), same("## B\n"), "a heading");
        assert_eq!(
            stable("a\n\n- x\n- y\n\n  z"),
            same("- x\n- y\n\n  z"),
            "a list"
        );
        assert_eq!(
            stable("a\n\n```\nb\n\nc"),
            same("```\nb\n\nc"),
            "an open fence"
        );
        assert_eq!(stable("a\n  b"), same("a\n  b"), "a lazy continuation");
        assert_eq!(stable("a\n\nb\n==="), same("b\n==="), "a setext heading");
    }

    #[test]
    fn the_stable_point_counts_the_chars_drawn_tables_add() {
        let table = "| a | b |\n|---|--:|\n| 1 | 22 |\n";
        let (source, shown) = stable(&format!("{table}\nafter é"));
        assert_eq!((source.as_str(), shown.as_str()), ("after é", "after é"));
        let text = format!("x\n\n{table}\nmid\n\n{table}");
        let (source, shown) = stable(&text);
        assert_eq!(source, table, "a table last");
        assert_eq!(shown, render(table, false, false, true, |_| false).0);
    }
}
