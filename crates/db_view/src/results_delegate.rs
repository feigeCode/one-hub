use std::collections::{HashMap, HashSet};

use db::{FieldType, TableColumnMeta};
use gpui::{div, App, Context, IntoElement, ParentElement, Styled, Window};
use gpui_component::{
    h_flex,
    table::{Column, TableDelegate, TableState}
    ,
};

/// Represents a single cell change with old and new values
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellChange {
    pub col_ix: usize,
    pub col_name: String,
    pub old_value: String,
    pub new_value: String,
}

/// Represents the status of a row
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowStatus {
    /// Original data, unchanged
    Original,
    /// Newly added row
    New,
    /// Modified row
    Modified,
    /// Marked for deletion
    Deleted,
}

/// Represents a change to a row with detailed tracking
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowChange {
    /// A new row was added
    Added {
        /// Data for the new row
        data: Vec<String>,
    },
    /// An existing row was updated
    Updated {
        /// Original row data (for generating WHERE clause)
        original_data: Vec<String>,
        /// Changed cells only
        changes: Vec<CellChange>,
    },
    /// A row was marked for deletion
    Deleted {
        /// Original data (for generating WHERE clause)
        original_data: Vec<String>,
    },
}



pub struct EditorTableDelegate {
    pub columns: Vec<Column>,
    /// Column metadata with type information
    pub column_meta: Vec<TableColumnMeta>,
    pub rows: Vec<Vec<String>>,
    /// Original data snapshot for change detection
    original_rows: Vec<Vec<String>>,
    /// Track row status: key is current row index
    row_status: HashMap<usize, RowStatus>,
    /// Track modified cells (row_ix, col_ix) -> (old_value, new_value)
    cell_changes: HashMap<(usize, usize), (String, String)>,
    /// Track modified cells for UI highlighting
    pub modified_cells: HashSet<(usize, usize)>,
    /// Rows marked for deletion (original row indices)
    deleted_original_rows: HashSet<usize>,
    /// Mapping from current row index to original row index (for tracking)
    row_index_map: HashMap<usize, usize>,
    /// Next row index for new rows (negative conceptually, but we use high numbers)
    next_new_row_id: usize,
    /// New rows data: key is the new_row_id
    new_rows: HashMap<usize, Vec<String>>,
    /// Primary key column indices
    primary_key_columns: Vec<usize>,
}

impl Clone for EditorTableDelegate {
    fn clone(&self) -> Self {
        Self {
            columns: self.columns.clone(),
            column_meta: self.column_meta.clone(),
            rows: self.rows.clone(),
            original_rows: self.original_rows.clone(),
            row_status: self.row_status.clone(),
            cell_changes: self.cell_changes.clone(),
            modified_cells: self.modified_cells.clone(),
            deleted_original_rows: self.deleted_original_rows.clone(),
            row_index_map: self.row_index_map.clone(),
            next_new_row_id: self.next_new_row_id,
            new_rows: self.new_rows.clone(),
            primary_key_columns: self.primary_key_columns.clone(),
        }
    }
}

impl EditorTableDelegate {
    pub fn new(columns: Vec<Column>, rows: Vec<Vec<String>>) -> Self {
        let row_count = rows.len();
        let row_index_map: HashMap<usize, usize> = (0..row_count).map(|i| (i, i)).collect();

        Self {
            columns,
            column_meta: Vec::new(),
            original_rows: rows.clone(),
            rows,
            row_status: HashMap::new(),
            cell_changes: HashMap::new(),
            modified_cells: HashSet::new(),
            deleted_original_rows: HashSet::new(),
            row_index_map,
            next_new_row_id: 1_000_000,
            new_rows: HashMap::new(),
            primary_key_columns: Vec::new(),
        }
    }

    /// Set column metadata
    pub fn set_column_meta(&mut self, meta: Vec<TableColumnMeta>) {
        self.column_meta = meta;
    }

    /// Get column metadata
    pub fn column_meta(&self) -> &[TableColumnMeta] {
        &self.column_meta
    }

    /// Get field type for a column
    pub fn get_field_type(&self, col_ix: usize) -> FieldType {
        self.column_meta
            .get(col_ix)
            .map(|m| m.field_type)
            .unwrap_or(FieldType::Unknown)
    }

    /// Set primary key column indices
    pub fn set_primary_keys(&mut self, pk_columns: Vec<usize>) {
        self.primary_key_columns = pk_columns;
    }

    /// Get primary key column indices
    pub fn primary_key_columns(&self) -> &[usize] {
        &self.primary_key_columns
    }

    pub fn update_data(&mut self, columns: Vec<Column>, rows: Vec<Vec<String>>) {
        // Calculate column widths based on content
        let mut col_widths: Vec<usize> = columns.iter().map(|c| c.name.len()).collect();

        for row in &rows {
            for (col_ix, cell) in row.iter().enumerate() {
                if col_ix < col_widths.len() {
                    col_widths[col_ix] = col_widths[col_ix].max(cell.len());
                }
            }
        }

        // Set column widths and make sortable (min 60px, max 300px, ~8px per char)
        self.columns = columns
            .into_iter()
            .enumerate()
            .map(|(ix, mut col)| {
                let char_width = col_widths.get(ix).copied().unwrap_or(10);
                // Add extra width for filter/sort icons
                let width = ((char_width * 8) + 60).max(80).min(300);
                col.width = gpui::px(width as f32);
                // Make column sortable
                col = col.sortable();
                col
            })
            .collect();

        let row_count = rows.len();
        self.original_rows = rows.clone();
        self.rows = rows;
        self.row_index_map = (0..row_count).map(|i| (i, i)).collect();

        // Clear all change tracking
        self.clear_changes();
    }

    /// Get all pending changes for saving to database
    pub fn get_changes(&self) -> Vec<RowChange> {
        let mut changes = Vec::new();

        // Collect deleted rows
        for &original_ix in &self.deleted_original_rows {
            if let Some(original_data) = self.original_rows.get(original_ix) {
                changes.push(RowChange::Deleted {
                    original_data: original_data.clone(),
                });
            }
        }

        // Collect modified rows
        let mut modified_rows: HashMap<usize, Vec<CellChange>> = HashMap::new();
        for (&(row_ix, col_ix), (old_val, new_val)) in &self.cell_changes {
            // Skip if this row is deleted
            if let Some(&original_ix) = self.row_index_map.get(&row_ix) {
                if self.deleted_original_rows.contains(&original_ix) {
                    continue;
                }
            }

            let col_name = self
                .columns
                .get(col_ix)
                .map(|c| c.name.to_string())
                .unwrap_or_default();

            modified_rows
                .entry(row_ix)
                .or_default()
                .push(CellChange {
                    col_ix,
                    col_name,
                    old_value: old_val.clone(),
                    new_value: new_val.clone(),
                });
        }

        for (row_ix, cell_changes) in modified_rows {
            if let Some(&original_ix) = self.row_index_map.get(&row_ix) {
                if let Some(original_data) = self.original_rows.get(original_ix) {
                    changes.push(RowChange::Updated {
                        original_data: original_data.clone(),
                        changes: cell_changes,
                    });
                }
            }
        }

        // Collect new rows
        for (_, data) in &self.new_rows {
            changes.push(RowChange::Added { data: data.clone() });
        }

        changes
    }

    /// Clear all pending changes
    pub fn clear_changes(&mut self) {
        self.row_status.clear();
        self.cell_changes.clear();
        self.modified_cells.clear();
        self.deleted_original_rows.clear();
        self.new_rows.clear();
    }

    /// Check if there are any pending changes
    pub fn has_changes(&self) -> bool {
        !self.cell_changes.is_empty()
            || !self.deleted_original_rows.is_empty()
            || !self.new_rows.is_empty()
    }

    /// Get the count of pending changes
    pub fn changes_count(&self) -> usize {
        let modified_rows: HashSet<usize> = self.cell_changes.keys().map(|(r, _)| *r).collect();
        modified_rows.len() + self.deleted_original_rows.len() + self.new_rows.len()
    }

    /// Get column names
    pub fn column_names(&self) -> Vec<String> {
        self.columns.iter().map(|c| c.name.to_string()).collect()
    }

    /// Check if a row is newly added
    pub fn is_new_row(&self, row_ix: usize) -> bool {
        self.row_status.get(&row_ix) == Some(&RowStatus::New)
    }

    /// Check if a row is marked for deletion
    pub fn is_deleted_row(&self, row_ix: usize) -> bool {
        self.row_status.get(&row_ix) == Some(&RowStatus::Deleted)
    }
}

impl TableDelegate for EditorTableDelegate {
    fn row_number_enabled(&self, _cx: &App) -> bool {
        true
    }
    fn columns_count(&self, _cx: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _cx: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _cx: &App) -> &Column {
        &self.columns[col_ix]
    }

    fn render_th(&self, col_ix: usize, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let col_name = self
            .columns
            .get(col_ix)
            .map(|c| c.name.clone())
            .unwrap_or_default();


        h_flex()
            .size_full()
            .items_center()
            .justify_between()
            .gap_1()
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(col_name),
            )
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

    fn is_cell_editable(&self, row_ix: usize, _col_ix: usize, _cx: &App) -> bool {
        // Don't allow editing deleted rows
        !self.is_deleted_row(row_ix)
    }

    fn get_cell_value(&self, row_ix: usize, col_ix: usize, _cx: &App) -> String {
        self.rows
            .get(row_ix)
            .and_then(|r| r.get(col_ix))
            .cloned()
            .unwrap_or_default()
    }

    fn on_cell_edited(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        new_value: String,
        _window: &mut Window,
        _cx: &mut Context<TableState<Self>>,
    ) -> bool {
        // Update the cell value
        if let Some(row) = self.rows.get_mut(row_ix) {
            if let Some(cell) = row.get_mut(col_ix) {
                // Only mark as modified if value actually changed
                if *cell == new_value {
                    return false;
                }

                let old_value = cell.clone();
                *cell = new_value.clone();

                // Mark cell as modified for UI
                self.modified_cells.insert((row_ix, col_ix));

                // Track the change with old and new values
                // If this is a new row, we don't need to track cell changes
                if self.is_new_row(row_ix) {
                    // Just update the new_rows data
                    if let Some(new_row_id) = self.find_new_row_id(row_ix) {
                        if let Some(new_row_data) = self.new_rows.get_mut(&new_row_id) {
                            if let Some(cell) = new_row_data.get_mut(col_ix) {
                                *cell = new_value;
                            }
                        }
                    }
                } else {
                    // For existing rows, track the cell change
                    // If we already have a change for this cell, keep the original old_value
                    self.cell_changes
                        .entry((row_ix, col_ix))
                        .and_modify(|(_, new)| *new = new_value.clone())
                        .or_insert((old_value, new_value));

                    // Update row status
                    self.row_status.insert(row_ix, RowStatus::Modified);
                }

                return true;
            }
        }
        false
    }

    fn is_cell_modified(&self, row_ix: usize, col_ix: usize, _cx: &App) -> bool {
        self.modified_cells.contains(&(row_ix, col_ix))
    }

    fn on_row_added(&mut self, _window: &mut Window, cx: &mut Context<TableState<Self>>) {
        // Add a new empty row
        let new_row = vec!["".to_string(); self.columns.len()];
        let row_ix = self.rows.len();
        self.rows.push(new_row.clone());

        // Track as new row
        let new_row_id = self.next_new_row_id;
        self.next_new_row_id += 1;
        self.new_rows.insert(new_row_id, new_row);
        self.row_status.insert(row_ix, RowStatus::New);

        // Map the new row index to the new_row_id (using high number as marker)
        self.row_index_map.insert(row_ix, new_row_id);

        cx.notify();
    }

    fn on_row_deleted(
        &mut self,
        row_ix: usize,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) {
        if row_ix >= self.rows.len() {
            return;
        }

        // Check if this is a new row (not yet saved to DB)
        if self.is_new_row(row_ix) {
            // Just remove it completely
            if let Some(new_row_id) = self.find_new_row_id(row_ix) {
                self.new_rows.remove(&new_row_id);
            }
            self.rows.remove(row_ix);
            self.row_status.remove(&row_ix);
            self.row_index_map.remove(&row_ix);

            // Re-index rows after deletion
            self.reindex_after_deletion(row_ix);
        } else {
            // Mark existing row for deletion
            if let Some(&original_ix) = self.row_index_map.get(&row_ix) {
                self.deleted_original_rows.insert(original_ix);
            }
            self.row_status.insert(row_ix, RowStatus::Deleted);

            // Remove from display
            self.rows.remove(row_ix);

            // Re-index rows after deletion
            self.reindex_after_deletion(row_ix);
        }

        // Clean up cell changes for deleted row
        self.cell_changes.retain(|&(r, _), _| r != row_ix);
        self.modified_cells.retain(|&(r, _)| r != row_ix);

        cx.notify();
    }
}

impl EditorTableDelegate {
    /// Find the new_row_id for a given row index
    fn find_new_row_id(&self, row_ix: usize) -> Option<usize> {
        self.row_index_map.get(&row_ix).copied().filter(|&id| id >= 1_000_000)
    }

    /// Re-index rows after a deletion
    fn reindex_after_deletion(&mut self, deleted_ix: usize) {
        // Update row_index_map: shift all indices after deleted_ix
        let mut new_map = HashMap::new();
        for (&row_ix, &original_ix) in &self.row_index_map {
            if row_ix > deleted_ix {
                new_map.insert(row_ix - 1, original_ix);
            } else if row_ix < deleted_ix {
                new_map.insert(row_ix, original_ix);
            }
            // Skip the deleted row
        }
        self.row_index_map = new_map;

        // Update row_status
        let mut new_status = HashMap::new();
        for (&row_ix, &status) in &self.row_status {
            if row_ix > deleted_ix {
                new_status.insert(row_ix - 1, status);
            } else if row_ix < deleted_ix {
                new_status.insert(row_ix, status);
            }
        }
        self.row_status = new_status;

        // Update cell_changes
        let mut new_changes = HashMap::new();
        for (&(row_ix, col_ix), change) in &self.cell_changes {
            if row_ix > deleted_ix {
                new_changes.insert((row_ix - 1, col_ix), change.clone());
            } else if row_ix < deleted_ix {
                new_changes.insert((row_ix, col_ix), change.clone());
            }
        }
        self.cell_changes = new_changes;

        // Update modified_cells
        let mut new_modified = HashSet::new();
        for &(row_ix, col_ix) in &self.modified_cells {
            if row_ix > deleted_ix {
                new_modified.insert((row_ix - 1, col_ix));
            } else if row_ix < deleted_ix {
                new_modified.insert((row_ix, col_ix));
            }
        }
        self.modified_cells = new_modified;
    }
}


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
    fn row_number_enabled(&self, _cx: &App) -> bool {
        true
    }
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