use std::any::Any;
use std::marker::PhantomData;

use gpui::{
    div, AnyElement, App, AppContext, ClickEvent, Entity, FocusHandle, Focusable,
    IntoElement, ParentElement, Pixels, SharedString, Styled, Subscription, Window, px,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    resizable::{resizable_panel, v_resizable},
    table::{Column, Table, TableState},
    v_flex, ActiveTheme as _, IconName, Sizable as _, Size,
};

use crate::filter_editor::{ColumnSchema, TableFilterEditor, TableSchema};
use crate::multi_text_editor::{create_multi_text_editor_with_content, MultiTextEditor};
use crate::results_delegate::{EditorTableDelegate};
use db::{GlobalDbState, TableDataRequest};
use gpui_component::table::TableEvent;
use one_core::tab_container::{TabContent, TabContentType};
// ============================================================================
// Table Data Tab Content - Display table rows
// ============================================================================

pub struct TableDataTabContent {
    database_name: String,
    table_name: String,
    connection_id: String,
    table: Entity<TableState<EditorTableDelegate>>,
    status_msg: Entity<String>,
    focus_handle: FocusHandle,
    delegate: EditorTableDelegate,
    /// Text editor for large text editing (MultiTextEditor)
    text_editor: Entity<MultiTextEditor>,
    /// Currently editing cell position
    editing_large_text: Entity<Option<(usize, usize)>>,
    /// Current page (1-based)
    current_page: Entity<usize>,
    /// Page size
    page_size: usize,
    /// Total row count
    total_count: Entity<usize>,
    /// Filter editor with WHERE and ORDER BY inputs
    filter_editor: Entity<TableFilterEditor>,
    /// Editor visibility state
    editor_visible: Entity<bool>,
    /// Subscription to table events (stored but not used directly)
    _table_subscription: Option<Subscription>,
    /// Marker to make the struct Send + Sync
    _phantom: PhantomData<*const ()>,
}

impl TableDataTabContent {
    pub fn new(
        database_name: impl Into<String>,
        table_name: impl Into<String>,
        connection_id: impl Into<String>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let database_name = database_name.into();
        let table_name = table_name.into();
        let connection_id = connection_id.into();
        let delegate = EditorTableDelegate::new(vec![], vec![]);
        let table = cx.new(|cx| TableState::new(delegate.clone(), window, cx));
        let status_msg = cx.new(|_| "Loading...".to_string());
        let focus_handle = cx.focus_handle();
        let editing_large_text = cx.new(|_| None);
        let current_page = cx.new(|_| 1usize);
        let total_count = cx.new(|_| 0usize);

        // Create filter editor with empty schema initially
        let filter_editor = cx.new(|cx| TableFilterEditor::new(window, cx));


        // Editor visibility state (default hidden)
        let editor_visible = cx.new(|_| false);

        // Create multi text editor for cell editing
        let text_editor = create_multi_text_editor_with_content(None, window, cx);

        // Subscribe to table events - capture necessary entities for the closure
        let editing_large_text_clone = editing_large_text.clone();
        let editor_visible_clone = editor_visible.clone();

        let table_subscription = cx.subscribe(&table, move |_this, evt: &TableEvent, cx| {
            match evt {
                TableEvent::SelectCell(row_ix, col_ix) => {
                    println!("Selecting cell: {},{}", row_ix, col_ix);

                    let is_visible = *editor_visible_clone.read(cx);
                    if is_visible {
                        // Update editing position - user needs to click "Load to Editor" to sync content
                        editing_large_text_clone.update(cx, |pos, cx| {
                            *pos = Some((*row_ix, *col_ix));
                            cx.notify();
                        });
                    }
                },
                _ => {}
            }
        });




        let result = Self {
            database_name: database_name.clone(),
            table_name: table_name.clone(),
            connection_id,
            table: table.clone(),
            status_msg: status_msg.clone(),
            focus_handle,
            delegate,
            text_editor,
            editing_large_text,
            current_page,
            page_size: 100,
            total_count,
            filter_editor,
            editor_visible,
            _table_subscription: Some(table_subscription),
            _phantom: PhantomData,
        };

        // Load data initially
        result.load_data_with_clauses(1, cx);

        result
    }

    fn update_status(status_msg: &Entity<String>, message: String, cx: &mut App) {
        status_msg.update(cx, |s, cx| {
            *s = message;
            cx.notify();
        });
    }

    fn load_data_with_clauses(&self, page: usize, cx: &mut App) {
        let global_state = cx.global::<GlobalDbState>().clone();
        let connection_id = self.connection_id.clone();
        let table_name = self.table_name.clone();
        let database_name = self.database_name.clone();
        let status_msg = self.status_msg.clone();
        let table_state = self.table.clone();
        let current_page = self.current_page.clone();
        let total_count = self.total_count.clone();
        let page_size = self.page_size;
        let where_clause = self.filter_editor.read(cx).get_where_clause(cx);
        let order_by_clause = self.filter_editor.read(cx).get_order_by_clause(cx);
        let filter_editor = self.filter_editor.clone();

        cx.spawn(async move |cx| {
            let (plugin, conn_arc) = match global_state.get_plugin_and_connection(&connection_id).await {
                Ok(result) => result,
                Err(e) => {
                    cx.update(|cx| {
                        Self::update_status(&status_msg, format!("Failed to get connection: {}", e), cx);
                    }).ok();
                    return;
                }
            };

            let conn = conn_arc.read().await;

            // Build request with raw where/order by clauses
            let request = TableDataRequest::new(&database_name, &table_name)
                .with_page(page, page_size)
                .with_where_clause(where_clause)
                .with_order_by_clause(order_by_clause);

            match plugin.query_table_data(&**conn, &request).await {
                Ok(response) => {
                    let columns: Vec<Column> = response
                        .columns
                        .iter()
                        .map(|col| Column::new(col.name.clone(), col.name.clone()))
                        .collect();

                    let rows: Vec<Vec<String>> = response
                        .rows
                        .iter()
                        .map(|row| {
                            row.iter()
                                .map(|cell| cell.as_ref().map(|s| s.to_string()).unwrap_or_else(|| "NULL".to_string()))
                                .collect()
                        })
                        .collect();

                    let row_count = rows.len();
                    let total = response.total_count;
                    let total_pages = (total + page_size - 1) / page_size;
                    let pk_columns = response.primary_key_indices;

                    // Build column schema for completion providers
                    let column_schemas: Vec<ColumnSchema> = response
                        .columns
                        .iter()
                        .map(|col| ColumnSchema {
                            name: col.name.clone(),
                            data_type: col.db_type.clone(),
                            is_nullable: col.nullable,
                        })
                        .collect();

                    cx.update(|cx| {
                        // Update filter editor schema
                        filter_editor.update(cx, |editor, cx| {
                            editor.set_schema(TableSchema {
                                table_name: table_name.clone(),
                                columns: column_schemas,
                            }, cx);
                        });

                        table_state.update(cx, |state, cx| {
                            state.delegate_mut().update_data(columns, rows);
                            state.delegate_mut().set_primary_keys(pk_columns);
                            state.refresh(cx);
                        });

                        current_page.update(cx, |p, cx| {
                            *p = page;
                            cx.notify();
                        });

                        total_count.update(cx, |t, cx| {
                            *t = total;
                            cx.notify();
                        });

                        Self::update_status(
                            &status_msg,
                            format!("Page {}/{} ({} rows, {} total)", page, total_pages.max(1), row_count, total),
                            cx,
                        );
                    })
                    .ok();
                }
                Err(e) => {
                    cx.update(|cx| {
                        Self::update_status(&status_msg, format!("Query failed: {}", e), cx);
                    }).ok();
                }
            }
        })
        .detach();
    }

    fn handle_refresh(&self, _: &ClickEvent, _: &mut Window, cx: &mut App) {
        let page = *self.current_page.read(cx);
        self.load_data_with_clauses(page, cx);
    }

    fn handle_prev_page(&self, cx: &mut App) {
        let page = *self.current_page.read(cx);
        if page > 1 {
            self.load_data_with_clauses(page - 1, cx);
        }
    }

    fn handle_next_page(&self, cx: &mut App) {
        let page = *self.current_page.read(cx);
        let total = *self.total_count.read(cx);
        let total_pages = (total + self.page_size - 1) / self.page_size;
        if page < total_pages {
            self.load_data_with_clauses(page + 1, cx);
        }
    }

    fn handle_apply_query(&self, cx: &mut App) {
        self.load_data_with_clauses(1, cx);
    }

    fn handle_save_changes(&self, cx: &mut App) {
        let (changes, column_names, pk_columns) = {
            let delegate = self.table.read(cx).delegate();
            (
                delegate.get_changes(),
                delegate.column_names(),
                delegate.primary_key_columns().to_vec(),
            )
        };

        if changes.is_empty() {
            Self::update_status(&self.status_msg, "No changes to save".to_string(), cx);
            return;
        }

        let changes_count = changes.len();
        Self::update_status(
            &self.status_msg,
            format!("Saving {} changes...", changes_count),
            cx,
        );

        let global_state = cx.global::<GlobalDbState>().clone();
        let connection_id = self.connection_id.clone();
        let table_name = self.table_name.clone();
        let database_name = self.database_name.clone();
        let status_msg = self.status_msg.clone();
        let table_state = self.table.clone();

        cx.spawn(async move |cx| {
            let (plugin, conn_arc) = match global_state.get_plugin_and_connection(&connection_id).await {
                Ok(result) => result,
                Err(e) => {
                    cx.update(|cx| {
                        Self::update_status(&status_msg, format!("Failed to get connection: {}", e), cx);
                    }).ok();
                    return;
                }
            };

            let conn = conn_arc.read().await;
            let mut success_count = 0;
            let mut error_messages = Vec::new();

            for change in changes {
                let sql = Self::generate_sql(&change, &database_name, &table_name, &column_names, &pk_columns);
                if sql.is_empty() {
                    continue;
                }

                match plugin.execute_query(&**conn, &database_name, &sql, None).await {
                    Ok(db::SqlResult::Exec(result)) => {
                        success_count += 1;
                        let _ = result.rows_affected;
                    }
                    Ok(db::SqlResult::Error(err)) => {
                        error_messages.push(err.message);
                    }
                    Err(e) => {
                        error_messages.push(e.to_string());
                    }
                    _ => {}
                }
            }

            cx.update(|cx| {
                if error_messages.is_empty() {
                    table_state.update(cx, |state, cx| {
                        state.delegate_mut().clear_changes();
                        cx.notify();
                    });
                    Self::update_status(
                        &status_msg,
                        format!("Successfully saved {} changes", success_count),
                        cx,
                    );
                } else {
                    Self::update_status(
                        &status_msg,
                        format!(
                            "Saved {} changes, {} errors: {}",
                            success_count,
                            error_messages.len(),
                            error_messages.first().unwrap_or(&String::new())
                        ),
                        cx,
                    );
                }
            }).ok();
        })
        .detach();
    }

    fn generate_sql(
        change: &crate::results_delegate::RowChange,
        database_name: &str,
        table_name: &str,
        column_names: &[String],
        pk_columns: &[usize],
    ) -> String {
        use crate::results_delegate::RowChange;

        match change {
            RowChange::Added { data } => {
                let columns = column_names.join("`, `");
                let values: Vec<String> = data
                    .iter()
                    .map(|v| {
                        if v == "NULL" || v.is_empty() {
                            "NULL".to_string()
                        } else {
                            format!("'{}'", v.replace('\'', "''"))
                        }
                    })
                    .collect();
                format!(
                    "INSERT INTO `{}`.`{}` (`{}`) VALUES ({})",
                    database_name,
                    table_name,
                    columns,
                    values.join(", ")
                )
            }
            RowChange::Updated { original_data, changes } => {
                if changes.is_empty() {
                    return String::new();
                }

                let set_clause: Vec<String> = changes
                    .iter()
                    .map(|c| {
                        let value = if c.new_value == "NULL" {
                            "NULL".to_string()
                        } else {
                            format!("'{}'", c.new_value.replace('\'', "''"))
                        };
                        format!("`{}` = {}", c.col_name, value)
                    })
                    .collect();

                let where_clause = Self::build_where_clause(original_data, column_names, pk_columns);

                format!(
                    "UPDATE `{}`.`{}` SET {} WHERE {}",
                    database_name,
                    table_name,
                    set_clause.join(", "),
                    where_clause
                )
            }
            RowChange::Deleted { original_data } => {
                let where_clause = Self::build_where_clause(original_data, column_names, pk_columns);
                format!(
                    "DELETE FROM `{}`.`{}` WHERE {}",
                    database_name, table_name, where_clause
                )
            }
        }
    }

    fn build_where_clause(original_data: &[String], column_names: &[String], pk_columns: &[usize]) -> String {
        // If we have primary keys, only use those columns
        let indices: Vec<usize> = if pk_columns.is_empty() {
            (0..column_names.len()).collect()
        } else {
            pk_columns.to_vec()
        };

        indices
            .iter()
            .filter_map(|&i| {
                let col_name = column_names.get(i)?;
                let value = original_data.get(i)?;
                Some((col_name, value))
            })
            .map(|(col_name, value)| {
                if value == "NULL" {
                    format!("`{}` IS NULL", col_name)
                } else {
                    format!("`{}` = '{}'", col_name, value.replace('\'', "''"))
                }
            })
            .collect::<Vec<_>>()
            .join(" AND ")
    }

    fn load_cell_to_editor(&self, window: &mut Window, cx: &mut App) {
        let table = self.table.read(cx);
        let selected_row = table.selected_cell();
        let editing_cell = table.editing_cell();

        // Get the cell to load - prefer editing cell, then selected cell
        let (row_ix, col_ix) = if let Some((r, c)) = editing_cell {
            (r, c)
        } else if let Some((r, c)) = selected_row {
            (r, c)
        } else {
            Self::update_status(&self.status_msg, "Please select a cell first".to_string(), cx);
            return;
        };

        // Get current cell value
        let value = table
            .delegate()
            .rows
            .get(row_ix)
            .and_then(|r| r.get(col_ix - 1))
            .cloned()
            .unwrap_or_default();

        // Set the value in MultiTextEditor
        self.text_editor.update(cx, |editor, cx| {
            editor.set_active_text(value, window, cx);
        });

        // Store the editing position
        self.editing_large_text.update(cx, |pos, cx| {
            *pos = Some((row_ix, col_ix));
            cx.notify();
        });
    }

    fn toggle_editor(&self, window: &mut Window, cx: &mut App) {
        let is_visible = *self.editor_visible.read(cx);
        
        if is_visible {
            // Hide editor
            self.editor_visible.update(cx, |visible, cx| {
                *visible = false;
                cx.notify();
            });
            
            // Clear editing position
            self.editing_large_text.update(cx, |pos, cx| {
                *pos = None;
                cx.notify();
            });
        } else {
            // Show editor and load cell
            self.load_cell_to_editor(window, cx);

            // Show editor
            self.editor_visible.update(cx, |visible, cx| {
                *visible = true;
                cx.notify();
            });
        }
    }
    


}

impl TabContent for TableDataTabContent {
    fn title(&self) -> SharedString {
        format!("{}.{} - Data", self.database_name, self.table_name).into()
    }

    fn icon(&self) -> Option<IconName> {
        Some(IconName::Folder)
    }

    fn closeable(&self) -> bool {
        true
    }

    fn render_content(&self, _window: &mut Window, cx: &mut App) -> AnyElement {
        let status_msg_render = self.status_msg.clone();

        v_flex()
            .size_full()
            .gap_2()
            .pt_2()
            .child(
                // Top Toolbar - All buttons
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        Button::new("refresh-data")
                            .with_size(Size::Small)
                            .label("Refresh")
                            .icon(IconName::ArrowDown)
                            .on_click({
                                let this = self.clone();
                                move |e, w, cx| this.handle_refresh(e, w, cx)
                            }),
                    )
                    .child(
                        Button::new("add-row")
                            .with_size(Size::Small)
                            .label("Add Row")
                            .icon(IconName::Plus)
                            .on_click({
                                let table = self.table.clone();
                                move |_, w, cx| {
                                    table.update(cx, |state, cx| {
                                        state.add_row(w, cx);
                                    });
                                }
                            }),
                    )
                    .child(
                        Button::new("delete-row")
                            .with_size(Size::Small)
                            .label("Delete Row")
                            .icon(IconName::Delete)
                            .on_click({
                                let table = self.table.clone();
                                move |_, w, cx| {
                                    table.update(cx, |state, cx| {
                                        if let Some(row_ix) = state.selected_row() {
                                            state.delete_row(row_ix, w, cx);
                                        }
                                    });
                                }
                            }),
                    )
                    .child(
                        Button::new("save-changes")
                            .with_size(Size::Small)
                            .label("Save Changes")
                            .icon(IconName::Check)
                            .on_click({
                                let this = self.clone();
                                move |_, _, cx| {
                                    this.handle_save_changes(cx);
                                }
                            }),
                    )
                    .child({
                        let is_editor_visible = *self.editor_visible.read(cx);
                        let mut btn = Button::new("load-to-editor")
                            .with_size(Size::Small)
                            .label("Load to Editor")
                            .icon(IconName::ArrowDown);
                        
                        if is_editor_visible {
                            btn = btn.primary();
                        }
                        
                        btn.on_click({
                            let this = self.clone();
                            move |_, w, cx| {
                                this.toggle_editor(w, cx);
                            }
                        })
                    })

                    // Pagination controls
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child(
                                Button::new("prev-page")
                                    .with_size(Size::Small)
                                    .icon(IconName::ChevronLeft)
                                    .on_click({
                                        let this = self.clone();
                                        move |_, _, cx| this.handle_prev_page(cx)
                                    }),
                            )
                            .child(
                                Button::new("next-page")
                                    .with_size(Size::Small)
                                    .icon(IconName::ChevronRight)
                                    .on_click({
                                        let this = self.clone();
                                        move |_, _, cx| this.handle_next_page(cx)
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .justify_end()
                            .items_center()
                            .px_2()
                            .text_color(cx.theme().muted_foreground)
                            .text_sm()
                            .child(status_msg_render.read(cx).clone()),
                    ),
            )
            .child(
                // Query bar
                h_flex()
                    .gap_2()
                    .items_center()
                    .w_full()
                    .child(self.filter_editor.clone())
                    .child(
                        Button::new("apply-query")
                            .with_size(Size::Small)
                            .primary()
                            .label("Apply")
                            .icon(IconName::Check)
                            .on_click({
                                let this = self.clone();
                                move |_, _, cx| this.handle_apply_query(cx)
                            }),
                    ),
            )
            .child({
                let is_editor_visible = *self.editor_visible.read(cx);
                
                if is_editor_visible {
                    // Resizable split: Table (top) and Editor (bottom)
                    div()
                        .flex_1()
                        .w_full()
                        .child(
                            v_resizable("table-editor-split")
                                .child(
                                    resizable_panel()
                                        .size(px(400.))
                                        .size_range(px(200.)..Pixels::MAX)
                                        .child(
                                            div()
                                                .size_full()
                                                .bg(cx.theme().background)
                                                .border_1()
                                                .border_color(cx.theme().border)
                                                .rounded_md()
                                                .overflow_hidden()
                                                .child(
                                                    Table::new(&self.table)
                                                        .stripe(true)
                                                        .bordered(false)
                                                ),
                                        ),
                                )
                                .child(
                                    resizable_panel()
                                        .size(px(200.))
                                        .size_range(px(100.)..Pixels::MAX)
                                        .child(
                                            div()
                                                .size_full()
                                                .bg(cx.theme().background)
                                                .border_1()
                                                .border_color(cx.theme().border)
                                                .rounded_md()
                                                .overflow_hidden()
                                                .child(self.text_editor.clone()),
                                        ),
                                ),
                        )
                } else {
                    // Only show table
                    div()
                        .flex_1()
                        .w_full()
                        .bg(cx.theme().background)
                        .border_1()
                        .border_color(cx.theme().border)
                        .rounded_md()
                        .overflow_hidden()
                        .child(
                            Table::new(&self.table)
                                .stripe(true)
                                .bordered(false)
                        )
                }
            })
            .into_any_element()
    }

    fn content_type(&self) -> TabContentType {
        TabContentType::TableData(format!("{}.{}", self.database_name, self.table_name))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}



impl Clone for TableDataTabContent {
    fn clone(&self) -> Self {
        Self {
            database_name: self.database_name.clone(),
            table_name: self.table_name.clone(),
            connection_id: self.connection_id.clone(),
            table: self.table.clone(),
            status_msg: self.status_msg.clone(),
            focus_handle: self.focus_handle.clone(),
            delegate: self.delegate.clone(),
            text_editor: self.text_editor.clone(),
            editing_large_text: self.editing_large_text.clone(),
            current_page: self.current_page.clone(),
            page_size: self.page_size,
            total_count: self.total_count.clone(),
            filter_editor: self.filter_editor.clone(),
            editor_visible: self.editor_visible.clone(),
            _table_subscription: None,
            _phantom: PhantomData,
        }
    }
}

// SAFETY: TableDataTabContent is safe to send across threads because:
// - All Entity<T> types are Send + Sync
// - The Subscription is only used for cleanup and doesn't affect thread safety
// - PhantomData is used to opt-out of auto Send/Sync
unsafe impl Send for TableDataTabContent {}
unsafe impl Sync for TableDataTabContent {}

impl Focusable for TableDataTabContent {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}


