//! Tree-sitter parsing of a visited file and highlighting of the lines a window shows.

use crate::buffer::Face;
use ropey::Rope;
use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::ops::Range;
use std::path::Path;
use std::sync::LazyLock;
use tree_sitter::{
    InputEdit, Language, Node, Parser, Point, Query, QueryCursor, StreamingIterator, Tree,
};

/// A language and its highlights query.
struct Grammar {
    language: Language,
    highlights: Query,
}

/// Compiles `queries` joined, later patterns overriding earlier ones, as Zed and Helix extend
/// JavaScript's query for TypeScript.
fn grammar(language: Language, queries: &[&str]) -> Grammar {
    let highlights = Query::new(&language, &queries.concat()).expect("bundled queries compile");
    Grammar {
        language,
        highlights,
    }
}

const JS: &str = tree_sitter_javascript::HIGHLIGHT_QUERY;
const JSX: &str = tree_sitter_javascript::JSX_HIGHLIGHT_QUERY;
const TS: &str = tree_sitter_typescript::HIGHLIGHTS_QUERY;

static JAVASCRIPT: LazyLock<Grammar> =
    LazyLock::new(|| grammar(tree_sitter_javascript::LANGUAGE.into(), &[JS, JSX]));
static TYPESCRIPT: LazyLock<Grammar> = LazyLock::new(|| {
    grammar(
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        &[JS, TS],
    )
});
static TSX: LazyLock<Grammar> =
    LazyLock::new(|| grammar(tree_sitter_typescript::LANGUAGE_TSX.into(), &[JS, JSX, TS]));
static RUST: LazyLock<Grammar> = LazyLock::new(|| {
    grammar(
        tree_sitter_rust::LANGUAGE.into(),
        &[tree_sitter_rust::HIGHLIGHTS_QUERY],
    )
});
static ELIXIR: LazyLock<Grammar> = LazyLock::new(|| {
    grammar(
        tree_sitter_elixir::LANGUAGE.into(),
        &[tree_sitter_elixir::HIGHLIGHTS_QUERY],
    )
});
static HEEX: LazyLock<Grammar> = LazyLock::new(|| {
    grammar(
        tree_sitter_heex::LANGUAGE.into(),
        &[tree_sitter_heex::HIGHLIGHTS_QUERY],
    )
});
static YAML: LazyLock<Grammar> = LazyLock::new(|| {
    grammar(
        tree_sitter_yaml::LANGUAGE.into(),
        &[tree_sitter_yaml::HIGHLIGHTS_QUERY],
    )
});
static MARKDOWN: LazyLock<Grammar> = LazyLock::new(|| {
    grammar(
        tree_sitter_md::LANGUAGE.into(),
        &[tree_sitter_md::HIGHLIGHT_QUERY_BLOCK],
    )
});
static TOML: LazyLock<Grammar> = LazyLock::new(|| {
    grammar(
        tree_sitter_toml_ng::LANGUAGE.into(),
        // The bundled query colours a plain key as a type; keys in a pair are properties.
        &[
            tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
            "(pair (bare_key) @property)",
        ],
    )
});

/// A buffer's syntax tree, kept in step with its text.
// shortcut: parses on the core thread, so a huge file stalls visiting and big edits; move it to the syntax service when that shows.
pub(crate) struct Syntax {
    /// The language's name, for the mode line.
    pub(crate) name: &'static str,
    grammar: &'static Grammar,
    parser: Parser,
    tree: Tree,
    /// Edits since the last parse.
    stale: bool,
    /// The last highlights: their ranges, palette and runs, reused while the tree is unchanged.
    cache: RefCell<Option<Highlights>>,
}

type Runs = Vec<(Range<usize>, Face)>;
type Highlights = (Vec<Range<usize>>, BTreeMap<String, Face>, Runs);

/// The byte offset and tree-sitter point of char `char_idx`.
pub(crate) fn position(text: &Rope, char_idx: usize) -> (usize, Point) {
    let byte = text.char_to_byte(char_idx);
    let row = text.char_to_line(char_idx);
    (byte, Point::new(row, byte - text.line_to_byte(row)))
}

fn parse(parser: &mut Parser, text: &Rope, old: Option<&Tree>) -> Tree {
    let mut read = |byte: usize, _| match byte < text.len_bytes() {
        true => {
            let (chunk, start, _, _) = text.chunk_at_byte(byte);
            &chunk.as_bytes()[byte - start..]
        }
        false => &[][..],
    };
    (parser.parse_with_options(&mut read, old, None))
        .expect("a parse without timeout or cancellation finishes")
}

/// `name`'s face in `palette`, or that of its longest dotted prefix there.
pub(crate) fn face(palette: &BTreeMap<String, Face>, mut name: &str) -> Option<Face> {
    loop {
        if let Some(&face) = palette.get(name) {
            return Some(face);
        }
        name = &name[..name.rfind('.')?];
    }
}

impl Syntax {
    /// `text` parsed in the language `path`'s extension names, if aiditor has its grammar.
    pub(crate) fn for_file(path: &Path, text: &Rope) -> Option<Syntax> {
        let mut ext = path.extension()?.to_str()?;
        // An EEx template (`a.html.heex.eex`) is highlighted as the file it generates.
        if ext == "eex" {
            ext = Path::new(path.file_stem()?).extension()?.to_str()?;
        }
        let (name, grammar): (_, &'static Grammar) = match ext {
            "js" | "mjs" | "cjs" | "jsx" => ("JavaScript", &JAVASCRIPT),
            "ts" | "mts" | "cts" => ("TypeScript", &TYPESCRIPT),
            "tsx" => ("TSX", &TSX),
            "rs" => ("Rust", &RUST),
            "ex" | "exs" => ("Elixir", &ELIXIR),
            "heex" => ("HEEx", &HEEX),
            "yml" | "yaml" => ("YAML", &YAML),
            "md" | "markdown" => ("Markdown", &MARKDOWN),
            "toml" => ("TOML", &TOML),
            _ => return None,
        };
        let mut parser = Parser::new();
        (parser.set_language(&grammar.language)).expect("bundled grammars suit this tree-sitter");
        let tree = parse(&mut parser, text, None);
        Some(Syntax {
            name,
            grammar,
            parser,
            tree,
            stale: false,
            cache: RefCell::default(),
        })
    }

    /// Records that the text from `start` to `old_end` became the text from `start` to `new_end`.
    pub(crate) fn edit(
        &mut self,
        start: (usize, Point),
        old_end: (usize, Point),
        new_end: (usize, Point),
    ) {
        self.tree.edit(&InputEdit {
            start_byte: start.0,
            old_end_byte: old_end.0,
            new_end_byte: new_end.0,
            start_position: start.1,
            old_end_position: old_end.1,
            new_end_position: new_end.1,
        });
        self.stale = true;
        self.cache.take();
    }

    /// Reparses `text`, reusing the old tree, if it was edited since the last parse.
    pub(crate) fn reparse(&mut self, text: &Rope) {
        if std::mem::take(&mut self.stale) {
            self.tree = parse(&mut self.parser, text, Some(&self.tree));
            self.cache.take();
        }
    }

    /// The faces `palette` gives the chars of sorted, disjoint `ranges` of `text`, as sorted,
    /// disjoint char ranges. Each range is queried alone, so a long line's unseen chars cost
    /// nothing, while nodes reaching in from outside it still colour it.
    pub(crate) fn highlights(
        &self,
        text: &Rope,
        ranges: &[Range<usize>],
        palette: &BTreeMap<String, Face>,
    ) -> Runs {
        if let Some((cached, faces, runs)) = &*self.cache.borrow()
            && cached == ranges
            && faces == palette
        {
            return runs.clone();
        }
        let query = &self.grammar.highlights;
        let faces: Vec<_> = (query.capture_names().iter())
            .map(|name| face(palette, name))
            .collect();
        let mut cursor = QueryCursor::new();
        let mut runs: Runs = Vec::new();
        for range in ranges {
            let bytes = text.char_to_byte(range.start)..text.char_to_byte(range.end);
            cursor.set_byte_range(bytes.clone());
            let source = |node: Node| {
                text.byte_slice(node.byte_range())
                    .chunks()
                    .map(str::as_bytes)
            };
            let mut matches = cursor.matches(query, self.tree.root_node(), source);
            let mut captures = Vec::new();
            while let Some(m) = matches.next() {
                for capture in m.captures() {
                    if let Some(face) = faces[capture.index as usize] {
                        captures.push((capture.node.byte_range(), m.pattern_index, face));
                    }
                }
            }
            // Painted in order, so the innermost node wins, then the later pattern on one node.
            captures.sort_by_key(|(r, pattern, _)| (Reverse(r.len()), *pattern));
            let mut painted = vec![None; bytes.len()];
            for (r, _, face) in captures {
                let clamp = |b: usize| b.clamp(bytes.start, bytes.end) - bytes.start;
                painted[clamp(r.start)..clamp(r.end).max(clamp(r.start))].fill(Some(face));
            }
            let mut byte = 0;
            for (i, c) in text.slice(range.clone()).chars().enumerate() {
                let at = range.start + i;
                match (painted[byte], runs.last_mut()) {
                    (Some(face), Some((r, last))) if r.end == at && *last == face => r.end += 1,
                    (Some(face), _) => runs.push((at..at + 1, face)),
                    (None, _) => {}
                }
                byte += c.len_utf8();
            }
        }
        *self.cache.borrow_mut() = Some((ranges.to_vec(), palette.clone(), runs.clone()));
        runs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fg(color: u32) -> Face {
        Face {
            fg: Some(color),
            ..Face::default()
        }
    }

    fn palette(entries: &[(&str, u32)]) -> BTreeMap<String, Face> {
        (entries.iter())
            .map(|&(k, v)| (k.to_string(), fg(v)))
            .collect()
    }

    /// Each coloured run of `text` parsed as `file`, as (text, colour).
    fn runs(file: &str, text: &str, palette: &BTreeMap<String, Face>) -> Vec<(String, u32)> {
        let rope = Rope::from_str(text);
        let syntax = Syntax::for_file(Path::new(file), &rope).unwrap();
        (syntax
            .highlights(&rope, &[0..rope.len_chars()], palette)
            .into_iter())
        .map(|(r, f)| (rope.slice(r).to_string(), f.fg.unwrap()))
        .collect()
    }

    #[test]
    fn languages_by_extension() {
        let rope = Rope::from_str("x");
        let exts = ["js", "mjs", "cjs", "jsx", "ts", "mts", "cts", "tsx"];
        for ext in exts.into_iter().chain([
            "rs", "ex", "exs", "heex", "yml", "yaml", "md", "markdown", "toml",
        ]) {
            let path = format!("a.{ext}");
            assert!(Syntax::for_file(Path::new(&path), &rope).is_some(), "{ext}");
        }
        for file in ["a.html.heex.eex", "a.ex.eex", "a.js.eex"] {
            assert!(Syntax::for_file(Path::new(file), &rope).is_some(), "{file}");
        }
        assert!(Syntax::for_file(Path::new("a.eex"), &rope).is_none());
        assert!(Syntax::for_file(Path::new("a.html.eex"), &rope).is_none());
        assert!(Syntax::for_file(Path::new("a.txt"), &rope).is_none());
        assert!(Syntax::for_file(Path::new("Makefile"), &rope).is_none());
    }

    #[test]
    fn a_typescript_snippet() {
        let p = palette(&[("keyword", 1), ("type", 2), ("number", 3), ("string", 4)]);
        assert_eq!(
            runs("a.ts", "const x: number = 1; // é\nlet s = \"hi\";", &p),
            [
                ("const".into(), 1),
                ("number".into(), 2),
                ("1".into(), 3),
                ("let".into(), 1),
                ("\"hi\"".into(), 4),
            ]
        );
    }

    #[test]
    fn rust_elixir_heex_and_yaml_snippets() {
        let p = palette(&[
            ("keyword", 1),
            ("function", 2),
            ("module", 3),
            ("tag", 4),
            ("attribute", 5),
            ("string", 6),
            ("property", 7),
            ("boolean", 8),
        ]);
        let r = |file, text| runs(file, text, &p);
        assert_eq!(
            r("a.rs", "fn main() {}"),
            [("fn".into(), 1), ("main".into(), 2)]
        );
        assert_eq!(
            r("a.ex", "defmodule Foo do end"),
            [
                ("defmodule".into(), 1),
                ("Foo".into(), 3),
                ("do".into(), 1),
                ("end".into(), 1),
            ]
        );
        assert_eq!(
            r("a.heex", "<div class=\"a\"></div>"),
            [
                ("div".into(), 4),
                ("class".into(), 5),
                ("\"a\"".into(), 6),
                ("div".into(), 4),
            ]
        );
        assert_eq!(
            r("a.yaml", "a: true"),
            [("a".into(), 7), ("true".into(), 8)]
        );
    }

    #[test]
    fn markdown_blocks() {
        let p = palette(&[
            ("text.title", 1),
            ("text.literal", 2),
            ("punctuation.special", 3),
            ("text.reference", 4),
            ("text.uri", 5),
        ]);
        assert_eq!(
            runs(
                "a.md",
                "# Title\n\n- item\n\n> quote\n\n---\n\n```rs\nfn x\n```\n\n[a]: http://x\n",
                &p
            ),
            [
                ("#".into(), 3),
                ("Title".into(), 1),
                // Markers' nodes take the space or newline after them.
                ("- ".into(), 3),
                ("> ".into(), 3),
                ("---\n".into(), 3),
                ("```rs\nfn x\n```\n".into(), 2),
                ("[a]".into(), 4),
                ("http://x".into(), 5),
            ]
        );
    }

    #[test]
    fn toml_keys_and_values() {
        let p = palette(&[
            ("type", 1),
            ("property", 2),
            ("string", 3),
            ("boolean", 4),
            ("operator", 5),
        ]);
        assert_eq!(
            runs("a.toml", "[package]\nname = \"a\"\nx.y = true\n", &p),
            [
                ("package".into(), 1),
                // The bundled query captures the whole pair, so its spaces take the key's colour.
                ("name ".into(), 2),
                ("=".into(), 5),
                (" ".into(), 2),
                ("\"a\"".into(), 3),
                ("x".into(), 2),
                ("y".into(), 2),
                ("=".into(), 5),
                ("true".into(), 4),
            ]
        );
    }

    #[test]
    fn bundled_toml_files_parse_cleanly() {
        for text in [crate::settings::BUNDLED, include_str!("../Cargo.toml")] {
            let syntax = Syntax::for_file(Path::new("a.toml"), &Rope::from_str(text)).unwrap();
            assert!(!syntax.tree.root_node().has_error());
        }
    }

    #[test]
    fn jsx_tags_in_tsx_and_js() {
        let p = palette(&[("tag", 5)]);
        assert_eq!(runs("a.tsx", "const a = <div />;", &p), [("div".into(), 5)]);
        assert_eq!(runs("a.js", "const a = <div />;", &p), [("div".into(), 5)]);
    }

    #[test]
    fn prefix_fallback_and_inner_nodes_win() {
        let p = palette(&[("function", 6), ("string", 4), ("comment", 7)]);
        assert_eq!(face(&p, "function.method"), Some(fg(6)));
        assert_eq!(face(&p, "keyword"), None);
        assert_eq!(
            runs("a.js", "o.f(`a${g()}b`) /* c */", &p),
            [
                ("f".into(), 6),
                ("`a${".into(), 4),
                ("g".into(), 6),
                ("()}b`".into(), 4),
                ("/* c */".into(), 7),
            ]
        );
    }

    #[test]
    fn only_the_given_range_is_coloured() {
        let rope = Rope::from_str("let a;\nlet b;\nlet c;");
        let syntax = Syntax::for_file(Path::new("a.js"), &rope).unwrap();
        let p = palette(&[("keyword", 1)]);
        assert_eq!(syntax.highlights(&rope, &[7..14], &p), [(7..10, fg(1))]);
    }

    #[test]
    fn each_range_keeps_the_context_before_it() {
        let rope = Rope::from_str("let s = \"abcdef\"; let t;");
        let syntax = Syntax::for_file(Path::new("a.js"), &rope).unwrap();
        let p = palette(&[("keyword", 1), ("string", 4)]);
        // Inside a string opened before the range; the `let` past the ranges is not coloured.
        let runs = syntax.highlights(&rope, &[11..13, 15..17], &p);
        assert_eq!(runs, [(11..13, fg(4)), (15..16, fg(4))]);
    }

    #[test]
    fn highlights_are_reused_until_an_edit_or_another_palette() {
        let mut rope = Rope::from_str("let a;");
        let mut syntax = Syntax::for_file(Path::new("a.js"), &rope).unwrap();
        let p = palette(&[("keyword", 1)]);
        assert_eq!(syntax.highlights(&rope, &[0..6], &p), [(0..3, fg(1))]);
        // A tree swapped without `edit` isn't queried: the cached runs come back.
        syntax.tree = parse(&mut syntax.parser, &Rope::from_str("a; let b;"), None);
        assert_eq!(syntax.highlights(&rope, &[0..6], &p), [(0..3, fg(1))]);
        assert_eq!(
            syntax.highlights(&rope, &[0..6], &palette(&[("keyword", 2)])),
            [(3..6, fg(2))]
        );
        let start = position(&rope, 0);
        rope.insert(0, "// ");
        syntax.edit(start, start, position(&rope, 3));
        syntax.reparse(&rope);
        let p = palette(&[("keyword", 1), ("comment", 7)]);
        assert_eq!(syntax.highlights(&rope, &[0..9], &p), [(0..9, fg(7))]);
        syntax.edit(start, start, start);
        assert_eq!(syntax.cache.borrow().as_ref(), None, "an edit drops them");
    }

    #[test]
    fn edits_reparse_incrementally() {
        let mut rope = Rope::from_str("let a = 1;");
        let mut syntax = Syntax::for_file(Path::new("a.js"), &rope).unwrap();
        let start = position(&rope, 0);
        rope.insert(0, "// ");
        syntax.edit(start, start, position(&rope, 3));
        syntax.reparse(&rope);
        let p = palette(&[("keyword", 1), ("comment", 7)]);
        let all = 0..rope.len_chars();
        assert_eq!(syntax.highlights(&rope, &[all], &p), [(0..13, fg(7))]);
    }

    #[test]
    fn buffer_edits_after_multibyte_text_keep_the_tree_in_step() {
        let mut buffers = crate::buffer::Buffers::default();
        let id = buffers.create_for_file(Path::new("/a.js"), "'é'\nx;");
        let b = buffers.get_mut(id).unwrap();
        b.insert(4, "let ").unwrap();
        b.delete(0..4).unwrap();
        b.reparse();
        let p = palette(&[("keyword", 1), ("string", 4)]);
        assert_eq!(b.text().to_string(), "let x;");
        assert_eq!(b.highlights(&[0..6], &p), [(0..3, fg(1))]);
        b.undo();
        b.reparse();
        assert_eq!(b.highlights(&[0..b.len_chars()], &p), [(0..3, fg(4))]);
    }
}
