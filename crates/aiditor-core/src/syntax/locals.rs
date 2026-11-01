use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Range;
use std::sync::Arc;

use ropey::Rope;
use tree_sitter::{InputEdit, Node, QueryCursor, StreamingIterator, Tree};

use super::{Grammar, Locals, NodeRange, local_pattern};

#[derive(Clone)]
enum Kind {
    Definition(usize),
    Reference,
}

#[derive(Clone)]
struct Event {
    range: Range<usize>,
    kind: Kind,
}

struct Fragment {
    id: usize,
    len: usize,
    scope: Option<bool>,
    expanded: bool,
    events: Vec<Event>,
    children: Vec<(usize, Arc<Fragment>)>,
}

impl Fragment {
    fn ready(&self, start: usize, bytes: &Range<usize>) -> bool {
        if start >= bytes.end || start + self.len <= bytes.start {
            return true;
        }
        self.expanded
            && self
                .children
                .iter()
                .all(|(at, c)| c.ready(start + at, bytes))
    }

    fn index(self: &Arc<Self>, index: &mut HashMap<usize, Arc<Self>>) {
        index.insert(self.id, self.clone());
        for (_, child) in &self.children {
            child.index(index);
        }
    }

    fn events(&self, offset: usize, out: &mut Vec<Event>) {
        out.extend(self.events.iter().map(|e| Event {
            range: e.range.start + offset..e.range.end + offset,
            kind: match e.kind {
                Kind::Definition(end) => Kind::Definition(end + offset),
                Kind::Reference => Kind::Reference,
            },
        }));
        for (at, child) in &self.children {
            if child.scope.is_none() {
                child.events(offset + at, out);
            }
        }
    }
}

struct Definition {
    start: usize,
    value_end: usize,
    capture: Option<u32>,
}

struct Facts {
    definitions: BTreeMap<String, Vec<Definition>>,
    nodes: Vec<(NodeRange, String, bool)>,
}

type Environment = Vec<(usize, i128)>;

#[derive(Clone, Default)]
pub(super) struct Cache {
    root: Option<Arc<Fragment>>,
    // Pins the allocations whose node identities the immutable fragments refer to.
    tree: Option<Tree>,
    edited: HashSet<usize>,
    facts: HashMap<usize, Arc<Facts>>,
    resolved: HashMap<usize, (Environment, Arc<Locals>)>,
    #[cfg(test)]
    pub(super) rebuilt: usize,
}

impl Cache {
    pub(super) fn edit(&mut self, tree: &Tree, edit: &InputEdit) {
        let mut node = tree
            .root_node()
            .descendant_for_byte_range(edit.start_byte, edit.old_end_byte);
        while let Some(n) = node {
            self.edited.insert(n.id());
            node = n.parent();
        }
    }

    pub(super) fn locals(
        &mut self,
        grammar: &Grammar,
        tree: &Tree,
        text: &Rope,
        bytes: Range<usize>,
    ) -> Locals {
        if grammar.locals.is_none() {
            return Locals::new();
        }
        let root = tree.root_node();
        if !self.edited.is_empty()
            || self
                .root
                .as_ref()
                .is_none_or(|r| r.id != root.id() || !r.ready(root.start_byte(), &bytes))
        {
            let mut old = HashMap::new();
            if let Some(root) = &self.root {
                root.index(&mut old);
            }
            for id in &self.edited {
                old.remove(id);
            }
            let root = fragment(grammar, root, text, &bytes, &old, true);
            let mut live = HashMap::new();
            root.index(&mut live);
            self.facts
                .retain(|id, _| live.contains_key(id) && !self.edited.contains(id));
            self.resolved
                .retain(|id, _| live.contains_key(id) && !self.edited.contains(id));
            self.root = Some(root);
            self.tree = Some(tree.clone());
            self.edited.clear();
        }
        let root = self.root.as_ref().unwrap().clone();
        let mut out = Locals::new();
        self.walk(
            grammar,
            tree,
            text,
            &root,
            tree.root_node().start_byte(),
            &bytes,
            &[],
            &mut out,
        );
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn walk(
        &mut self,
        grammar: &Grammar,
        tree: &Tree,
        text: &Rope,
        node: &Arc<Fragment>,
        start: usize,
        bytes: &Range<usize>,
        parents: &[(usize, usize, Arc<Facts>)],
        out: &mut Locals,
    ) {
        if start >= bytes.end || start + node.len <= bytes.start {
            return;
        }
        let mut chain = parents.to_vec();
        if let Some(inherits) = node.scope {
            if !inherits {
                chain.clear();
            }
            let facts = self
                .facts
                .entry(node.id)
                .or_insert_with(|| {
                    #[cfg(test)]
                    {
                        self.rebuilt += 1;
                    }
                    Arc::new(facts(grammar, tree, text, node, start))
                })
                .clone();
            chain.push((node.id, start, facts));
            let env: Environment = chain
                .iter()
                .map(|(id, at, _)| (*id, *at as i128 - start as i128))
                .collect();
            if self
                .resolved
                .get(&node.id)
                .is_none_or(|(old, _)| old != &env)
            {
                let mut locals = Locals::new();
                let own = &chain.last().unwrap().2;
                for ((from, to), name, definition) in &own.nodes {
                    let at = start + from;
                    for (_, origin, scope) in chain.iter().rev() {
                        if let Some(def) = scope.definitions.get(name).and_then(|defs| {
                            defs.iter().rev().find(|d| {
                                origin + d.start <= at
                                    && (*definition && *origin == start && d.start == *from
                                        || origin + d.value_end <= at)
                            })
                        }) {
                            locals.insert((*from, *to), def.capture);
                            break;
                        }
                    }
                }
                self.resolved.insert(node.id, (env, Arc::new(locals)));
            }
            let locals = &self.resolved[&node.id].1;
            out.extend(locals.iter().filter_map(|(&(a, b), &face)| {
                (start + a < bytes.end && start + b > bytes.start)
                    .then_some(((start + a, start + b), face))
            }));
        }
        for (offset, child) in &node.children {
            self.walk(
                grammar,
                tree,
                text,
                child,
                start + offset,
                bytes,
                &chain,
                out,
            );
        }
    }
}

fn fragment(
    grammar: &Grammar,
    node: Node,
    text: &Rope,
    bytes: &Range<usize>,
    old: &HashMap<usize, Arc<Fragment>>,
    root: bool,
) -> Arc<Fragment> {
    let id = node.id();
    let previous = old.get(&id);
    if let Some(old) = previous
        && old.ready(node.start_byte(), bytes)
    {
        return old.clone();
    }
    let start = node.start_byte();
    let (scope, events) = previous.map_or_else(
        || {
            let query = grammar.locals.as_ref().unwrap();
            let mut cursor = QueryCursor::new();
            cursor.set_max_start_depth(Some(1));
            let source = |n: Node| text.byte_slice(n.byte_range()).chunks().map(str::as_bytes);
            let mut matches = cursor.matches(query, node, source);
            let mut scope = root.then_some(false);
            let mut events = Vec::new();
            while let Some(m) = matches.next() {
                let value_end = m.captures().iter().find_map(|c| {
                    (query.capture_names()[c.index as usize] == "local.definition-value")
                        .then_some(c.node.end_byte())
                });
                for c in m.captures() {
                    let kind = match query.capture_names()[c.index as usize] {
                        "local.scope" if c.node.id() == node.id() => {
                            scope =
                                Some(!query.property_settings(m.pattern_index).iter().any(|p| {
                                    p.key.as_ref() == "local.scope-inherits"
                                        && p.value.as_deref() == Some("false")
                                }));
                            continue;
                        }
                        "local.definition" => {
                            Kind::Definition(value_end.unwrap_or(c.node.start_byte()) - start)
                        }
                        "local.reference" => Kind::Reference,
                        _ => continue,
                    };
                    events.push(Event {
                        range: c.node.start_byte() - start..c.node.end_byte() - start,
                        kind,
                    });
                }
            }
            (scope, events)
        },
        |p| (p.scope, p.events.clone()),
    );
    let expanded = root || scope.is_none() || (start < bytes.end && node.end_byte() > bytes.start);
    let children = if expanded {
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .map(|child| {
                (
                    child.start_byte() - start,
                    fragment(grammar, child, text, bytes, old, false),
                )
            })
            .collect()
    } else {
        Vec::new()
    };
    Arc::new(Fragment {
        id,
        len: node.byte_range().len(),
        scope,
        expanded,
        events,
        children,
    })
}

fn facts(grammar: &Grammar, tree: &Tree, text: &Rope, node: &Fragment, start: usize) -> Facts {
    let mut events = Vec::new();
    node.events(0, &mut events);
    events.sort_by_key(|e| {
        (
            e.range.start,
            std::cmp::Reverse(e.range.end),
            usize::from(matches!(e.kind, Kind::Reference)),
        )
    });
    events.dedup_by(|a, b| {
        a.range == b.range && std::mem::discriminant(&a.kind) == std::mem::discriminant(&b.kind)
    });
    let mut captures: BTreeMap<NodeRange, Option<(usize, u32)>> = events
        .iter()
        .filter(|e| matches!(e.kind, Kind::Definition(_)))
        .map(|e| ((start + e.range.start, start + e.range.end), None))
        .collect();
    if let (Some((&(first, _), _)), Some((&(_, end), _))) =
        (captures.first_key_value(), captures.last_key_value())
    {
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(first..end);
        let source = |n: Node| text.byte_slice(n.byte_range()).chunks().map(str::as_bytes);
        let mut matches = cursor.matches(&grammar.highlights, tree.root_node(), source);
        while let Some(m) = matches.next() {
            if local_pattern(&grammar.highlights, m.pattern_index, true) {
                for c in m.captures() {
                    if let Some(capture) =
                        captures.get_mut(&(c.node.start_byte(), c.node.end_byte()))
                        && capture.is_none_or(|(p, _)| p <= m.pattern_index)
                    {
                        *capture = Some((m.pattern_index, c.index));
                    }
                }
            }
        }
    }
    let mut facts = Facts {
        definitions: BTreeMap::new(),
        nodes: Vec::new(),
    };
    for e in events {
        let key = (e.range.start, e.range.end);
        let name = text
            .byte_slice(start + e.range.start..start + e.range.end)
            .to_string();
        let definition = matches!(e.kind, Kind::Definition(_));
        if let Kind::Definition(value_end) = e.kind {
            facts
                .definitions
                .entry(name.clone())
                .or_default()
                .push(Definition {
                    start: e.range.start,
                    value_end,
                    capture: captures[&(start + key.0, start + key.1)].map(|(_, c)| c),
                });
        }
        if facts.nodes.last().is_none_or(|(last, _, _)| *last != key) {
            facts.nodes.push((key, name, definition));
        }
    }
    facts
}
