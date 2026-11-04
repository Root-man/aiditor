//! Markdown as text properties: the markup stays in the text and is only hidden, so search
//! matches it and copying yields markdown.

use std::ops::Range;

use tree_sitter::Node;
use tree_sitter_md::MarkdownParser;

use crate::buffer::{Decoration, FaceRef, Prop};
use crate::settings::FaceName;

/// The properties that show `text` as rendered markdown, in char offsets of `text`, sorted by
/// start then end. Faces and hidden markup never overlap, and code blocks decorate whole lines,
/// so starts and ends both ascend. Words `is_file` accepts and bare web URLs are links. In a tool call's `body` a
/// fence line takes no row: it hides with the newline joining it to its block.
pub(crate) fn render(
    text: &str,
    thought: bool,
    body: bool,
    is_file: impl Fn(&str) -> bool,
) -> Vec<(Range<usize>, Prop)> {
    let Some(tree) = MarkdownParser::default().parse(text.as_bytes(), None) else {
        return Vec::new();
    };
    let title = &[FaceRef::Syntax("text.title"), FaceRef::Bold][..];
    let mut faces: Vec<(Range<usize>, &[FaceRef])> = Vec::new();
    let mut hidden = Vec::new();
    let mut code = Vec::new();
    if thought {
        faces.push((0..text.len(), &[FaceRef::Face(FaceName::SessionThought)]));
    }
    walk(
        tree.block_tree().root_node(),
        &mut |node| match node.kind() {
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
    props
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
        let props = render(text, thought, false, |w| w.starts_with("src/"));
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
        let props = render(text, false, false, |_| false);
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

    #[test]
    fn offsets_are_chars_and_props_ascend() {
        let text = "é **ünï** `ç`\n\n```\nß\nx\n```\n";
        let (shown, _) = shown(text, false);
        assert_eq!(shown, "é ünï ç\n\n\nß\nx\n\n");
        // A thought's dim face runs through the code lines too.
        for thought in [false, true] {
            let props = render(text, thought, false, |_| false);
            let ascend = |w: &[(Range<usize>, Prop)]| {
                w[0].0.start <= w[1].0.start && w[0].0.end <= w[1].0.end
            };
            assert!(props.windows(2).all(ascend), "{props:?}");
        }
    }
}
