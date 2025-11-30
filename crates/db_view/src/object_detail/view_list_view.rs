use gpui::{
    div, px, App, AppContext, IntoElement, ParentElement, Styled, Window,
};
use gpui_component::{
    v_flex, table::{Column, Table, TableDelegate, TableState}, ActiveTheme, StyledExt,
};
use db::types::ViewInfo;

/// Delegate for displaying view metadata
pub struct ViewListDelegate {
    views: Vec<ViewInfo>,
    columns: Vec<Column>,
}

impl ViewListDelegate {
    pub fn new(views: Vec<ViewInfo>) -> Self {
        let columns = vec![
            Column::new("name", "Name").width(px(250.0)),
            Column::new("comment", "Comment").width(px(400.0)),
        ];

        Self { views, columns }
    }

    pub fn update_views(&mut self, views: Vec<ViewInfo>) {
        self.views = views;
    }
}

impl TableDelegate for ViewListDelegate {
    fn columns_count(&self, _cx: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _cx: &App) -> usize {
        self.views.len()
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
        let view = &self.views[row_ix];
        let column = &self.columns[col_ix];

        let content: String = match column.key.as_ref() {
            "name" => view.name.clone(),
            "comment" => view.comment.as_deref().unwrap_or("").to_string(),
            _ => "".to_string(),
        };

        let mut el = div();
        if column.key.as_ref() == "comment" {
            el = el.text_color(cx.theme().muted_foreground);
        }
        el.child(content)
    }
}

/// View for displaying a list of views with their metadata
pub struct ViewListView;

impl ViewListView {
    pub fn new(views: Vec<ViewInfo>, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let delegate = ViewListDelegate::new(views.clone());
        let view_count = views.len();
        let state = cx.new(|cx| TableState::new(delegate, window, cx));

        v_flex()
            .size_full()
            .gap_2()
            .child(
                div()
                    .p_2()
                    .text_sm()
                    .font_semibold()
                    .child(format!("{} view(s)", view_count)),
            )
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(Table::new(&state).stripe(true).bordered(true))
            )
    }
}
