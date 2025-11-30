use std::any::Any;

use gpui::{
    div, AnyElement, App, AppContext, ClickEvent, Entity, FocusHandle, Focusable,
    IntoElement, ParentElement, SharedString, Styled, Window,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    table::{Column, Table, TableState},
    v_flex, ActiveTheme as _, IconName, Sizable as _, Size,
};

use crate::results_delegate::ResultsDelegate;
use one_core::tab_container::{TabContent, TabContentType};
use db::GlobalDbState;
// ============================================================================
// Table Data Tab Content - Display table rows
// ============================================================================

pub struct TableDataTabContent {
    database_name: String,
    table_name: String,
    connection_id: String,
    table: Entity<TableState<ResultsDelegate>>,
    status_msg: Entity<String>,
    focus_handle: FocusHandle,
    delegate: ResultsDelegate,
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
        let delegate = ResultsDelegate::new(vec![], vec![]);
        let table = cx.new(|cx| TableState::new(delegate.clone(), window, cx));
        let status_msg = cx.new(|_| "Loading...".to_string());
        let focus_handle = cx.focus_handle();

        let result = Self {
            database_name: database_name.clone(),
            table_name: table_name.clone(),
            connection_id,
            table: table.clone(),
            status_msg: status_msg.clone(),
            focus_handle,
            delegate,
        };

        // Load data initially
        result.load_data(cx);

        result
    }

    fn update_status(status_msg: &Entity<String>, message: String, cx: &mut App) {
        status_msg.update(cx, |s, cx| {
            *s = message;
            cx.notify();
        });
    }

    fn load_data(&self, cx: &mut App) {
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
                        table_state.update(cx, |state, cx| {
                            state.delegate_mut().update_data(columns, rows);
                            state.refresh(cx);
                        });

                        Self::update_status(&status_msg, format!("Loaded {} rows", row_count), cx);
                    })
                    .ok();
                }
                Ok(db::SqlResult::Error(err)) => {
                    cx.update(|cx| {
                        Self::update_status(&status_msg, format!("Query error: {}", err.message), cx);
                    }).ok();
                }
                Ok(_) => {
                    cx.update(|cx| {
                        Self::update_status(&status_msg, "Unexpected result type".to_string(), cx);
                    }).ok();
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
                div()
                    .flex_1()
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
            connection_id: self.connection_id.clone(),
            table: self.table.clone(),
            status_msg: self.status_msg.clone(),
            focus_handle: self.focus_handle.clone(),
            delegate: self.delegate.clone(),
        }
    }
}

impl Focusable for TableDataTabContent {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}


