use std::ops::Deref;
use std::rc::Rc;
use std::str::FromStr;

use anyhow::Result;
use gpui::{App, AppContext, Context, Entity, IntoElement, Render, SharedString, Styled as _, Subscription, Task, Window};
use gpui_component::highlighter::Language;
use gpui_component::input::{
    CodeActionProvider, CompletionProvider, HoverProvider, Input, InputEvent, InputState, TabSize,
};
use gpui_component::{Rope, RopeExt};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    Hover, HoverContents, InsertReplaceEdit, MarkedString, Range as LspRange, TextEdit, Uri,
    WorkspaceEdit,
};

/// Simple schema hints to improve autocomplete suggestions.
#[derive(Clone, Default)]
pub struct SqlSchema {
    pub tables: Vec<(String, String)>,   // (name, doc)
    pub columns: Vec<(String, String)>,  // global (name, doc)
    pub columns_by_table: std::collections::HashMap<String, Vec<(String, String)>>,
}

impl SqlSchema {
    pub fn with_tables(
        mut self,
        tables: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        self.tables = tables.into_iter().map(|(n, d)| (n.into(), d.into())).collect();
        self
    }
    pub fn with_columns(
        mut self,
        columns: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        self.columns = columns.into_iter().map(|(n, d)| (n.into(), d.into())).collect();
        self
    }
    pub fn with_table_columns(
        mut self,
        table: impl Into<String>,
        columns: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        self.columns_by_table.insert(
            table.into(),
            columns
                .into_iter()
                .map(|(n, d)| (n.into(), d.into()))
                .collect(),
        );
        self
    }
}

// Built-in SQL keywords and docs (trimmed for brevity vs example).
const SQL_KEYWORDS: &[&str] = &[
    "SELECT", "INSERT", "UPDATE", "DELETE", "CREATE", "ALTER", "DROP", "FROM", "WHERE",
    "JOIN", "LEFT", "RIGHT", "INNER", "GROUP", "ORDER", "BY", "HAVING", "LIMIT",
    "OFFSET", "VALUES", "INTO", "AND", "OR", "NOT", "IN", "EXISTS", "BETWEEN", "LIKE",
    "IS", "NULL", "AS", "DISTINCT", "UNION", "ALL", "ON",
];

const SQL_FUNCTIONS: &[(&str, &str)] = &[
    ("COUNT(*)", "Returns the number of rows"),
    ("SUM(x)", "Sum of values"),
    ("AVG(x)", "Average value"),
    ("MIN(x)", "Minimum value"),
    ("MAX(x)", "Maximum value"),
    ("NOW()", "Current timestamp"),
];

const SQL_KEYWORD_DOCS: &[(&str, &str)] = &[
    ("SELECT", "Query rows from table(s)"),
    ("INSERT", "Insert new rows"),
    ("UPDATE", "Update existing rows"),
    ("DELETE", "Delete rows"),
    ("WHERE", "Filter rows with predicates"),
    ("JOIN", "Combine rows from tables"),
    ("GROUP", "Group rows for aggregation"),
    ("ORDER", "Sort result set"),
    ("LIMIT", "Limit number of rows"),
];

#[derive(Clone)]
pub struct DefaultSqlCompletionProvider {
    schema: SqlSchema,
}

impl DefaultSqlCompletionProvider {
    pub fn new(schema: SqlSchema) -> Self {
        Self { schema }
    }
}

impl CompletionProvider for DefaultSqlCompletionProvider {
    fn completions(
        &self,
        rope: &Rope,
        offset: usize,
        _trigger: CompletionContext,
        _window: &mut Window,
        cx: &mut Context<InputState>,
    ) -> Task<Result<CompletionResponse>> {
        let rope = rope.clone();
        let schema = self.schema.clone();

        cx.background_spawn(async move {
            // Current word
            let mut chars_before = vec![];
            for ch in rope.chars_at(0) {
                if chars_before.len() >= offset {
                    break;
                }
                chars_before.push(ch);
            }
            let word_start = chars_before
                .iter()
                .rev()
                .take_while(|c| c.is_alphanumeric() || **c == '_')
                .count();
            let start_offset = offset.saturating_sub(word_start);
            let current_word = rope
                .slice(start_offset..offset)
                .to_string()
                .to_uppercase();

            let start_pos = rope.offset_to_position(start_offset);
            let end_pos = rope.offset_to_position(offset);
            let replace_range = LspRange::new(start_pos, end_pos);

            let mut items = Vec::new();

            let before_text = rope.slice(0..offset).to_string().to_uppercase();
            let after_kw = before_text.contains(" FROM ")
                || before_text.contains(" JOIN ")
                || before_text.contains(" INTO ")
                || before_text.contains(" TABLE ");
            let suggest_tables = after_kw;
            let suggest_columns = before_text.contains(" SELECT ")
                || (before_text.contains(" SELECT ") && before_text.ends_with(", "));

            // Dot context: table.column
            let mut dot_table: Option<String> = None;
            {
                let slice = rope.slice(offset.saturating_sub(128)..offset).to_string();
                if let Some(dot_ix) = slice.rfind('.') {
                    let left = &slice[..dot_ix];
                    let mut t = String::new();
                    for ch in left.chars().rev() {
                        if ch.is_alphanumeric() || ch == '_' {
                            t.push(ch);
                        } else {
                            break;
                        }
                    }
                    if !t.is_empty() {
                        dot_table = Some(t.chars().rev().collect::<String>());
                    }
                }
            }

            // helper to compute matched prefix safely
            let matched_prefix = |label: &str| -> String {
                let lu = label.to_uppercase();
                if !current_word.is_empty() && lu.starts_with(&current_word) {
                    let count = current_word.chars().count();
                    label.chars().take(count).collect::<String>()
                } else {
                    String::new()
                }
            };

            // Keywords
            for keyword in SQL_KEYWORDS {
                if keyword.starts_with(&current_word) || current_word.is_empty() {
                    items.push(CompletionItem {
                        label: keyword.to_string(),
                        kind: Some(CompletionItemKind::KEYWORD),
                        text_edit: Some(CompletionTextEdit::InsertAndReplace(
                            InsertReplaceEdit {
                                new_text: keyword.to_string(),
                                insert: replace_range.clone(),
                                replace: replace_range.clone(),
                            },
                        )),
                        filter_text: Some(matched_prefix(keyword)),
                        documentation: SQL_KEYWORD_DOCS
                            .iter()
                            .find(|(k, _)| k == keyword)
                            .map(|(_, doc)| lsp_types::Documentation::String(doc.to_string())),
                        sort_text: Some(format!("1_{}", keyword)),
                        ..Default::default()
                    });
                }
            }

            // Functions
            for (func, doc) in SQL_FUNCTIONS {
                let func_name = func.split('(').next().unwrap_or("");
                if func_name.starts_with(&current_word) || current_word.is_empty() {
                    items.push(CompletionItem {
                        label: func.to_string(),
                        kind: Some(CompletionItemKind::FUNCTION),
                        text_edit: Some(CompletionTextEdit::InsertAndReplace(
                            InsertReplaceEdit {
                                new_text: func.to_string(),
                                insert: replace_range.clone(),
                                replace: replace_range.clone(),
                            },
                        )),
                        filter_text: Some(matched_prefix(func)),
                        documentation: Some(lsp_types::Documentation::String(doc.to_string())),
                        sort_text: Some(format!("2_{}", func)),
                        ..Default::default()
                    });
                }
            }

            // Tables
            if suggest_tables || current_word.is_empty() {
                for (table, doc) in &schema.tables {
                    let table_upper = table.to_uppercase();
                    if table_upper.starts_with(&current_word) || current_word.is_empty() {
                        items.push(CompletionItem {
                            label: table.clone(),
                            kind: Some(CompletionItemKind::STRUCT),
                            detail: Some("Table".to_string()),
                            text_edit: Some(CompletionTextEdit::InsertAndReplace(
                                InsertReplaceEdit {
                                    new_text: table.clone(),
                                    insert: replace_range.clone(),
                                    replace: replace_range.clone(),
                                },
                            )),
                            filter_text: Some(matched_prefix(&table)),
                            documentation: Some(lsp_types::Documentation::String(doc.clone())),
                            sort_text: Some(format!("0_{}", table)),
                            ..Default::default()
                        });
                    }
                }
            }

            // Columns (dot context first)
            if let Some(tname) = dot_table.clone() {
                if let Some(cols) = schema.columns_by_table.get(&tname) {
                    for (column, doc) in cols {
                        let column_upper = column.to_uppercase();
                        if column_upper.starts_with(&current_word) || current_word.is_empty() {
                            items.push(CompletionItem {
                                label: column.clone(),
                                kind: Some(CompletionItemKind::FIELD),
                                detail: Some(format!("{}.column", tname)),
                                text_edit: Some(CompletionTextEdit::InsertAndReplace(
                                    InsertReplaceEdit {
                                        new_text: column.clone(),
                                        insert: replace_range.clone(),
                                        replace: replace_range.clone(),
                                    },
                                )),
                                filter_text: Some(matched_prefix(&column)),
                                documentation: Some(lsp_types::Documentation::String(doc.clone())),
                                sort_text: Some(format!("0_{}", column)),
                                ..Default::default()
                            });
                        }
                    }
                }
            } else if suggest_columns || current_word.is_empty() {
                for (column, doc) in &schema.columns {
                    let column_upper = column.to_uppercase();
                    if column_upper.starts_with(&current_word) || current_word.is_empty() {
                        items.push(CompletionItem {
                            label: column.clone(),
                            kind: Some(CompletionItemKind::FIELD),
                            detail: Some("Column".to_string()),
                            text_edit: Some(CompletionTextEdit::InsertAndReplace(
                                InsertReplaceEdit {
                                    new_text: column.clone(),
                                    insert: replace_range.clone(),
                                    replace: replace_range.clone(),
                                },
                            )),
                            filter_text: Some(matched_prefix(&column)),
                            documentation: Some(lsp_types::Documentation::String(doc.clone())),
                            sort_text: Some(format!("0_{}", column)),
                            ..Default::default()
                        });
                    }
                }
            }

            items.sort_by(|a, b| {
                a.sort_text
                    .as_ref()
                    .unwrap_or(&a.label)
                    .cmp(b.sort_text.as_ref().unwrap_or(&b.label))
            });
            items.truncate(30);
            Ok(CompletionResponse::Array(items))
        })
    }

    fn is_completion_trigger(
        &self,
        _offset: usize,
        new_text: &str,
        _cx: &mut Context<InputState>,
    ) -> bool {
        if new_text.ends_with(";") {
            return false;
        }
        true
    }
}

#[derive(Clone)]
struct DefaultSqlHoverProvider;

impl HoverProvider for DefaultSqlHoverProvider {
    fn hover(
        &self,
        text: &Rope,
        offset: usize,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Task<Result<Option<Hover>>> {
        let word = text.word_at(offset).to_uppercase();

        for (keyword, doc) in SQL_KEYWORD_DOCS {
            if *keyword == word.as_str() {
                let hover = Hover {
                    contents: HoverContents::Scalar(MarkedString::String(format!(
                        "**{}**\n\n{}",
                        keyword, doc
                    ))),
                    range: None,
                };
                return Task::ready(Ok(Some(hover)));
            }
        }
        for (func, doc) in SQL_FUNCTIONS {
            let func_name = func.split('(').next().unwrap_or("");
            if func_name == word.as_str() {
                let hover = Hover {
                    contents: HoverContents::Scalar(MarkedString::String(format!(
                        "**{}**\n\n{}",
                        func, doc
                    ))),
                    range: None,
                };
                return Task::ready(Ok(Some(hover)));
            }
        }
        Task::ready(Ok(None))
    }
}

#[derive(Clone)]
struct SqlActionsProvider {
    /// Callback for executing SQL.
    on_execute: Option<Rc<dyn Fn(String, &mut Window, &mut gpui::App) + 'static>>,
}

impl SqlActionsProvider {
    fn new() -> Self {
        Self { on_execute: None }
    }
    #[allow(dead_code)]
    fn with_execute(
        mut self,
        f: Rc<dyn Fn(String, &mut Window, &mut gpui::App) + 'static>,
    ) -> Self {
        self.on_execute = Some(f);
        self
    }

    fn format_sql(sql: &str) -> String {
        let mut formatted = String::new();
        let mut indent_level = 0;
        let lines: Vec<&str> = sql.lines().collect();
        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.starts_with("FROM")
                || trimmed.starts_with("WHERE")
                || trimmed.starts_with("JOIN")
                || trimmed.starts_with("INNER")
                || trimmed.starts_with("LEFT")
                || trimmed.starts_with("RIGHT")
                || trimmed.starts_with("ORDER BY")
                || trimmed.starts_with("GROUP BY")
                || trimmed.starts_with("HAVING")
                || trimmed.starts_with("LIMIT")
            {
                indent_level = 0;
            }
            formatted.push_str(&"  ".repeat(indent_level));
            formatted.push_str(trimmed);
            formatted.push('\n');
            if trimmed.starts_with("SELECT") {
                indent_level = 1;
            }
        }
        formatted.trim_end().to_string()
    }

    fn minify_sql(sql: &str) -> String {
        sql.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn uppercase_keywords(sql: &str) -> String {
        let mut result = String::new();
        let mut current_word = String::new();
        let mut in_string = false;
        let mut string_char = ' ';
        for ch in sql.chars() {
            if (ch == '\'' || ch == '"') && !in_string {
                if !current_word.is_empty() {
                    result.push_str(&Self::uppercase_if_keyword(&current_word));
                    current_word.clear();
                }
                in_string = true;
                string_char = ch;
                result.push(ch);
                continue;
            } else if in_string && ch == string_char {
                in_string = false;
                result.push(ch);
                continue;
            }
            if in_string {
                result.push(ch);
                continue;
            }
            if ch.is_alphanumeric() || ch == '_' {
                current_word.push(ch);
            } else {
                if !current_word.is_empty() {
                    result.push_str(&Self::uppercase_if_keyword(&current_word));
                    current_word.clear();
                }
                result.push(ch);
            }
        }
        if !current_word.is_empty() {
            result.push_str(&Self::uppercase_if_keyword(&current_word));
        }
        result
    }

    fn uppercase_if_keyword(word: &str) -> String {
        let upper = word.to_uppercase();
        if SQL_KEYWORDS.contains(&upper.as_str()) {
            upper
        } else {
            word.to_string()
        }
    }
}

impl CodeActionProvider for SqlActionsProvider {
    fn id(&self) -> SharedString {
        "SqlActionsProvider".into()
    }

    fn code_actions(
        &self,
        state: Entity<InputState>,
        range: std::ops::Range<usize>,
        _window: &mut Window,
        _cx: &mut gpui::App,
    ) -> Task<Result<Vec<lsp_types::CodeAction>>> {
        let state_read = state.read(_cx);
        let document_uri = Uri::from_str("file://one-hub").unwrap();
        let mut actions = vec![];

        if !range.is_empty() {
            let old_text = state_read.text().slice(range.clone()).to_string();
            let start = state_read.text().offset_to_position(range.start);
            let end = state_read.text().offset_to_position(range.end);
            let lsp_range = lsp_types::Range { start, end };

            // Uppercase
            let new_text = Self::uppercase_keywords(&old_text);
            actions.push(lsp_types::CodeAction {
                title: "Uppercase Keywords".into(),
                kind: Some(lsp_types::CodeActionKind::REFACTOR),
                edit: Some(WorkspaceEdit {
                    changes: Some(
                        std::iter::once((
                            document_uri.clone(),
                            vec![TextEdit { range: lsp_range.clone(), new_text }],
                        ))
                        .collect(),
                    ),
                    document_changes: None,
                    change_annotations: None,
                }),
                ..Default::default()
            });

            // Minify
            let new_text = Self::minify_sql(&old_text);
            actions.push(lsp_types::CodeAction {
                title: "Minify SQL".into(),
                kind: Some(lsp_types::CodeActionKind::REFACTOR),
                edit: Some(WorkspaceEdit {
                    changes: Some(
                        std::iter::once((
                            document_uri.clone(),
                            vec![TextEdit { range: lsp_range.clone(), new_text }],
                        ))
                        .collect(),
                    ),
                    document_changes: None,
                    change_annotations: None,
                }),
                ..Default::default()
            });
        }

        // Format whole document
        let old_text = state_read.text().to_string();
        let new_text = Self::format_sql(&old_text);
        let start = state_read.text().offset_to_position(0);
        let end = state_read.text().offset_to_position(state_read.text().len());
        let lsp_range = lsp_types::Range { start, end };
        actions.push(lsp_types::CodeAction {
            title: "Format SQL".into(),
            kind: Some(lsp_types::CodeActionKind::REFACTOR),
            edit: Some(WorkspaceEdit {
                changes: Some(
                    std::iter::once((
                        document_uri.clone(),
                        vec![TextEdit { range: lsp_range, new_text }],
                    ))
                    .collect(),
                ),
                document_changes: None,
                change_annotations: None,
            }),
            ..Default::default()
        });

        Task::ready(Ok(actions))
    }

    fn perform_code_action(
        &self,
        state: Entity<InputState>,
        action: lsp_types::CodeAction,
        _push_to_history: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<()>> {
        let _ = (state, action, window, cx);
        Task::ready(Ok(()))
    }
}

/// A reusable SQL editor component built on top of `Input`.
pub struct SqlEditor {
    editor: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl SqlEditor {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| {
            let mut editor = InputState::new(window, cx)
                .code_editor(Language::from_str("sql"))
                .line_number(true)
                .searchable(true)
                .indent_guides(true)
                .tab_size(TabSize { tab_size: 2, hard_tabs: false })
                .soft_wrap(false)
                .placeholder("Enter your SQL query here...");


            // Defaults: completion + hover + actions
            let default_schema = SqlSchema::default()
                .with_tables([
                    ("users", "User accounts"),
                    ("orders", "Orders"),
                    ("products", "Products"),
                    ("customers", "Customers"),
                ])
                .with_columns([
                    ("id", "Identifier"),
                    ("name", "Name"),
                    ("email", "Email"),
                    ("created_at", "Created time"),
                    ("status", "Status"),
                ]);

            editor.lsp.completion_provider =
                Some(Rc::new(DefaultSqlCompletionProvider::new(default_schema)));
            editor.lsp.hover_provider = Some(Rc::new(DefaultSqlHoverProvider));

            editor
        });

        let _subscriptions = vec![cx.subscribe_in(
            &editor,
            window,
            move |_, _, _: &InputEvent, _window, cx| cx.notify(),
        )];

        // Provide default text utilities as code actions (format/minify/uppercase)
        editor.update(cx, |state, _| {
            state.lsp.code_action_providers.push(Rc::new(SqlActionsProvider::new()));
        });

        Self { editor, _subscriptions }
    }

    /// Access underlying editor state.
    pub fn input(&self) -> Entity<InputState> {
        self.editor.clone()
    }

    /// Replace default completion provider.
    pub fn set_completion_provider(
        &mut self,
        provider: Rc<dyn CompletionProvider>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editor
            .update(cx, |state, _| state.lsp.completion_provider = Some(provider));
    }

    /// Set schema for default completion provider.
    pub fn set_schema(
        &mut self,
        schema: SqlSchema,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editor.update(cx, |state, _| {
            state.lsp.completion_provider = Some(Rc::new(DefaultSqlCompletionProvider::new(
                schema,
            )));
        });
    }

    /// Replace hover provider.
    pub fn set_hover_provider(
        &mut self,
        provider: Rc<dyn HoverProvider>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editor
            .update(cx, |state, _| state.lsp.hover_provider = Some(provider));
    }

    /// Add a custom code action provider.
    pub fn add_code_action_provider(
        &mut self,
        provider: Rc<dyn CodeActionProvider>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editor
            .update(cx, |state, _| state.lsp.code_action_providers.push(provider));
    }

    /// Convenient toggles for consumers
    pub fn set_line_number(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.editor
            .update(cx, |s, cx| s.set_line_number(on, window, cx));
    }
    pub fn set_soft_wrap(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |s, cx| s.set_soft_wrap(on, window, cx));
    }
    pub fn set_indent_guides(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.editor
            .update(cx, |s, cx| s.set_indent_guides(on, window, cx));
    }
    pub fn set_value(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |s, cx| s.set_value(text, window, cx));
    }

    /// Get the current text content of the editor.
    /// This is a convenience method that accesses the underlying InputState.
    pub fn get_text<T>(&self, cx: &Context<T>) -> String {
        self.editor.read(cx.deref()).text().to_string()
    }

    /// Get the current text content using App context.
    pub fn get_text_from_app(&self, cx: &App) -> String {
        use std::ops::Deref;
        self.editor.read(cx.deref()).text().to_string()
    }
}

impl Render for SqlEditor {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Input::new(&self.editor).size_full()
    }
}
