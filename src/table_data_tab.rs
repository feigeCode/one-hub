use std::any::Any;
use std::sync::Arc;

use gpui::{
    div, AnyElement, App, AppContext, ClickEvent, Context, Entity, EventEmitter, Focusable, FocusHandle,
    IntoElement, ParentElement, Render, SharedString, Styled, WeakEntity, Window,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    table::{Column, Table, TableDelegate, TableState},
    v_flex, ActiveTheme as _, IconName, Sizable as _, Size,
};

use db::{DbConnectionConfig, GlobalDbState};
use crate::tab_container::{TabContent, TabContentType};

// ============================================================================
// Table Data Tab Content - Display table rows
// ============================================================================

pub struct TableDataTabContent {
    database_name: String,
    table_name: String,
    config: DbConnectionConfig,
    delegate: Arc<std::sync::RwLock<ResultsDelegate>>,
    table: Entity<TableState<DelegateWrapper>>,
    status_msg: Entity<String>,
    focus_handle: FocusHandle,
}

impl TableDataTabContent {
    pub fn new(
        database_name: impl Into<String>,
        table_name: impl Into<String>,
        config: DbConnectionConfig,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let database_name = database_name.into();
        let table_name = table_name.into();
        let delegate = Arc::new(std::sync::RwLock::new(ResultsDelegate {
            columns: vec![],
            rows: vec![],
        }));

        let delegate_wrapper = DelegateWrapper {
            inner: delegate.clone(),
        };
        let table = cx.new(|cx| TableState::new(delegate_wrapper, window, cx));
        let status_msg = cx.new(|_| "Loading...".to_string());
        let focus_handle = cx.focus_handle();

        let result = Self {
            database_name: database_name.clone(),
            table_name: table_name.clone(),
            config: config.clone(),
            delegate: delegate.clone(),
            table: table.clone(),
            status_msg: status_msg.clone(),
            focus_handle,
        };

        // Load data initially
        result.load_data(cx);

        result
    }

    fn load_data(&self, cx: &mut App) {
        let global_state = cx.global::<GlobalDbState>().clone();
        let config = self.config.clone();
        let table_name = self.table_name.clone();
        let database_name = self.database_name.clone();
        let delegate = self.delegate.clone();
        let status_msg = self.status_msg.clone();
        let table_state = self.table.clone();

        cx.spawn(async move |cx| {
            let plugin = match global_state.db_manager.get_plugin(&config.database_type) {
                Ok(p) => p,
                Err(e) => {
                    cx.update(|cx| {
                        status_msg.update(cx, |s, cx| {
                            *s = format!("Failed to get plugin: {}", e);
                            cx.notify();
                        });
                    })
                    .ok();
                    return;
                }
            };

            let conn_arc = match global_state
                .connection_pool
                .get_connection(config.clone(), &global_state.db_manager)
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    cx.update(|cx| {
                        status_msg.update(cx, |s, cx| {
                            *s = format!("Connection failed: {}", e);
                            cx.notify();
                        });
                    })
                    .ok();
                    return;
                }
            };

            let conn = conn_arc.read().await;

            // Query table data with LIMIT
            let query = format!("SELECT * FROM `{}`.`{}` LIMIT 1000", database_name, table_name);
            let result = plugin.execute_query(&**conn, &database_name, &query, None).await;

            match result {
                Ok(db::SqlResult::Query(query_result)) => {
                    let columns: Vec<Column> = query_result
                        .columns
                        .iter()
                        .map(|col| Column::new(col.clone(), col.clone()))
                        .collect();

                    let rows: Vec<Vec<String>> = query_result
                        .rows
                        .iter()
                        .map(|row| {
                            row.iter()
                                .map(|cell| cell.as_ref().map(|s| s.to_string()).unwrap_or_else(|| "NULL".to_string()))
                                .collect()
                        })
                        .collect();

                    let row_count = rows.len();

                    cx.update(|cx| {
                        delegate.write().unwrap().columns = columns;
                        delegate.write().unwrap().rows = rows;

                        status_msg.update(cx, |s, cx| {
                            *s = format!("Loaded {} rows", row_count);
                            cx.notify();
                        });

                        table_state.update(cx, |_state, cx| {
                            cx.notify();
                        });
                    })
                    .ok();
                }
                Ok(db::SqlResult::Error(err)) => {
                    cx.update(|cx| {
                        status_msg.update(cx, |s, cx| {
                            *s = format!("Query error: {}", err.message);
                            cx.notify();
                        });
                    })
                    .ok();
                }
                Ok(_) => {
                    cx.update(|cx| {
                        status_msg.update(cx, |s, cx| {
                            *s = "Unexpected result type".to_string();
                            cx.notify();
                        });
                    })
                    .ok();
                }
                Err(e) => {
                    cx.update(|cx| {
                        status_msg.update(cx, |s, cx| {
                            *s = format!("Query failed: {}", e);
                            cx.notify();
                        });
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn handle_refresh(&self, _: &ClickEvent, _: &mut Window, cx: &mut App) {
        self.load_data(cx);
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
            .child(
                // Toolbar
                h_flex()
                    .gap_2()
                    .p_2()
                    .bg(cx.theme().muted)
                    .rounded_md()
                    .items_center()
                    .w_full()
                    .child(
                        Button::new("refresh-data")
                            .with_size(Size::Small)
                            .primary()
                            .label("Refresh")
                            .icon(IconName::ArrowDown)
                            .on_click({
                                let this = self.clone();
                                move |e, w, cx| this.handle_refresh(e, w, cx)
                            }),
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
                // Table
                v_flex()
                    .flex_1()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .overflow_hidden()
                    .child(Table::new(&self.table)),
            )
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
            config: self.config.clone(),
            delegate: self.delegate.clone(),
            table: self.table.clone(),
            status_msg: self.status_msg.clone(),
            focus_handle: self.focus_handle.clone(),
        }
    }
}

impl Render for TableDataTabContent {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.render_content(window, cx))
    }
}

impl Focusable for TableDataTabContent {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// ============================================================================
// Helper Types
// ============================================================================

pub struct ResultsDelegate {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<String>>,
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

#[derive(Clone)]
pub struct DelegateWrapper {
    pub inner: Arc<std::sync::RwLock<ResultsDelegate>>,
}

impl TableDelegate for DelegateWrapper {
    fn columns_count(&self, _cx: &App) -> usize {
        self.inner.read().unwrap().columns.len()
    }
    fn rows_count(&self, _cx: &App) -> usize {
        self.inner.read().unwrap().rows.len()
    }
    fn column(&self, col_ix: usize, _cx: &App) -> &Column {
        unsafe { &*(&self.inner.read().unwrap().columns[col_ix] as *const Column) }
    }
    fn render_td(
        &self,
        row: usize,
        col: usize,
        _window: &mut Window,
        _cx: &mut App,
    ) -> impl IntoElement {
        self.inner
            .read()
            .unwrap()
            .rows
            .get(row)
            .and_then(|r| r.get(col))
            .cloned()
            .unwrap_or_default()
    }
}
