use gpui::{
    div, px, App, AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window,
};
use gpui_component::{
    v_flex, table::{Column, Table, TableDelegate, TableState}, ActiveTheme, StyledExt,
};
use db::types::IndexInfo;

/// Delegate for displaying index metadata
pub struct IndexListDelegate {
    table_name: String,
    indexes: Vec<IndexInfo>,
    columns: Vec<Column>,
}

impl IndexListDelegate {
    pub fn new(table_name: String, indexes: Vec<IndexInfo>) -> Self {
        let columns = vec![
            Column::new("name", "Name").width(px(200.0)),
            Column::new("type", "Type").width(px(100.0)),
            Column::new("unique", "Unique").width(px(80.0)),
            Column::new("columns", "Columns").width(px(300.0)),
        ];

        Self {
            table_name,
            indexes,
            columns,
        }
    }

    pub fn update_indexes(&mut self, table_name: String, indexes: Vec<IndexInfo>) {
        self.table_name = table_name;
        self.indexes = indexes;
    }
}

impl TableDelegate for IndexListDelegate {
    fn columns_count(&self, _cx: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _cx: &App) -> usize {
        self.indexes.len()
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
        let index = &self.indexes[row_ix];
        let column = &self.columns[col_ix];

        match column.key.as_ref() {
            "name" => div().child(index.name.clone()),
            "type" => div().child(index.index_type.clone().unwrap_or_else(|| "_".to_string())),
            "unique" => div().child(if index.is_unique { "YES" } else { "NO" }),
            "columns" => div()
                .text_color(cx.theme().muted_foreground)
                .child(index.columns.join(", ")),
            _ => div().child(""),
        }
    }
}

/// View for displaying a list of indexes with their metadata
pub struct IndexListView {
    state: Entity<TableState<IndexListDelegate>>,
}

impl IndexListView {
    pub fn new(table_name: String, indexes: Vec<IndexInfo>, window: &mut Window, cx: &mut App) -> Self {
        let delegate = IndexListDelegate::new(table_name, indexes);
        let state = cx.new(|cx| TableState::new(delegate, window, cx));

        Self { state }
    }

    pub fn update_indexes(&self, table_name: String, indexes: Vec<IndexInfo>, cx: &mut App) {
        self.state.update(cx, |state, cx| {
            state.delegate_mut().update_indexes(table_name, indexes);
            state.refresh(cx);
        });
    }
}

impl Render for IndexListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let delegate = self.state.read(cx).delegate();
        let table_name = delegate.table_name.clone();
        let index_count = delegate.indexes.len();

        v_flex()
            .size_full()
            .p_2()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .child(format!(
                        "Indexes for table: {} ({} index(es))",
                        table_name, index_count
                    )),
            )
            .child(Table::new(&self.state).stripe(true).bordered(true))
    }
}
