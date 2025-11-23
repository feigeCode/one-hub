use gpui::{
    div, px, App, AppContext, IntoElement, ParentElement, Styled, Window,
};
use gpui_component::{
    v_flex, table::{Column, Table, TableDelegate, TableState}, ActiveTheme, StyledExt,
};
use db::types::FunctionInfo;

/// Delegate for displaying function/procedure metadata
pub struct FunctionListDelegate {
    title: String,
    functions: Vec<FunctionInfo>,
    columns: Vec<Column>,
}

impl FunctionListDelegate {
    pub fn new(title: String, functions: Vec<FunctionInfo>) -> Self {
        let columns = vec![
            Column::new("name", "Name").width(px(250.0)),
            Column::new("return_type", "Return Type").width(px(150.0)),
            Column::new("comment", "Comment").width(px(300.0)),
        ];

        Self {
            title,
            functions,
            columns,
        }
    }

    pub fn update_functions(&mut self, title: String, functions: Vec<FunctionInfo>) {
        self.title = title;
        self.functions = functions;
    }
}

impl TableDelegate for FunctionListDelegate {
    fn columns_count(&self, _cx: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _cx: &App) -> usize {
        self.functions.len()
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
        let func = &self.functions[row_ix];
        let column = &self.columns[col_ix];

        let content: String = match column.key.as_ref() {
            "name" => func.name.clone(),
            "return_type" => func.return_type.as_deref().unwrap_or("-").to_string(),
            "comment" => func.comment.as_deref().unwrap_or("").to_string(),
            _ => "".to_string(),
        };

        let mut el = div();
        if column.key.as_ref() == "comment" {
            el = el.text_color(cx.theme().muted_foreground);
        }
        el.child(content)
    }
}

/// View for displaying a list of functions/procedures with their metadata
pub struct FunctionListView;

impl FunctionListView {
    pub fn new(title: String, functions: Vec<FunctionInfo>, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let delegate = FunctionListDelegate::new(title.clone(), functions.clone());
        let count = functions.len();
        let state = cx.new(|cx| TableState::new(delegate, window, cx));

        v_flex()
            .size_full()
            .p_2()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .child(format!("{} {}", count, title.to_lowercase())),
            )
            .child(Table::new(&state).stripe(true).bordered(true))
    }
}
