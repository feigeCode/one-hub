use gpui::{
    div, px, App, AppContext, IntoElement, ParentElement, Styled, Window,
};
use gpui_component::{
    v_flex, table::{Column, Table, TableDelegate, TableState}, ActiveTheme, StyledExt,
};
use db::types::DatabaseInfo;

/// Delegate for displaying database list
pub struct DatabaseListDelegate {
    databases: Vec<DatabaseInfo>,
    columns: Vec<Column>,
}

impl DatabaseListDelegate {
    pub fn new(databases: Vec<DatabaseInfo>) -> Self {
        let columns = vec![
            Column::new("name", "Name").width(px(180.0)),
            Column::new("charset", "Charset").width(px(120.0)),
            Column::new("collation", "Collation").width(px(180.0)),
            Column::new("size", "Size").width(px(100.0)).text_right(),
            Column::new("tables", "Tables").width(px(80.0)).text_right(),
            Column::new("comment", "Comment").width(px(250.0)),
        ];

        Self { databases, columns }
    }
}

impl TableDelegate for DatabaseListDelegate {
    fn columns_count(&self, _cx: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _cx: &App) -> usize {
        self.databases.len()
    }

    fn column(&self, col_ix: usize, _cx: &App) -> &Column {
        &self.columns[col_ix]
    }

    fn render_td(
        &self,
        row_ix: usize,
        col_ix: usize,
        _window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let database = &self.databases[row_ix];
        let column = &self.columns[col_ix];

        let content: String = match column.key.as_ref() {
            "name" => database.name.clone(),
            "charset" => database.charset.as_deref().unwrap_or("-").to_string(),
            "collation" => database.collation.as_deref().unwrap_or("-").to_string(),
            "size" => database.size.as_deref().unwrap_or("-").to_string(),
            "tables" => database.table_count.map(|n| n.to_string()).unwrap_or_else(|| "-".to_string()),
            "comment" => database.comment.as_deref().unwrap_or("").to_string(),
            _ => "".to_string(),
        };

        let mut el = div();
        if column.key.as_ref() == "size" || column.key.as_ref() == "tables" {
            el = el.text_right();
        }
        if column.key.as_ref() == "charset" || column.key.as_ref() == "collation" || column.key.as_ref() == "comment" {
            el = el.text_color(cx.theme().muted_foreground);
        }
        el.child(content)
    }
}

/// View for displaying a list of databases
pub struct DatabaseListView;

impl DatabaseListView {
    pub fn new(databases: Vec<DatabaseInfo>, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let delegate = DatabaseListDelegate::new(databases.clone());
        let database_count = databases.len();
        let state = cx.new(|cx| TableState::new(delegate, window, cx));

        v_flex()
            .size_full()
            .gap_2()
            .child(
                div()
                    .p_2()
                    .text_sm()
                    .font_semibold()
                    .child(format!("{} database(s)", database_count)),
            )
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(Table::new(&state).stripe(true).bordered(true))
            )
    }
}
