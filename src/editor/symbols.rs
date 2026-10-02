use std::rc::Rc;

use gpui::{
    App, ElementId, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
};

use crate::{
    buttons::SegmentedControl,
    i18n,
    lists::{Tree, TreeNode},
    navigation::{Group, Palette, Row, fuzzy, marked, query_field},
    primitives::IconName,
    theme::{ActiveTheme, ControlSize, Palette as Colors},
};

/// What a symbol in code is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    Module,
    Struct,
    Enum,
    Trait,
    Function,
    Method,
    Field,
    Variable,
    Constant,
    Keyword,
    Snippet,
}

impl SymbolKind {
    pub fn icon(self) -> IconName {
        match self {
            SymbolKind::Module => IconName::Package,
            SymbolKind::Struct => IconName::Braces,
            SymbolKind::Enum => IconName::ListOrdered,
            SymbolKind::Trait => IconName::Shapes,
            SymbolKind::Function | SymbolKind::Method => IconName::SquareFunction,
            SymbolKind::Field => IconName::Tag,
            SymbolKind::Variable => IconName::Variable,
            SymbolKind::Constant => IconName::Hash,
            SymbolKind::Keyword => IconName::Key,
            SymbolKind::Snippet => IconName::Code,
        }
    }

    /// The syntax color its name takes.
    pub fn color(self, colors: &Colors) -> Hsla {
        let syntax = &colors.syntax;
        match self {
            SymbolKind::Module | SymbolKind::Snippet => colors.fg_muted,
            SymbolKind::Struct | SymbolKind::Enum | SymbolKind::Trait => syntax.type_name,
            SymbolKind::Function | SymbolKind::Method => syntax.function,
            SymbolKind::Field => syntax.property,
            SymbolKind::Variable => syntax.variable,
            SymbolKind::Constant => syntax.constant,
            SymbolKind::Keyword => syntax.keyword,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            SymbolKind::Module => "module",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Trait => "trait",
            SymbolKind::Function => "function",
            SymbolKind::Method => "method",
            SymbolKind::Field => "field",
            SymbolKind::Variable => "variable",
            SymbolKind::Constant => "constant",
            SymbolKind::Keyword => "keyword",
            SymbolKind::Snippet => "snippet",
        }
    }
}

/// A symbol in a file: its name, kind, the line it starts on, a detail such as its type, and what it holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Symbol {
    pub name: SharedString,
    pub kind: SymbolKind,
    pub line: usize,
    pub detail: Option<SharedString>,
    pub children: Vec<Symbol>,
}

impl Symbol {
    pub fn new(name: impl Into<SharedString>, kind: SymbolKind, line: usize) -> Self {
        Self {
            name: name.into(),
            kind,
            line,
            detail: None,
            children: Vec::new(),
        }
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn children(mut self, children: impl IntoIterator<Item = Symbol>) -> Self {
        self.children = children.into_iter().collect();
        self
    }
}

type OnLine = Rc<dyn Fn(usize, &mut Window, &mut App)>;

fn key(symbol: &Symbol) -> SharedString {
    format!("{}:{}", symbol.line, symbol.name).into()
}

fn node(symbol: &Symbol) -> TreeNode {
    let tree = TreeNode::new(key(symbol), symbol.name.clone()).icon(symbol.kind.icon());
    let tree = match &symbol.detail {
        Some(detail) => tree.note(detail.clone()),
        None => tree,
    };
    if symbol.children.is_empty() {
        return tree;
    }
    tree.children(symbol.children.iter().map(node))
}

/// Each symbol with the path of those holding it, outermost first.
pub(crate) fn flattened(symbols: &[Symbol]) -> Vec<(Vec<SharedString>, &Symbol)> {
    fn walk<'a>(
        symbols: &'a [Symbol],
        path: &mut Vec<SharedString>,
        out: &mut Vec<(Vec<SharedString>, &'a Symbol)>,
    ) {
        for symbol in symbols {
            out.push((path.clone(), symbol));
            path.push(symbol.name.clone());
            walk(&symbol.children, path, out);
            path.pop();
        }
    }
    let mut out = Vec::new();
    walk(symbols, &mut Vec::new(), &mut out);
    out
}

/// The innermost symbol at or above a line.
pub(crate) fn enclosing(symbols: &[Symbol], line: usize) -> Option<&Symbol> {
    let mut best = None;
    for (_, symbol) in flattened(symbols) {
        if symbol.line <= line && best.is_none_or(|best: &Symbol| symbol.line >= best.line) {
            best = Some(symbol);
        }
    }
    best
}

/// A file's symbols as a tree filling its box, the one around the cursor chosen; a press goes to its line.
#[derive(IntoElement)]
pub struct SymbolOutline {
    id: ElementId,
    symbols: Vec<Symbol>,
    cursor: Option<usize>,
    on_pick: Option<OnLine>,
}

impl SymbolOutline {
    pub fn new(id: impl Into<ElementId>, symbols: impl IntoIterator<Item = Symbol>) -> Self {
        Self {
            id: id.into(),
            symbols: symbols.into_iter().collect(),
            cursor: None,
            on_pick: None,
        }
    }

    /// The cursor's line, whose symbol is chosen.
    pub fn cursor(mut self, line: usize) -> Self {
        self.cursor = Some(line);
        self
    }

    pub fn on_pick(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_pick = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for SymbolOutline {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let open: Vec<SharedString> = flattened(&self.symbols)
            .into_iter()
            .filter(|(_, symbol)| !symbol.children.is_empty())
            .map(|(_, symbol)| key(symbol))
            .collect();
        let chosen = self
            .cursor
            .and_then(|line| enclosing(&self.symbols, line))
            .map(key);
        let lines: Vec<(SharedString, usize)> = flattened(&self.symbols)
            .into_iter()
            .map(|(_, symbol)| (key(symbol), symbol.line))
            .collect();
        let tree = Tree::new(self.id, self.symbols.iter().map(node))
            .size_full()
            .open(open)
            .selected(chosen);
        match self.on_pick {
            Some(on_pick) => tree.on_select(move |keys, window, cx| {
                let Some(picked) = keys.first() else {
                    return;
                };
                let line = lines
                    .iter()
                    .find(|(key, _)| key == picked)
                    .map(|(_, line)| *line)
                    .expect("a picked symbol is in the outline");
                log::info!("symbol outline: line {line}");
                on_pick(line, window, cx);
            }),
            None => tree,
        }
    }
}

type OnClose = Rc<dyn Fn(&mut Window, &mut App)>;
type OnFlip = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// A palette of a file's symbols by name, each with its kind and what holds it; `@` style jumping.
#[derive(IntoElement)]
pub struct GoToSymbol {
    id: ElementId,
    symbols: Vec<Symbol>,
    on_pick: Option<OnLine>,
    on_close: OnClose,
}

impl GoToSymbol {
    /// Render it while open; `on_close` runs on Escape, a click outside, or a pick.
    pub fn new(
        id: impl Into<ElementId>,
        symbols: impl IntoIterator<Item = Symbol>,
        on_close: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            symbols: symbols.into_iter().collect(),
            on_pick: None,
            on_close: Rc::new(on_close),
        }
    }

    pub fn on_pick(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_pick = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for GoToSymbol {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let input = query_field(
            &self.id,
            i18n::text(cx, "palette.symbol.placeholder", &[]),
            window,
            cx,
        );
        let query = input.read(cx).text().trim().to_string();
        let colors = cx.theme().colors.clone();
        let mut found: Vec<(i32, Row)> = flattened(&self.symbols)
            .into_iter()
            .filter_map(|(path, symbol)| {
                let fit = if query.is_empty() {
                    None
                } else {
                    Some(fuzzy(&query, &symbol.name)?)
                };
                let (score, hits) = fit.map_or((0, Vec::new()), |fit| (fit.score, fit.hits));
                let detail = (!path.is_empty()).then(|| {
                    div()
                        .text_color(colors.fg_subtle)
                        .child(path.join(" › "))
                        .into_any_element()
                });
                let row = Row {
                    value: symbol.line.to_string().into(),
                    name: symbol.name.clone(),
                    icon: Some(symbol.kind.icon()),
                    label: marked(symbol.name.clone(), hits, cx),
                    detail,
                    end: Some(
                        div()
                            .text_color(colors.fg_subtle)
                            .child(symbol.kind.word())
                            .into_any_element(),
                    ),
                };
                Some((score, row))
            })
            .collect();
        if !query.is_empty() {
            found.sort_by(|(a, _), (b, _)| b.cmp(a));
        }
        let on_pick = self.on_pick.map(|on_pick| {
            Rc::new(
                move |value: &SharedString, window: &mut Window, cx: &mut App| {
                    let line: usize = value.parse().expect("a symbol row carries its line");
                    on_pick(line, window, cx);
                },
            ) as Rc<dyn Fn(&SharedString, &mut Window, &mut App)>
        });
        Palette {
            id: self.id,
            input,
            groups: vec![Group {
                title: None,
                rows: found.into_iter().map(|(_, row)| row).collect(),
            }],
            start: 0,
            empty: i18n::text(cx, "palette.symbol.empty", &[]),
            on_pick,
            on_close: self.on_close,
        }
        .overlay(window, cx)
    }
}

/// A call in a hierarchy: who, of what kind, where, and the calls under it.
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub name: SharedString,
    pub kind: SymbolKind,
    pub place: SharedString,
    pub calls: Vec<Call>,
}

/// Who calls a function or whom it calls, or a type's supertypes and subtypes, as a tree filling its box that flips between the two directions.
#[derive(IntoElement)]
pub struct CallHierarchy {
    id: ElementId,
    root: Call,
    labels: (SharedString, SharedString),
    incoming: bool,
    on_flip: Option<OnFlip>,
}

impl CallHierarchy {
    /// Calls to or from `root`; `incoming` picks which the tree shows.
    pub fn new(id: impl Into<ElementId>, root: Call, incoming: bool) -> Self {
        Self {
            id: id.into(),
            root,
            labels: ("Incoming".into(), "Outgoing".into()),
            incoming,
            on_flip: None,
        }
    }

    /// For types: supertypes and subtypes instead of calls.
    pub fn types(mut self) -> Self {
        self.labels = ("Supertypes".into(), "Subtypes".into());
        self
    }

    /// Gets whether the incoming side is chosen.
    pub fn on_flip(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_flip = Some(Rc::new(handler));
        self
    }
}

/// A call's node key: its callers' keys, its name and its place, so two calls of one name stay apart.
fn call_key(call: &Call, path: &str) -> String {
    format!("{path}/{}@{}", call.name, call.place)
}

fn call_node(call: &Call, path: &str) -> TreeNode {
    let key = call_key(call, path);
    let node = TreeNode::new(key.clone(), call.name.clone())
        .icon(call.kind.icon())
        .note(call.place.clone());
    if call.calls.is_empty() {
        return node;
    }
    node.children(call.calls.iter().map(|inner| call_node(inner, &key)))
}

fn keys(call: &Call, path: &str, out: &mut Vec<SharedString>) {
    let key = call_key(call, path);
    for inner in &call.calls {
        keys(inner, &key, out);
    }
    out.push(key.into());
}

impl RenderOnce for CallHierarchy {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let (incoming, outgoing) = self.labels;
        let side = SegmentedControl::new(
            (self.id.clone(), "side"),
            if self.incoming { "in" } else { "out" },
        )
        .size(ControlSize::Sm)
        .segment("in", incoming, None)
        .segment("out", outgoing, None);
        let side = match self.on_flip {
            Some(on_flip) => {
                side.on_change(move |key, window, cx| on_flip(key.as_ref() == "in", window, cx))
            }
            None => side,
        };
        let mut open = Vec::new();
        keys(&self.root, "", &mut open);
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .text_size(theme.text_size(crate::theme::TextSize::Sm))
            .child(side)
            .child(
                Tree::new((self.id, "calls"), [call_node(&self.root, "")])
                    .flex_1()
                    .min_h_0()
                    .open(open),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calls_of_one_name_from_two_places_keep_their_own_keys() {
        let call = |name: &str, place: &str| Call {
            name: name.to_string().into(),
            kind: SymbolKind::Function,
            place: place.to_string().into(),
            calls: Vec::new(),
        };
        let root = Call {
            calls: vec![call("draw", "a.rs:1"), call("draw", "b.rs:9")],
            ..call("main", "main.rs:1")
        };
        let mut all = Vec::new();
        keys(&root, "", &mut all);
        let mut unique = all.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), all.len());
    }

    #[test]
    fn the_innermost_symbol_above_a_line_holds_it() {
        let symbols = vec![
            Symbol::new("Editor", SymbolKind::Struct, 2),
            Symbol::new("impl Editor", SymbolKind::Module, 10).children([
                Symbol::new("new", SymbolKind::Method, 11),
                Symbol::new("render", SymbolKind::Method, 20),
            ]),
        ];
        assert_eq!(
            enclosing(&symbols, 25).map(|symbol| &symbol.name[..]),
            Some("render")
        );
        assert_eq!(
            enclosing(&symbols, 5).map(|symbol| &symbol.name[..]),
            Some("Editor")
        );
        assert_eq!(enclosing(&symbols, 0), None);
        let paths: Vec<usize> = flattened(&symbols)
            .iter()
            .map(|(path, _)| path.len())
            .collect();
        assert_eq!(paths, [0, 0, 1, 1]);
    }
}
