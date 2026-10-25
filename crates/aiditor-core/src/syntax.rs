//! Tree-sitter parsing of a visited file and highlighting of the lines a window shows.

use ropey::Rope;
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

/// A buffer's syntax tree, kept in step with its text.
// shortcut: parses on the core thread, so a huge file stalls visiting and big edits; move it to the syntax service when that shows.
pub(crate) struct Syntax {
    grammar: &'static Grammar,
    parser: Parser,
    tree: Tree,
    /// Edits since the last parse.
    stale: bool,
}

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

/// `name`'s colour in `palette`, or that of its longest dotted prefix there.
fn color(palette: &BTreeMap<String, u32>, mut name: &str) -> Option<u32> {
    loop {
        if let Some(&color) = palette.get(name) {
            return Some(color);
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
        let grammar: &'static Grammar = match ext {
            "js" | "mjs" | "cjs" | "jsx" => &JAVASCRIPT,
            "ts" | "mts" | "cts" => &TYPESCRIPT,
            "tsx" => &TSX,
            "rs" => &RUST,
            "ex" | "exs" => &ELIXIR,
            "heex" => &HEEX,
            "yml" | "yaml" => &YAML,
            _ => return None,
        };
        let mut parser = Parser::new();
        (parser.set_language(&grammar.language)).expect("bundled grammars suit this tree-sitter");
        let tree = parse(&mut parser, text, None);
        Some(Syntax {
            grammar,
            parser,
            tree,
            stale: false,
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
    }

    /// Reparses `text`, reusing the old tree, if it was edited since the last parse.
    pub(crate) fn reparse(&mut self, text: &Rope) {
        if std::mem::take(&mut self.stale) {
            self.tree = parse(&mut self.parser, text, Some(&self.tree));
        }
    }

    /// The colours `palette` gives chars `range` of `text`, as sorted, disjoint char ranges.
    pub(crate) fn highlights(
        &self,
        text: &Rope,
        range: Range<usize>,
        palette: &BTreeMap<String, u32>,
    ) -> Vec<(Range<usize>, u32)> {
        let bytes = text.char_to_byte(range.start)..text.char_to_byte(range.end);
        let query = &self.grammar.highlights;
        let colors: Vec<_> = (query.capture_names().iter())
            .map(|name| color(palette, name))
            .collect();
        let mut cursor = QueryCursor::new();
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
                if let Some(color) = colors[capture.index as usize] {
                    captures.push((capture.node.byte_range(), m.pattern_index, color));
                }
            }
        }
        // Painted in order, so the innermost node wins, then the later pattern on one node.
        captures.sort_by_key(|(r, pattern, _)| (Reverse(r.len()), *pattern));
        let mut painted = vec![None; bytes.len()];
        for (r, _, color) in captures {
            let clamp = |b: usize| b.clamp(bytes.start, bytes.end) - bytes.start;
            painted[clamp(r.start)..clamp(r.end).max(clamp(r.start))].fill(Some(color));
        }
        let mut runs: Vec<(Range<usize>, u32)> = Vec::new();
        let mut byte = 0;
        for (i, c) in text.slice(range.clone()).chars().enumerate() {
            let at = range.start + i;
            match (painted[byte], runs.last_mut()) {
                (Some(color), Some((r, last))) if r.end == at && *last == color => r.end += 1,
                (Some(color), _) => runs.push((at..at + 1, color)),
                (None, _) => {}
            }
            byte += c.len_utf8();
        }
        runs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette(entries: &[(&str, u32)]) -> BTreeMap<String, u32> {
        (entries.iter()).map(|&(k, v)| (k.to_string(), v)).collect()
    }

    /// Each coloured run of `text` parsed as `file`, as (text, colour).
    fn runs(file: &str, text: &str, palette: &BTreeMap<String, u32>) -> Vec<(String, u32)> {
        let rope = Rope::from_str(text);
        let syntax = Syntax::for_file(Path::new(file), &rope).unwrap();
        (syntax
            .highlights(&rope, 0..rope.len_chars(), palette)
            .into_iter())
        .map(|(r, c)| (rope.slice(r).to_string(), c))
        .collect()
    }

    #[test]
    fn languages_by_extension() {
        let rope = Rope::from_str("x");
        let exts = ["js", "mjs", "cjs", "jsx", "ts", "mts", "cts", "tsx"];
        for ext in exts
            .into_iter()
            .chain(["rs", "ex", "exs", "heex", "yml", "yaml"])
        {
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
    fn jsx_tags_in_tsx_and_js() {
        let p = palette(&[("tag", 5)]);
        assert_eq!(runs("a.tsx", "const a = <div />;", &p), [("div".into(), 5)]);
        assert_eq!(runs("a.js", "const a = <div />;", &p), [("div".into(), 5)]);
    }

    #[test]
    fn prefix_fallback_and_inner_nodes_win() {
        let p = palette(&[("function", 6), ("string", 4), ("comment", 7)]);
        assert_eq!(color(&p, "function.method"), Some(6));
        assert_eq!(color(&p, "keyword"), None);
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
        assert_eq!(syntax.highlights(&rope, 7..14, &p), [(7..10, 1)]);
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
        assert_eq!(syntax.highlights(&rope, all, &p), [(0..13, 7)]);
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
        assert_eq!(b.highlights(0..6, &p), [(0..3, 1)]);
        b.undo();
        b.reparse();
        assert_eq!(b.highlights(0..b.len_chars(), &p), [(0..3, 4)]);
    }
}
