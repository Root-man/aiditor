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
    locals: Option<Query>,
}

/// Compiles `queries` joined, later patterns overriding earlier ones, as Zed and Helix extend
/// JavaScript's query for TypeScript.
fn grammar(language: Language, queries: &[&str]) -> Grammar {
    let highlights = Query::new(&language, &queries.concat()).expect("bundled queries compile");
    Grammar {
        language,
        highlights,
        locals: None,
    }
}

impl Grammar {
    fn with_locals(mut self, queries: &[&str]) -> Self {
        self.locals =
            Some(Query::new(&self.language, &queries.concat()).expect("bundled locals compile"));
        self
    }
}

const JS: &str = tree_sitter_javascript::HIGHLIGHT_QUERY;
const JSX: &str = tree_sitter_javascript::JSX_HIGHLIGHT_QUERY;
const TS: &str = tree_sitter_typescript::HIGHLIGHTS_QUERY;

static JAVASCRIPT: LazyLock<Grammar> = LazyLock::new(|| {
    grammar(tree_sitter_javascript::LANGUAGE.into(), &[JS, JSX])
        .with_locals(&[tree_sitter_javascript::LOCALS_QUERY])
});
static TYPESCRIPT: LazyLock<Grammar> = LazyLock::new(|| {
    grammar(
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        &[JS, TS],
    )
    .with_locals(&[
        tree_sitter_javascript::LOCALS_QUERY,
        tree_sitter_typescript::LOCALS_QUERY,
    ])
});
static TSX: LazyLock<Grammar> = LazyLock::new(|| {
    grammar(tree_sitter_typescript::LANGUAGE_TSX.into(), &[JS, JSX, TS]).with_locals(&[
        tree_sitter_javascript::LOCALS_QUERY,
        tree_sitter_typescript::LOCALS_QUERY,
    ])
});
static RUST: LazyLock<Grammar> = LazyLock::new(|| {
    grammar(
        tree_sitter_rust::LANGUAGE.into(),
        &[
            tree_sitter_rust::HIGHLIGHTS_QUERY,
            "(let_declaration pattern: (identifier) @variable)
           (closure_parameters (identifier) @variable.parameter)",
        ],
    )
    .with_locals(&[include_str!("../queries/rust/locals.scm")])
});
static ELIXIR: LazyLock<Grammar> = LazyLock::new(|| {
    grammar(
        tree_sitter_elixir::LANGUAGE.into(),
        &[tree_sitter_elixir::HIGHLIGHTS_QUERY],
    )
});
static ELIXIR_INJECTIONS: LazyLock<Query> = LazyLock::new(|| {
    Query::new(
        &ELIXIR.language,
        "((sigil (sigil_name) @_name (quoted_content) @content) (#eq? @_name \"H\"))",
    )
    .expect("bundled queries compile")
});
static HEEX_INJECTIONS: LazyLock<Query> = LazyLock::new(|| {
    Query::new(&HEEX.language, tree_sitter_heex::INJECTIONS_QUERY).expect("bundled queries compile")
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
static MARKDOWN_INLINE: LazyLock<Grammar> = LazyLock::new(|| {
    grammar(
        tree_sitter_md::INLINE_LANGUAGE.into(),
        &[tree_sitter_md::HIGHLIGHT_QUERY_INLINE],
    )
});
static MARKDOWN_INJECTIONS: LazyLock<Query> = LazyLock::new(|| {
    Query::new(
        &MARKDOWN.language,
        "[(inline) (pipe_table_cell)] @content
         (fenced_code_block (info_string (language) @language)
                            (code_fence_content) @content)",
    )
    .expect("bundled queries compile")
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
    injections: Vec<Injection>,
    /// Edits since the last parse.
    stale: bool,
    /// The last highlights: their ranges, palette and runs, reused while the tree is unchanged.
    cache: RefCell<Option<Highlights>>,
    locals: RefCell<Option<Locals>>,
}

type Runs = Vec<(Range<usize>, Face)>;
type Highlights = (Vec<Range<usize>>, BTreeMap<String, Face>, Runs);

#[derive(Clone)]
struct Injection {
    grammar: &'static Grammar,
    ranges: Vec<tree_sitter::Range>,
    tree: Tree,
    children: Vec<Injection>,
    locals: RefCell<Option<Locals>>,
}

fn content_ranges(node: Node) -> Vec<tree_sitter::Range> {
    let mut range = node.range();
    let mut ranges = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "block_continuation" {
            if range.start_byte < child.start_byte() {
                ranges.push(tree_sitter::Range {
                    end_byte: child.start_byte(),
                    end_point: child.start_position(),
                    ..range
                });
            }
            range.start_byte = child.end_byte();
            range.start_point = child.end_position();
        }
    }
    if range.start_byte < range.end_byte {
        ranges.push(range);
    }
    ranges
}

fn injections(
    host: &'static Grammar,
    text: &Rope,
    tree: &Tree,
    old: Vec<Injection>,
) -> Vec<Injection> {
    let (query, default): (&Query, &Grammar) = if std::ptr::eq(host, &*MARKDOWN) {
        (&MARKDOWN_INJECTIONS, &MARKDOWN_INLINE)
    } else if std::ptr::eq(host, &*ELIXIR) {
        (&ELIXIR_INJECTIONS, &HEEX)
    } else if std::ptr::eq(host, &*HEEX) {
        (&HEEX_INJECTIONS, &ELIXIR)
    } else {
        return Vec::new();
    };
    let mut old: BTreeMap<_, _> = old
        .into_iter()
        .map(|i| (i.ranges[0].start_byte, i))
        .collect();
    let mut parser = Parser::new();
    let mut cursor = QueryCursor::new();
    let source = |node: Node| {
        text.byte_slice(node.byte_range())
            .chunks()
            .map(str::as_bytes)
    };
    let mut matches = cursor.matches(query, tree.root_node(), source);
    let included = tree.included_ranges();
    let mut regions = Vec::new();
    let mut combined = BTreeMap::<usize, Vec<tree_sitter::Range>>::new();
    while let Some(m) = matches.next() {
        let mut content = None;
        let mut grammar: Option<&Grammar> = Some(default);
        for c in m.captures() {
            match query.capture_names()[c.index as usize] {
                "content" | "injection.content" => content = Some(c.node),
                "language" => {
                    grammar = match text.byte_slice(c.node.byte_range()).to_string().as_str() {
                        "rust" => Some(&RUST),
                        "elixir" => Some(&ELIXIR),
                        "js" => Some(&JAVASCRIPT),
                        "ts" => Some(&TYPESCRIPT),
                        "yaml" => Some(&YAML),
                        "toml" => Some(&TOML),
                        _ => None,
                    };
                }
                _ => {}
            }
        }
        let (Some(content), Some(grammar)) = (content, grammar) else {
            continue;
        };
        let ranges = content_ranges(content)
            .into_iter()
            .flat_map(|r| {
                included.iter().filter_map(move |parent| {
                    let start_byte = r.start_byte.max(parent.start_byte);
                    let end_byte = r.end_byte.min(parent.end_byte);
                    (start_byte < end_byte).then_some(tree_sitter::Range {
                        start_byte,
                        end_byte,
                        start_point: r.start_point.max(parent.start_point),
                        end_point: r.end_point.min(parent.end_point),
                    })
                })
            })
            .collect::<Vec<_>>();
        if query
            .property_settings(m.pattern_index)
            .iter()
            .any(|p| p.key.as_ref() == "injection.combined")
        {
            combined.entry(m.pattern_index).or_default().extend(ranges);
        } else {
            regions.push((grammar, ranges));
        }
    }
    regions.extend(combined.into_values().map(|ranges| (default, ranges)));
    let mut injections = Vec::new();
    for (grammar, ranges) in regions {
        let Some(first) = ranges.first() else {
            continue;
        };
        let previous = old
            .remove(&first.start_byte)
            .filter(|i| std::ptr::eq(i.grammar, grammar) && i.ranges == ranges);
        parser
            .set_language(&grammar.language)
            .expect("bundled grammar");
        parser
            .set_included_ranges(&ranges)
            .expect("ordered content ranges");
        let tree = parse(&mut parser, text, previous.as_ref().map(|i| &i.tree));
        let children = self::injections(
            grammar,
            text,
            &tree,
            previous.map_or_else(Vec::new, |i| i.children),
        );
        injections.push(Injection {
            grammar,
            ranges,
            tree,
            children,
            locals: RefCell::default(),
        });
    }
    injections.sort_by_key(|i| i.ranges[0].start_byte);
    injections
}

impl Injection {
    fn edit(&mut self, edit: &InputEdit) {
        self.tree.edit(edit);
        for range in &mut self.ranges {
            edit.edit_range(range);
        }
        for child in &mut self.children {
            child.edit(edit);
        }
    }
}

fn paint_injections(
    injections: &[Injection],
    text: &Rope,
    bytes: Range<usize>,
    palette: &BTreeMap<String, Face>,
    painted: &mut [Option<Face>],
) {
    // Combined directives can surround other expressions, so sibling ends are not sorted.
    for injection in injections
        .iter()
        .take_while(|i| i.ranges[0].start_byte < bytes.end)
        .filter(|i| i.ranges.last().unwrap().end_byte > bytes.start)
    {
        let first = injection
            .ranges
            .partition_point(|r| r.end_byte <= bytes.start);
        for included in injection.ranges[first..]
            .iter()
            .take_while(|r| r.start_byte < bytes.end)
        {
            let start = included.start_byte.max(bytes.start);
            let end = included.end_byte.min(bytes.end);
            if start >= end {
                continue;
            }
            let painted = &mut painted[start - bytes.start..end - bytes.start];
            if !std::ptr::eq(injection.grammar, &*MARKDOWN_INLINE) {
                painted.fill(None);
            }
            paint(
                injection.grammar,
                &injection.tree,
                text,
                start..end,
                palette,
                painted,
                &injection.locals,
            );
            paint_injections(&injection.children, text, start..end, palette, painted);
        }
    }
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

type NodeRange = (usize, usize);
type Locals = BTreeMap<NodeRange, Option<u32>>;

fn local_pattern(query: &Query, pattern: usize, local: bool) -> bool {
    query
        .property_predicates(pattern)
        .iter()
        .all(|(p, positive)| p.key.as_ref() != "local" || local == *positive)
}

fn locals(grammar: &Grammar, tree: &Tree, text: &Rope) -> Locals {
    let Some(query) = &grammar.locals else {
        return Locals::new();
    };
    enum Kind {
        Scope(bool),
        Definition(usize),
        Reference,
    }
    struct Scope {
        end: usize,
        inherits: bool,
        definitions: BTreeMap<String, Vec<(usize, Option<u32>)>>,
    }
    let mut cursor = QueryCursor::new();
    let source = |node: Node| {
        text.byte_slice(node.byte_range())
            .chunks()
            .map(str::as_bytes)
    };
    let mut matches = cursor.matches(query, tree.root_node(), source);
    let mut events = Vec::new();
    while let Some(m) = matches.next() {
        let value_end = m
            .captures()
            .iter()
            .find_map(|c| {
                (query.capture_names()[c.index as usize] == "local.definition-value")
                    .then_some(c.node.end_byte())
            })
            .unwrap_or(0);
        for c in m.captures() {
            let kind = match query.capture_names()[c.index as usize] {
                "local.scope" => {
                    Kind::Scope(!query.property_settings(m.pattern_index).iter().any(|p| {
                        p.key.as_ref() == "local.scope-inherits"
                            && p.value.as_deref() == Some("false")
                    }))
                }
                "local.definition" => Kind::Definition(value_end),
                "local.reference" => Kind::Reference,
                _ => continue,
            };
            events.push((c.node.byte_range(), kind));
        }
    }
    events.sort_by_key(|(r, kind)| {
        (
            r.start,
            Reverse(r.end),
            match kind {
                Kind::Scope(_) => 0,
                Kind::Definition(_) => 1,
                Kind::Reference => 2,
            },
        )
    });
    let mut scopes = vec![Scope {
        end: usize::MAX,
        inherits: false,
        definitions: BTreeMap::new(),
    }];
    let mut locals = Locals::new();
    let mut highlights = QueryCursor::new();
    for (range, kind) in events {
        while range.start >= scopes.last().unwrap().end {
            scopes.pop();
        }
        let key = (range.start, range.end);
        match kind {
            Kind::Scope(inherits) => scopes.push(Scope {
                end: range.end,
                inherits,
                definitions: BTreeMap::new(),
            }),
            Kind::Definition(value_end) => {
                highlights.set_byte_range(range.clone());
                let mut matches = highlights.matches(&grammar.highlights, tree.root_node(), source);
                let mut capture = None;
                while let Some(m) = matches.next() {
                    if local_pattern(&grammar.highlights, m.pattern_index, true) {
                        for c in m.captures() {
                            if c.node.byte_range() == range
                                && capture.is_none_or(|(p, _)| p <= m.pattern_index)
                            {
                                capture = Some((m.pattern_index, c.index));
                            }
                        }
                    }
                }
                let capture = capture.map(|(_, index)| index);
                scopes
                    .last_mut()
                    .unwrap()
                    .definitions
                    .entry(text.byte_slice(range).to_string())
                    .or_default()
                    .push((value_end, capture));
                locals.insert(key, capture);
            }
            Kind::Reference if !locals.contains_key(&key) => {
                let name = text.byte_slice(range.clone()).to_string();
                for scope in scopes.iter().rev() {
                    if let Some((_, capture)) = scope
                        .definitions
                        .get(&name)
                        .and_then(|defs| defs.iter().rev().find(|(end, _)| range.start >= *end))
                    {
                        locals.insert(key, *capture);
                        break;
                    }
                    if !scope.inherits {
                        break;
                    }
                }
            }
            Kind::Reference => {}
        }
    }
    locals
}

fn paint(
    grammar: &Grammar,
    tree: &Tree,
    text: &Rope,
    bytes: Range<usize>,
    palette: &BTreeMap<String, Face>,
    painted: &mut [Option<Face>],
    locals: &RefCell<Option<Locals>>,
) {
    if locals.borrow().is_none() {
        *locals.borrow_mut() = Some(self::locals(grammar, tree, text));
    }
    let locals = locals.borrow();
    let locals = locals.as_ref().unwrap();
    let compose = std::ptr::eq(grammar, &*MARKDOWN_INLINE);
    let query = &grammar.highlights;
    let faces: Vec<_> = query
        .capture_names()
        .iter()
        .map(|name| face(palette, name))
        .collect();
    let mut cursor = QueryCursor::new();
    cursor.set_byte_range(bytes.clone());
    let source = |node: Node| {
        text.byte_slice(node.byte_range())
            .chunks()
            .map(str::as_bytes)
    };
    let mut matches = cursor.matches(query, tree.root_node(), source);
    let mut captures = Vec::new();
    while let Some(m) = matches.next() {
        for capture in m.captures() {
            let key = (capture.node.start_byte(), capture.node.end_byte());
            if !local_pattern(query, m.pattern_index, locals.contains_key(&key)) {
                continue;
            }
            if let Some(face) = faces[capture.index as usize] {
                captures.push((capture.node.byte_range(), m.pattern_index, face));
            }
        }
    }
    for (&(start, end), capture) in locals
        .range(..(bytes.end, 0))
        .rev()
        .take_while(|((_, end), _)| *end > bytes.start)
    {
        if let Some(face) = capture.and_then(|c| faces[c as usize]) {
            captures.push((start..end, usize::MAX, face));
        }
    }
    // Painted in order, so the innermost node wins, then the later pattern on one node.
    captures.sort_by_key(|(r, pattern, _)| (Reverse(r.len()), *pattern));
    for (r, _, face) in captures {
        let clamp = |b: usize| b.clamp(bytes.start, bytes.end) - bytes.start;
        for under in &mut painted[clamp(r.start)..clamp(r.end).max(clamp(r.start))] {
            *under = Some(if compose {
                face.over(under.unwrap_or_default())
            } else {
                face
            });
        }
    }
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
    pub(crate) fn snapshot(&self) -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(&self.grammar.language)
            .expect("bundled grammar");
        Self {
            name: self.name,
            grammar: self.grammar,
            parser,
            tree: self.tree.clone(),
            injections: self.injections.clone(),
            stale: false,
            cache: RefCell::default(),
            locals: self.locals.clone(),
        }
    }

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
        let injections = injections(grammar, text, &tree, Vec::new());
        Some(Syntax {
            name,
            grammar,
            parser,
            tree,
            injections,
            stale: false,
            cache: RefCell::default(),
            locals: RefCell::default(),
        })
    }

    /// Records that the text from `start` to `old_end` became the text from `start` to `new_end`.
    pub(crate) fn edit(
        &mut self,
        start: (usize, Point),
        old_end: (usize, Point),
        new_end: (usize, Point),
    ) {
        let edit = InputEdit {
            start_byte: start.0,
            old_end_byte: old_end.0,
            new_end_byte: new_end.0,
            start_position: start.1,
            old_end_position: old_end.1,
            new_end_position: new_end.1,
        };
        self.tree.edit(&edit);
        for injection in &mut self.injections {
            injection.edit(&edit);
        }
        self.stale = true;
        self.cache.take();
        self.locals.take();
    }

    /// Reparses `text`, reusing the old tree, if it was edited since the last parse.
    pub(crate) fn reparse(&mut self, text: &Rope) {
        if std::mem::take(&mut self.stale) {
            self.tree = parse(&mut self.parser, text, Some(&self.tree));
            self.injections = injections(
                self.grammar,
                text,
                &self.tree,
                std::mem::take(&mut self.injections),
            );
            self.cache.take();
        }
    }

    /// The faces `palette` gives the chars of sorted, disjoint `ranges` of `text`, as sorted,
    /// disjoint char ranges. Painting queries each range alone; local bindings and definition
    /// captures are cached across the whole tree, including definitions outside the window.
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
        let mut runs: Runs = Vec::new();
        for range in ranges {
            let bytes = text.char_to_byte(range.start)..text.char_to_byte(range.end);
            let mut painted = vec![None; bytes.len()];
            paint(
                self.grammar,
                &self.tree,
                text,
                bytes.clone(),
                palette,
                &mut painted,
                &self.locals,
            );
            paint_injections(&self.injections, text, bytes.clone(), palette, &mut painted);
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

    fn faces(file: &str, text: &str) -> Vec<Face> {
        let rope = Rope::from_str(text);
        let syntax = Syntax::for_file(Path::new(file), &rope).unwrap();
        let mut faces = vec![Face::default(); rope.len_chars()];
        for (range, face) in syntax.highlights(
            &rope,
            &[0..rope.len_chars()],
            &crate::settings::Settings::default().theme.syntax,
        ) {
            faces[range].fill(face);
        }
        faces
    }

    fn face_at(text: &str, faces: &[Face], needle: &str) -> Face {
        faces[text[..text.find(needle).unwrap()].chars().count()]
    }

    #[test]
    fn heex_sigils_and_standalone_templates_have_elixir_expressions() {
        let template = r#"<div class="a" id={String.upcase("é")}><%= if true do %>
          <.card value={%{count: 42}} /> plain
          <%= String.downcase("VALUE") %><% end %><!-- comment --></div>"#;
        let p = crate::settings::Settings::default().theme.syntax;
        for (file, text) in [
            ("a.heex", template.to_string()),
            (
                "a.ex",
                format!("def render(assigns) do\n~H\"\"\"\n{template}\n\"\"\"\nend"),
            ),
            (
                "a.md",
                format!(
                    "> ```elixir\n> ~H\"\"\"\n> {}\n> \"\"\"\n> ```\n",
                    template.replace('\n', "\n> ")
                ),
            ),
        ] {
            let highlighted = faces(file, &text);
            for (needle, name) in [
                ("div", "tag"),
                ("class", "attribute"),
                ("\"a\"", "string"),
                ("String", "module"),
                ("upcase", "function"),
                ("\"é\"", "string"),
                ("if", "keyword"),
                ("true", "constant"),
                ("do", "keyword"),
                ("card", "function"),
                ("42", "number"),
                ("downcase", "function"),
                ("end %>", "keyword"),
                ("<!-- comment -->", "comment"),
                ("{String", "punctuation.bracket"),
                ("<%=", "punctuation.bracket"),
            ] {
                assert_eq!(
                    face_at(&text, &highlighted, needle),
                    face(&p, name).unwrap_or_default(),
                    "{file}: {needle}"
                );
            }
            assert_eq!(face_at(&text, &highlighted, "plain"), Face::default());
            if file == "a.md" {
                assert_eq!(
                    face_at(&text, &highlighted, ">           <.card"),
                    p["punctuation.special"]
                );
            }
        }
    }

    #[test]
    fn heex_sigils_keep_delimiters_and_separate_template_contexts() {
        let p = crate::settings::Settings::default().theme.syntax;
        for sigil in ["~H", "~s", "~h"] {
            let text = format!("{sigil}|<div>plain</div>|");
            let highlighted = faces("a.ex", &text);
            let sigil_face = if sigil == "~s" {
                "string"
            } else {
                "string.special"
            };
            assert_eq!(face_at(&text, &highlighted, "|"), p[sigil_face]);
            assert_eq!(
                face_at(&text, &highlighted, "div"),
                p[if sigil == "~H" { "tag" } else { sigil_face }]
            );
        }
        let text = "~H|<%= \"unclosed %>|\n~H|<span><%= String.upcase(\"ok\") %></span>|";
        let highlighted = faces("a.ex", text);
        assert_eq!(face_at(text, &highlighted, "span"), p["tag"]);
        assert_eq!(face_at(text, &highlighted, "String"), p["module"]);
        assert_eq!(face_at(text, &highlighted, "upcase"), p["function"]);
        assert_eq!(face_at(text, &highlighted, "\"ok\""), p["string"]);
    }

    #[test]
    fn heex_visible_ranges_keep_expression_context_and_current_palette() {
        let text =
            "~H\"\"\"\n<div><%= if true do %>\n<%= \"é\ncontinued\" %>\n<% end %></div>\n\"\"\"";
        let rope = Rope::from_str(text);
        let syntax = Syntax::for_file(Path::new("a.ex"), &rope).unwrap();
        for (needle, name) in [("continued", "string"), ("end", "keyword")] {
            let start = text[..text.find(needle).unwrap()].chars().count();
            for color in [1, 2] {
                let p = palette(&[(name, color)]);
                let ranges = if needle.len() == 3 {
                    vec![start..start + 3]
                } else {
                    vec![start..start + 3, start + 4..start + needle.len()]
                };
                let expected: Vec<_> = ranges.iter().cloned().map(|r| (r, fg(color))).collect();
                assert_eq!(syntax.highlights(&rope, &ranges, &p), expected);
            }
        }
    }

    #[test]
    fn heex_nested_edits_match_fresh_parses() {
        for (file, original) in [
            (
                "/a.ex",
                "é = ~H|<div>{String.upcase(\"é\")}</div>|\n~H|<p><%= 42 %></p>|",
            ),
            (
                "/a.heex",
                "<div>{String.upcase(\"é\")}</div>\n<p><%= 42 %></p>",
            ),
            (
                "/a.md",
                "> ```elixir\n> é = ~H|<div>{String.upcase(\"é\")}</div>|\n> ```",
            ),
        ] {
            let path = Path::new(file);
            let mut buffers = crate::buffer::Buffers::default();
            let id = buffers.create_for_file(path, original);
            let b = buffers.get_mut(id).unwrap();
            let p = crate::settings::Settings::default().theme.syntax;
            let check = |b: &mut crate::buffer::Buffer| {
                b.reparse();
                let fresh = Syntax::for_file(path, b.text()).unwrap();
                assert_eq!(
                    b.highlights(&[0..b.len_chars()], &p),
                    fresh.highlights(b.text(), &[0..b.len_chars()], &p),
                    "{file}: {}",
                    b.text()
                );
            };
            check(b);
            for (from, to) in [
                ("é", "😀\n"),
                ("upcase", "downcase"),
                ("(\"é\")", "(42)"),
                ("{String", "String"),
                ("~H", "~s"),
                ("<div>", "<%= if true do %><div>"),
                ("</div>", "</div><% end %>"),
                ("|<", "|\n<"),
            ] {
                let text = b.text().to_string();
                let Some(byte) = text.find(from) else {
                    continue;
                };
                let start = text[..byte].chars().count();
                b.undo_boundary(0);
                b.delete(start..start + from.chars().count()).unwrap();
                b.insert(start, to).unwrap();
                b.undo_boundary(0);
                check(b);
                b.undo();
                check(b);
                b.redo();
                check(b);
                b.undo();
                check(b);
            }
            b.delete(0..b.len_chars()).unwrap();
            check(b);
            b.insert(0, original).unwrap();
            check(b);
        }
    }

    fn local_faces(file: &str, text: &str, name: &str) -> Vec<Option<u32>> {
        let rope = Rope::from_str(text);
        let syntax = Syntax::for_file(Path::new(file), &rope).unwrap();
        let p = palette(&[
            ("variable.parameter", 1),
            ("variable", 2),
            ("variable.builtin", 3),
            ("function", 4),
            ("function.builtin", 5),
        ]);
        let runs = syntax.highlights(&rope, &[0..rope.len_chars()], &p);
        text.match_indices(name)
            .map(|(byte, _)| {
                let at = rope.byte_to_char(byte);
                runs.iter()
                    .find(|(r, _)| r.contains(&at))
                    .and_then(|(_, f)| f.fg)
            })
            .collect()
    }

    #[test]
    fn rust_parameters_and_shadowed_locals_keep_their_definition_faces() {
        let text = "fn f(value: i32) { value; { let value = value; value; } value; let value = value; value; } fn g() { value; }";
        assert_eq!(
            local_faces("a.rs", text, "value"),
            [
                Some(1),
                Some(1),
                Some(2),
                Some(1),
                Some(2),
                Some(1),
                Some(2),
                Some(1),
                Some(2),
                None
            ]
        );
        let text = "fn f(value: i32) { let f = || value; let g = |value| value; value; fn nested() { value; } }";
        assert_eq!(
            local_faces("a.rs", text, "value"),
            [Some(1), Some(1), Some(1), Some(1), Some(1), None]
        );
    }

    #[test]
    fn typescript_parameters_and_javascript_builtins_resolve_locally() {
        for ext in ["ts", "tsx"] {
            let text = "function f(console: number) { console; { let console = 1; console; } console; } console;";
            assert_eq!(
                local_faces(&format!("a.{ext}"), text, "console"),
                [Some(1), Some(1), Some(2), Some(2), Some(1), Some(3)]
            );
        }
        let text = "console; function f(console) { console; } console; { let require = 1; require(); } require();";
        assert_eq!(
            local_faces("a.js", text, "console"),
            [Some(3), Some(2), Some(2), Some(3)]
        );
        assert_eq!(
            local_faces("a.js", text, "require"),
            [Some(2), Some(2), Some(5)]
        );
    }

    #[test]
    fn offscreen_local_definitions_use_the_current_palette_and_partial_ranges() {
        let text = "// é\nfn f(value: i32) {\n value;\n value;\n}";
        let rope = Rope::from_str(text);
        let syntax = Syntax::for_file(Path::new("a.rs"), &rope).unwrap();
        let start = rope.byte_to_char(text.rfind("value").unwrap()) + 1;
        for p in [
            palette(&[("variable.parameter", 1)]),
            palette(&[("variable", 2)]),
        ] {
            assert_eq!(
                syntax.highlights(&rope, &[start..start + 3], &p),
                [(start..start + 3, p.values().next().copied().unwrap())]
            );
        }
    }

    #[test]
    fn markdown_fences_have_separate_local_bindings() {
        let text = "```rust\nfn f(value: i32) { value; }\n```\n\n```rust\nfn g() { value; }\n```";
        assert_eq!(local_faces("a.md", text, "value"), [Some(1), Some(1), None]);
    }

    #[test]
    fn local_bindings_follow_unicode_edits_undo_and_redo() {
        for (file, text) in [
            ("/a.rs", "fn f(été: i32) { été; }"),
            ("/a.md", "```rust\nfn f(été: i32) { été; }\n```"),
        ] {
            let mut buffers = crate::buffer::Buffers::default();
            let id = buffers.create_for_file(Path::new(file), text);
            let b = buffers.get_mut(id).unwrap();
            let p = palette(&[("variable.parameter", 1)]);
            let reference = text[..text.rfind("été").unwrap()].chars().count();
            let check = |b: &mut crate::buffer::Buffer, start, expected| {
                b.reparse();
                let mut faces = [Face::default(); 3];
                for (range, face) in b.highlights(&[start..start + 3], &p) {
                    faces[range.start - start..range.end - start].fill(face);
                }
                assert_eq!(faces, [expected; 3]);
            };
            check(b, reference, fg(1));
            let definition = text[..text.find("été").unwrap()].chars().count();
            b.undo_boundary(0);
            b.delete(definition..definition + 3).unwrap();
            b.insert(definition, "étéx").unwrap();
            b.undo_boundary(0);
            check(b, reference + 1, Face::default());
            b.undo();
            check(b, reference, fg(1));
            b.redo();
            check(b, reference + 1, Face::default());
        }
    }

    #[test]
    fn markdown_inline_faces_keep_markup_and_heading_context() {
        let text = "# é *one **two** three* `code` [docs](https://x.dev)\n";
        let faces = faces("a.md", text);
        let p = crate::settings::Settings::default().theme.syntax;
        assert_eq!(
            face_at(text, &faces, "one"),
            p["text.emphasis"].over(p["text.title"])
        );
        assert_eq!(
            face_at(text, &faces, "two"),
            p["text.strong"].over(p["text.emphasis"].over(p["text.title"]))
        );
        assert_eq!(
            face_at(text, &faces, "*"),
            p["text.emphasis"].over(p["text.title"])
        );
        assert_eq!(
            face_at(text, &faces, "`code`"),
            p["text.literal"].over(p["text.title"])
        );
        assert_eq!(
            face_at(text, &faces, "docs"),
            p["text.reference"].over(p["text.title"])
        );
        assert_eq!(
            face_at(text, &faces, "https"),
            p["text.uri"].over(p["text.title"])
        );
    }

    #[test]
    fn markdown_fences_use_each_languages_faces() {
        for (language, file, code) in [
            ("rust", "a.rs", "fn main() { let s = \"é\"; }\n"),
            (
                "elixir",
                "a.ex",
                "defmodule Foo do\n  def run, do: :ok\nend\n",
            ),
            ("js", "a.js", "const f = (x) => x + 1;\n"),
            ("ts", "a.ts", "const x: number = 1;\n"),
            ("yaml", "a.yaml", "a: true\n"),
            ("toml", "a.toml", "[package]\nname = \"é\"\n"),
        ] {
            let prefix = format!("é\n\n```{language} extra\n");
            let text = format!("{prefix}{code}```\n");
            let highlighted = faces("a.md", &text);
            let start = prefix.chars().count();
            assert_eq!(
                &highlighted[start..start + code.chars().count()],
                faces(file, code),
                "{language}"
            );
            let literal = crate::settings::Settings::default().theme.syntax["text.literal"];
            assert_eq!(face_at(&text, &highlighted, "```"), literal);
            assert_eq!(highlighted[highlighted.len() - 2], literal);
        }
    }

    #[test]
    fn markdown_unknown_unlabeled_and_empty_fences() {
        let literal = crate::settings::Settings::default().theme.syntax["text.literal"];
        for label in ["unknown", "", "rusty"] {
            let text = format!("```{label}\nfn main() {{}} *plain*\n```\n");
            assert_eq!(faces("a.md", &text), vec![literal; text.chars().count()]);
        }
        let text = "```rust\n```\n\n*after*\n";
        assert!(face_at(text, &faces("a.md", text), "after").italic);
    }

    #[test]
    fn markdown_continuations_tables_and_list_fences() {
        let text = "> *é\n> continued*\n\n| Header |\n| --- |\n| **cell** |\n\n- ```rust\n  fn main() {}\n  ```\n\n> ```rust\n> let s = \"é\n> continued\";\n> ```\n";
        let highlighted = faces("a.md", text);
        let p = crate::settings::Settings::default().theme.syntax;
        assert_eq!(
            face_at(text, &highlighted, "continued*"),
            p["text.emphasis"]
        );
        assert_eq!(
            face_at(text, &highlighted, "> continued*"),
            p["punctuation.special"]
        );
        assert_eq!(face_at(text, &highlighted, "cell"), p["text.strong"]);
        assert_eq!(face_at(text, &highlighted, "fn"), p["keyword"]);
        assert_eq!(face_at(text, &highlighted, "continued\""), p["string"]);
        assert_eq!(
            face_at(text, &highlighted, "> continued\""),
            p["punctuation.special"]
        );
    }

    #[test]
    fn markdown_visible_ranges_keep_injected_string_context() {
        let text = "```rust\nlet s = \"é\ncontinued\";\n```\n";
        let rope = Rope::from_str(text);
        let syntax = Syntax::for_file(Path::new("a.md"), &rope).unwrap();
        let start = text[..text.find("continued").unwrap()].chars().count();
        let p = crate::settings::Settings::default().theme.syntax;
        assert_eq!(
            syntax.highlights(&rope, &[start..start + 3, start + 4..start + 7], &p),
            [
                (start..start + 3, p["string"]),
                (start + 4..start + 7, p["string"]),
            ]
        );
    }

    #[test]
    fn markdown_edits_match_fresh_parses() {
        let mut buffers = crate::buffer::Buffers::default();
        let path = Path::new("/a.md");
        let id = buffers.create_for_file(path, "é *one*\n\n```rust\nfn main() {}\n```\n");
        let b = buffers.get_mut(id).unwrap();
        let p = crate::settings::Settings::default().theme.syntax;
        let check = |b: &mut crate::buffer::Buffer| {
            b.reparse();
            let fresh = Syntax::for_file(path, b.text()).unwrap();
            assert_eq!(
                b.highlights(&[0..b.len_chars()], &p),
                fresh.highlights(b.text(), &[0..b.len_chars()], &p),
                "{}",
                b.text()
            );
        };
        check(b);
        for (from, to) in [
            ("é", "😀\n\n"),
            ("one", "two **bold**"),
            ("fn main() {}", "fn main() { let s = \"é\"; }"),
            ("rust", "unknown"),
            ("unknown", "elixir"),
            ("fn main() { let s = \"é\"; }", "defmodule Foo do end"),
            ("```elixir", "elixir"),
            ("elixir", "~~~js"),
            ("~~~js", "```rust"),
        ] {
            let text = b.text().to_string();
            let start = text[..text.find(from).unwrap()].chars().count();
            b.undo_boundary(0);
            b.delete(start..start + from.chars().count()).unwrap();
            b.insert(start, to).unwrap();
            b.undo_boundary(0);
            check(b);
            b.undo();
            check(b);
            b.redo();
            check(b);
        }
        b.delete(0..b.len_chars()).unwrap();
        check(b);
        b.insert(0, "```ts\nconst x: number = 1;\n```\n\n*new*\n")
            .unwrap();
        check(b);
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
