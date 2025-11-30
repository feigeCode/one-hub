use gpui::{
    div, px, App, AppContext, IntoElement, ParentElement, Styled, Window,
};
use gpui_component::{
    v_flex, table::{Column, Table, TableDelegate, TableState}, ActiveTheme, Icon, IconName, StyledExt,
};
use db::types::ColumnInfo;

/// Delegate for displaying column metadata
pub struct ColumnListDelegate {
    table_name: String,
    columns: Vec<ColumnInfo>,
    table_columns: Vec<Column>,
}

impl ColumnListDelegate {
    pub fn new(table_name: String, columns: Vec<ColumnInfo>) -> Self {
        let table_columns = vec![
            Column::new("pk", "PK").width(px(50.0)),
            Column::new("name", "Name").width(px(200.0)),
            Column::new("type", "Type").width(px(150.0)),
            Column::new("nullable", "Nullable").width(px(80.0)),
            Column::new("default", "Default").width(px(150.0)),
            Column::new("comment", "Comment").width(px(300.0)),
        ];

        Self {
            table_name,
            columns,
            table_columns,
        }
    }

    pub fn update_columns(&mut self, table_name: String, columns: Vec<ColumnInfo>) {
        self.table_name = table_name;
        self.columns = columns;
    }
}

impl TableDelegate for ColumnListDelegate {
    fn columns_count(&self, _cx: &App) -> usize {
        self.table_columns.len()
    }

    fn rows_count(&self, _cx: &App) -> usize {
        self.columns.len()
    }

    fn column(&self, col_ix: usize, _cx: &App) -> &Column {
        &self.table_columns[col_ix]
    }

    fn render_td(
        &self,
        row_ix: usize,
        col_ix: usize,
        _window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let column = &self.columns[row_ix];
        let table_column = &self.table_columns[col_ix];

        match table_column.key.as_ref() {
            "pk" => {
                if column.is_primary_key {
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(cx.theme().primary)
                        .child(Icon::new(IconName::Key))
                } else {
                    div()
                }
            }
            _ => {
                let content: String = match table_column.key.as_ref() {
                    "name" => column.name.clone(),
                    "type" => column.data_type.clone(),
                    "nullable" => if column.is_nullable { "YES" } else { "NO" }.to_string(),
                    "default" => column.default_value.as_deref().unwrap_or("-").to_string(),
                    "comment" => column.comment.as_deref().unwrap_or("").to_string(),
                    _ => "".to_string(),
                };

                let mut el = div();
                if table_column.key.as_ref() == "name" && column.is_primary_key {
                    el = el.font_semibold().text_color(cx.theme().primary);
                }
                if table_column.key.as_ref() == "comment" {
                    el = el.text_color(cx.theme().muted_foreground);
                }
                el.child(content)
            }
        }
    }
}

/// View for displaying a list of columns with their metadata
pub struct ColumnListView;

impl ColumnListView {
    pub fn new(table_name: String, columns: Vec<ColumnInfo>, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let delegate = ColumnListDelegate::new(table_name.clone(), columns.clone());
        let column_count = columns.len();
        let state = cx.new(|cx| TableState::new(delegate, window, cx));

        v_flex()
            .size_full()
            .gap_2()
            .child(
                div()
                    .p_2()
                    .text_sm()
                    .font_semibold()
                    .child(format!(
                        "Columns for table: {} ({} column(s))",
                        table_name, column_count
                    )),
            )
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(Table::new(&state).stripe(true).bordered(true))
            )
    }
}
