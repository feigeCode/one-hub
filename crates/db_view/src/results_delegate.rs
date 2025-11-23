use gpui::{App, IntoElement, Window};
use gpui_component::table::{Column, TableDelegate};

pub struct ResultsDelegate {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<String>>,
}

impl Clone for ResultsDelegate {
    fn clone(&self) -> Self {
        Self {
            columns: self.columns.clone(),
            rows: self.rows.clone(),
        }
    }
}

impl ResultsDelegate {
    pub(crate) fn new(columns: Vec<Column>, rows: Vec<Vec<String>>) -> Self {
        Self {
            columns,
            rows,
        }
    }

    pub(crate) fn update_data(&mut self, columns: Vec<Column>, rows: Vec<Vec<String>>) {
        self.columns = columns;
        self.rows = rows;
    }
}

impl TableDelegate for ResultsDelegate {

    fn columns_count(&self, _cx: &App) -> usize {
        self.columns.len()
    }
    fn rows_count(&self, _cx: &App) -> usize {
        self.rows.len()
    }
    fn column(&self, col_ix: usize, _cx: &App) -> &Column {
        &self.columns[col_ix]
    }
    fn render_td(
        &self,
        row: usize,
        col: usize,
        _window: &mut Window,
        _cx: &mut App,
    ) -> impl IntoElement {
        self.rows
            .get(row)
            .and_then(|r| r.get(col))
            .cloned()
            .unwrap_or_default()
    }
}