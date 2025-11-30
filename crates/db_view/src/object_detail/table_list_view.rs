use gpui::{
    div, px, App, AppContext, IntoElement, ParentElement, Styled, Window,
};
use gpui_component::{
    v_flex, table::{Column, Table, TableDelegate, TableState}, ActiveTheme, StyledExt,
};
use db::types::TableInfo;

/// Delegate for displaying table metadata
pub struct TableListDelegate {
    tables: Vec<TableInfo>,
    columns: Vec<Column>,
}

impl TableListDelegate {
    pub fn new(tables: Vec<TableInfo>) -> Self {
        let columns = vec![
            Column::new("name", "Name").width(px(200.0)),
            Column::new("engine", "Engine").width(px(150.0)),
            Column::new("rows", "Rows").width(px(100.0)).text_right(),
            Column::new("created", "Created").width(px(180.0)),
            Column::new("comment", "Comment").width(px(300.0)),
        ];

        Self { tables, columns }
    }

    pub fn update_tables(&mut self, tables: Vec<TableInfo>) {
        self.tables = tables;
    }
}

impl TableDelegate for TableListDelegate {
    fn columns_count(&self, _cx: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _cx: &App) -> usize {
        self.tables.len()
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
        let table = &self.tables[row_ix];
        let column = &self.columns[col_ix];

        let content: String = match column.key.as_ref() {
            "name" => table.name.clone(),
            "engine" => table.engine.as_deref().unwrap_or("-").to_string(),
            "rows" => table.row_count.map(|n| n.to_string()).unwrap_or_else(|| "-".to_string()),
            "created" => table.create_time.as_deref().unwrap_or("-").to_string(),
            "comment" => table.comment.as_deref().unwrap_or("").to_string(),
            _ => "".to_string(),
        };

        let mut el = div();
        if column.key.as_ref() == "rows" {
            el = el.text_right();
        }
        if column.key.as_ref() == "comment" {
            el = el.text_color(cx.theme().muted_foreground);
        }
        el.child(content)
    }
}

/// View for displaying a list of tables with their metadata
pub struct TableListView;

impl TableListView {
    pub fn new(tables: Vec<TableInfo>, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let delegate = TableListDelegate::new(tables.clone());
        let table_count = tables.len();
        let state = cx.new(|cx| TableState::new(delegate, window, cx));

        v_flex()
            .size_full()
            .gap_2()
            .child(
                div()
                    .p_2()
                    .text_sm()
                    .font_semibold()
                    .child(format!("{} table(s)", table_count)),
            )
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(Table::new(&state).stripe(true).bordered(true))
            )
    }
}
