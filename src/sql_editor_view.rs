use std::sync::{Arc, RwLock};
use std::any::Any;
use gpui::{div, px, AnyElement, App, AppContext, ClickEvent, Entity, IntoElement, ParentElement, SharedString, Styled, Window, Focusable, FocusHandle, EventEmitter, Render, Context};
use gpui_component::{h_flex, v_flex, ActiveTheme, IconName, Sizable, Size};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::table::{Column, Table, TableState};
use gpui_component::select::{SelectState, Select, SearchableVec};
use gpui_component::tab::{Tab, TabBar};
use gpui_component::resizable::{v_resizable, resizable_panel};
use gpui_component::list::ListItem;
use gpui_component::StyledExt;
use gpui_component::dock::{Panel, PanelEvent, PanelState};
use db::{GlobalDbState, ExecOptions, SqlResult};
use crate::sql_editor::SqlEditor;
use crate::tab_container::{TabContent, TabContentType};
use crate::tab_contents::{DelegateWrapper};

// Structure to hold a single SQL result with its metadata
#[derive(Clone)]
pub struct SqlResultTab {
    pub sql: String,
    pub result: SqlResult,
    pub execution_time: String,
    pub rows_count: String,
    pub table: Entity<TableState<DelegateWrapper>>,
}

pub struct SqlEditorTabContent {
    title: SharedString,
    editor: Entity<SqlEditor>,
    // Multiple result tabs
    result_tabs: Arc<RwLock<Vec<SqlResultTab>>>,
    active_result_tab: Arc<RwLock<usize>>,
    status_msg: Entity<String>,
    current_database: Arc<RwLock<Option<String>>>,
    database_select: Entity<SelectState<SearchableVec<String>>>,
    // Add fields for tracking database loading
    databases_loading: Arc<std::sync::atomic::AtomicBool>,
    databases_cache: Arc<RwLock<Vec<String>>>,
    databases_loaded: Arc<std::sync::atomic::AtomicBool>,
}

impl SqlEditorTabContent {
    pub fn new(
        title: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        Self::new_with_database(title, None, window, cx)
    }

    pub fn new_with_database(
        title: impl Into<SharedString>,
        initial_database: Option<String>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let editor = cx.new(|cx| SqlEditor::new(window, cx));

        let result_tabs = Arc::new(RwLock::new(Vec::new()));
        let active_result_tab = Arc::new(RwLock::new(0));

        let status_msg = cx.new(|_| "Ready to execute query".to_string());

        let current_database = Arc::new(RwLock::new(initial_database.clone()));

        // Create database select with empty items initially
        let database_select = cx.new(|cx| {
            SelectState::new(SearchableVec::new(vec![]), None, window, cx)
        });

        let databases_loading = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let databases_cache = Arc::new(RwLock::new(Vec::new()));
        let databases_loaded = Arc::new(std::sync::atomic::AtomicBool::new(false));

        let instance = Self {
            title: title.into(),
            editor: editor.clone(),
            result_tabs,
            active_result_tab,
            status_msg,
            current_database: current_database.clone(),
            database_select: database_select.clone(),
            databases_loading,
            databases_cache,
            databases_loaded,
        };

        // Subscribe to select events for database switching
        let current_db_clone = current_database.clone();
        let instance_clone = instance.clone();

        cx.subscribe(&database_select, move |_select, event, cx| {
            use gpui_component::select::SelectEvent;
            if let SelectEvent::Confirm(Some(db_name)) = event {
                // Update current database
                if let Ok(mut guard) = current_db_clone.write() {
                    *guard = Some(db_name.clone());
                }

                let global_state = cx.global::<GlobalDbState>().clone();
                let db = db_name.clone();
                let instance = instance_clone.clone();

                cx.spawn(async move |cx| {
                    // Set current database in connection pool
                    global_state.connection_pool
                        .set_current_database(Some(db.clone()))
                        .await;

                    // Update editor schema
                    cx.update(|cx| {
                        instance.update_schema_for_db(&db, cx);
                    }).ok();
                }).detach();
            }
        }).detach();

        // If initial database is provided, set it immediately and load schema
        if let Some(db) = initial_database {
            // Set current database
            if let Ok(mut guard) = instance.current_database.write() {
                *guard = Some(db.clone());
            }

            let global_state = cx.global::<GlobalDbState>().clone();
            let db_clone = db.clone();
            let instance_for_schema = instance.clone();

            cx.spawn(async move |cx| {
                // Wait for connection
                let max_retries = 10;
                let mut retries = 0;
                while retries < max_retries {
                    if global_state.connection_pool.is_connected().await {
                        break;
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    retries += 1;
                }

                if !global_state.connection_pool.is_connected().await {
                    eprintln!("Failed to connect to database for schema loading");
                    return;
                }

                // Set current database in connection pool
                global_state.connection_pool
                    .set_current_database(Some(db_clone.clone()))
                    .await;

                // Update editor schema
                cx.update(|cx| {
                    instance_for_schema.update_schema_for_db(&db_clone, cx);
                }).ok();
            }).detach();
        }

        instance
    }

    pub fn set_sql(&self, sql: String, window: &mut Window, cx: &mut App) {
        self.editor.update(cx, |e, cx| e.set_value(sql, window, cx));
    }

    /// Load databases into the select dropdown
    pub fn load_databases(&self, window: &mut Window, cx: &mut App) {
        // Check if already loading
        if self.databases_loading.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }

        // Check if already loaded
        if self.databases_loaded.load(std::sync::atomic::Ordering::Relaxed) {
            // Already loaded, just update the UI with cached data
            let databases = self.databases_cache.read().unwrap().clone();
            if !databases.is_empty() {
                self.update_dropdown_with_databases(databases, window, cx);
            }
            return;
        }

        // Mark as loading
        self.databases_loading.store(true, std::sync::atomic::Ordering::Relaxed);

        // Set initial loading state
        self.database_select.update(cx, |state, cx| {
            let items = SearchableVec::new(vec!["Loading databases...".to_string()]);
            state.set_items(items, window, cx);
        });

        // Clone what we need for async
        let global_state = cx.global::<GlobalDbState>().clone();
        let current_database = self.current_database.clone();
        let databases_cache = self.databases_cache.clone();
        let databases_loaded = self.databases_loaded.clone();
        let databases_loading = self.databases_loading.clone();
        let database_select = self.database_select.clone();

        // Spawn async task to load databases
        cx.spawn(async move |cx| {
            // Check if connected
            if !global_state.connection_pool.is_connected().await {
                eprintln!("Not connected to database");
                databases_loading.store(false, std::sync::atomic::Ordering::Relaxed);
                return;
            }

            // Get current connection and config
            let conn_arc = match global_state.connection_pool.get_current_connection().await {
                Some(c) => c,
                None => {
                    eprintln!("No current connection");
                    databases_loading.store(false, std::sync::atomic::Ordering::Relaxed);
                    return;
                }
            };

            let config = match global_state.connection_pool.get_current_connection_config().await {
                Some(c) => c,
                None => {
                    eprintln!("No connection config");
                    databases_loading.store(false, std::sync::atomic::Ordering::Relaxed);
                    return;
                }
            };

            // Get plugin
            let plugin = match global_state.db_manager.get_plugin(&config.database_type) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Failed to get plugin: {}", e);
                    databases_loading.store(false, std::sync::atomic::Ordering::Relaxed);
                    return;
                }
            };

            // List databases
            let conn = conn_arc.read().await;
            let databases = match plugin.list_databases(&**conn).await {
                Ok(dbs) => dbs,
                Err(e) => {
                    eprintln!("Failed to list databases: {}", e);
                    databases_loading.store(false, std::sync::atomic::Ordering::Relaxed);
                    return;
                }
            };

            // Get current database
            let current_db = if let Ok(guard) = current_database.read() {
                guard.clone()
            } else {
                global_state.connection_pool.current_database().await
            };

            eprintln!("Loaded {} databases from server", databases.len());
            eprintln!("Current database: {:?}", current_db);

            // Store in cache
            *databases_cache.write().unwrap() = databases.clone();

            // Update current database if we got one
            if let Some(db) = &current_db {
                if let Ok(mut guard) = current_database.write() {
                    *guard = Some(db.clone());
                }
            }

            // Mark as loaded
            databases_loaded.store(true, std::sync::atomic::Ordering::Relaxed);
            databases_loading.store(false, std::sync::atomic::Ordering::Relaxed);

            // Update UI with loaded databases - access window through cx.update_window
            let result = cx.update(|cx| {
                if let Some(window_id) = cx.active_window() {
                    cx.update_window(window_id, |_entity, window, cx| {
                        database_select.update(cx, |state, cx| {
                            if !databases.is_empty() {
                                eprintln!("Updating dropdown with {} databases", databases.len());
                                let items = SearchableVec::new(databases.clone());
                                state.set_items(items, window, cx);

                                // Set current selection if there's a current database
                                if let Some(current_db) = &current_db {
                                    if let Some(index) = databases.iter().position(|d| d == current_db) {
                                        use gpui_component::IndexPath;
                                        state.set_selected_index(Some(IndexPath::new(index)), window, cx);
                                    }
                                }
                            } else {
                                let items = SearchableVec::new(vec!["No databases available".to_string()]);
                                state.set_items(items, window, cx);
                            }
                        });
                    })
                } else {
                    Err(anyhow::anyhow!("No active window"))
                }
            });

            if let Err(e) = result {
                eprintln!("Failed to update dropdown: {:?}", e);
            }
        }).detach();
    }

    /// Helper to update the dropdown with databases
    fn update_dropdown_with_databases(&self, databases: Vec<String>, window: &mut Window, cx: &mut App) {
        self.database_select.update(cx, |state, cx| {
            if !databases.is_empty() {
                eprintln!("Updating dropdown with {} databases", databases.len());
                let items = SearchableVec::new(databases.clone());
                state.set_items(items, window, cx);

                // Set current selection if there's a current database
                if let Ok(guard) = self.current_database.read() {
                    if let Some(current_db) = guard.as_ref() {
                        if let Some(index) = databases.iter().position(|d| d == current_db) {
                            use gpui_component::IndexPath;
                            state.set_selected_index(Some(IndexPath::new(index)), window, cx);
                        }
                    }
                }
            } else {
                let items = SearchableVec::new(vec!["No databases available".to_string()]);
                state.set_items(items, window, cx);
            }
        });
    }

    /// Try to update dropdown if databases are loaded (call from render)
    pub fn try_update_dropdown(&self, window: &mut Window, cx: &mut App) {
        // Check if databases are loaded but not yet displayed
        if self.databases_loaded.load(std::sync::atomic::Ordering::Relaxed) {
            let databases = self.databases_cache.read().unwrap().clone();
            if !databases.is_empty() {
                // Check if dropdown is still showing "Loading..."
                self.database_select.update(cx, |state, cx| {
                    // Get current items to check if we need to update
                    // We'll just update regardless to ensure it's correct
                    let items = SearchableVec::new(databases.clone());
                    state.set_items(items, window, cx);

                    // Set current selection if there's a current database
                    if let Ok(guard) = self.current_database.read() {
                        if let Some(current_db) = guard.as_ref() {
                            if let Some(index) = databases.iter().position(|d| d == current_db) {
                                use gpui_component::IndexPath;
                                state.set_selected_index(Some(IndexPath::new(index)), window, cx);
                            }
                        }
                    }
                });
            }
        }
    }

    /// Manually update databases when we have window access
    pub fn update_databases_now(&self, window: &mut Window, cx: &mut App) {
        let global_state = cx.global::<GlobalDbState>().clone();
        let current_database = self.current_database.clone();

        // Set refreshing placeholder
        self.database_select.update(cx, |state, cx| {
            let items = SearchableVec::new(vec!["Refreshing...".to_string()]);
            state.set_items(items, window, cx);
        });

        // Try to load and update immediately
        cx.spawn(async move |cx| {
            if !global_state.connection_pool.is_connected().await {
                eprintln!("Not connected to database");
                return;
            }

            // Get current connection and config
            let conn_arc = match global_state.connection_pool.get_current_connection().await {
                Some(c) => c,
                None => {
                    eprintln!("No current connection");
                    return;
                }
            };

            let config = match global_state.connection_pool.get_current_connection_config().await {
                Some(c) => c,
                None => {
                    eprintln!("No connection config");
                    return;
                }
            };

            // Get plugin
            let plugin = match global_state.db_manager.get_plugin(&config.database_type) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Failed to get plugin: {}", e);
                    return;
                }
            };

            // List databases
            let conn = conn_arc.read().await;
            if let Ok(databases) = plugin.list_databases(&**conn).await {
                let current_db = if let Ok(guard) = current_database.read() {
                    guard.clone()
                } else {
                    global_state.connection_pool.current_database().await
                };

                eprintln!("update_databases_now: {} databases loaded", databases.len());

                cx.update(|_cx| {
                    eprintln!("Ready to update with {} databases", databases.len());
                }).ok();
            }
        }).detach();
    }

    /// Refresh databases synchronously (call this after load_databases completes)
    pub fn refresh_databases(&self, databases: Vec<String>, current_db: Option<String>, window: &mut Window, cx: &mut App) {
        self.database_select.update(cx, |state, cx| {
            if !databases.is_empty() {
                let items = SearchableVec::new(databases.clone());
                state.set_items(items, window, cx);

                // Set current selection if there's a current database
                if let Some(db) = current_db.as_ref() {
                    if let Some(index) = databases.iter().position(|d| d == db) {
                        use gpui_component::IndexPath;
                        state.set_selected_index(Some(IndexPath::new(index)), window, cx);
                    }
                }
            } else {
                let items = SearchableVec::new(vec!["No databases".to_string()]);
                state.set_items(items, window, cx);
            }
        });

        // Update our current database tracking
        if let Some(db) = current_db {
            if let Ok(mut guard) = self.current_database.write() {
                *guard = Some(db);
            }
        }
    }

    /// Initialize database list after connection
    pub fn init_databases(&mut self, window: &mut Window, cx: &mut App) {
        let global_state = cx.global::<GlobalDbState>().clone();

        // For immediate feedback, set loading state
        self.database_select.update(cx, |state, cx| {
            let items = SearchableVec::new(vec!["Connecting...".to_string()]);
            state.set_items(items, window, cx);
        });

        // Check if we're connected and load databases
        cx.spawn(async move |_cx| {
            if !global_state.connection_pool.is_connected().await {
                eprintln!("Not connected to database");
                return;
            }

            // Get current connection and config
            let conn_arc = match global_state.connection_pool.get_current_connection().await {
                Some(c) => c,
                None => {
                    eprintln!("No current connection");
                    return;
                }
            };

            let config = match global_state.connection_pool.get_current_connection_config().await {
                Some(c) => c,
                None => {
                    eprintln!("No connection config");
                    return;
                }
            };

            // Get plugin
            let plugin = match global_state.db_manager.get_plugin(&config.database_type) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Failed to get plugin: {}", e);
                    return;
                }
            };

            // List databases
            let conn = conn_arc.read().await;
            if let Ok(databases) = plugin.list_databases(&**conn).await {
                eprintln!("Available databases: {:?}", databases);
            }
        }).detach();
    }


    /// Update SQL editor schema with tables and columns from current database
    pub fn update_schema_for_db(&self, database: &str, cx: &mut App) {
        use crate::sql_editor::SqlSchema;

        let global_state = cx.global::<GlobalDbState>().clone();
        let editor = self.editor.clone();
        let db = database.to_string();

        cx.spawn(async move |cx| {
            // Get current connection and config
            let conn_arc = match global_state.connection_pool.get_current_connection().await {
                Some(c) => c,
                None => {
                    eprintln!("No current connection");
                    return;
                }
            };

            let config = match global_state.connection_pool.get_current_connection_config().await {
                Some(c) => c,
                None => {
                    eprintln!("No connection config");
                    return;
                }
            };

            // Get plugin
            let plugin = match global_state.db_manager.get_plugin(&config.database_type) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Failed to get plugin: {}", e);
                    return;
                }
            };

            // Load tables
            let conn = conn_arc.read().await;
            let tables = match plugin.list_tables(&**conn, &db).await {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("Failed to list tables: {}", e);
                    return;
                }
            };

            let mut schema = SqlSchema::default();

            // Add tables to schema
            let table_items: Vec<(String, String)> = tables.iter()
                .map(|t| (t.clone(), format!("Table: {}", t)))
                .collect();
            schema = schema.with_tables(table_items);

            // Load columns for each table
            for table in &tables {
                if let Ok(columns) = plugin.list_columns(&**conn, &db, table).await {
                    let column_items: Vec<(String, String)> = columns.iter()
                        .map(|c| (c.name.clone(), format!("{} - {}", c.data_type,
                            c.comment.as_ref().unwrap_or(&String::new()))))
                        .collect();
                    schema = schema.with_table_columns(table, column_items);
                }
            }

            // Update editor schema
            cx.update(|cx| {
                editor.update(cx, |e, _cx| {
                    e.input().update(_cx, |state, _| {
                        use std::rc::Rc;
                        use crate::sql_editor::DefaultSqlCompletionProvider;
                        state.lsp.completion_provider = Some(Rc::new(DefaultSqlCompletionProvider::new(schema)));
                    });
                });
            }).ok();
        }).detach();
    }

    fn get_sql_text(&self, cx: &App) -> String {
        self.editor.read(cx).get_text_from_app(cx)
    }

    fn handle_run_query(&self, _: &ClickEvent, _window: &mut Window, cx: &mut App) {
        let sql = self.get_sql_text(cx);
        let result_tabs = self.result_tabs.clone();
        let active_result_tab = self.active_result_tab.clone();
        let status_msg = self.status_msg.clone();
        let global_state = cx.global::<GlobalDbState>().clone();

        // Clear existing result tabs
        result_tabs.write().unwrap().clear();
        *active_result_tab.write().unwrap() = 0;

        cx.spawn(async move |cx| {
            // Check connection
            if !global_state.connection_pool.is_connected().await {
                cx.update(|cx| {
                    status_msg.update(cx, |msg, cx| {
                        *msg = "Not connected to database".to_string();
                        cx.notify();
                    });
                }).ok();
                return;
            }

            // Check if SQL is empty
            if sql.trim().is_empty() {
                cx.update(|cx| {
                    status_msg.update(cx, |msg, cx| {
                        *msg = "No SQL statements to execute".to_string();
                        cx.notify();
                    });
                }).ok();
                return;
            }

            // Get current connection
            let conn_arc = match global_state.connection_pool.get_current_connection().await {
                Some(c) => c,
                None => {
                    cx.update(|cx| {
                        status_msg.update(cx, |msg, cx| {
                            *msg = "No active connection".to_string();
                            cx.notify();
                        });
                    }).ok();
                    return;
                }
            };

            // Execute script directly on connection
            let options = ExecOptions::default();
            let conn = conn_arc.read().await;
            let results = match conn.execute(&sql, options).await {
                Ok(r) => r,
                Err(e) => {
                    cx.update(|cx| {
                        status_msg.update(cx, |msg, cx| {
                            *msg = format!("Failed to execute script: {}", e);
                            cx.notify();
                        });
                    }).ok();
                    return;
                }
            };

            // Process results
            if results.is_empty() {
                cx.update(|cx| {
                    status_msg.update(cx, |msg, cx| {
                        *msg = "No results".to_string();
                        cx.notify();
                    });
                }).ok();
                return;
            }

            // Split SQL into individual statements for labeling
            let sql_statements: Vec<String> = sql
                .split(';')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            // Create tabs for each result
            let mut new_tabs = Vec::new();
            let mut total_rows = 0;
            let mut total_time = 0.0;

            for (idx, result) in results.iter().enumerate() {
                let sql_text = sql_statements.get(idx)
                    .map(|s| {
                        if s.len() > 50 {
                            format!("{}...", &s[..50])
                        } else {
                            s.clone()
                        }
                    })
                    .unwrap_or_else(|| format!("Statement {}", idx + 1));

                match result {
                    SqlResult::Query(query_result) => {
                        // Create table for this result
                        let delegate = Arc::new(RwLock::new(crate::tab_contents::ResultsDelegate {
                            columns: query_result.columns.iter()
                                .map(|h| Column::new(h.clone(), h.clone()))
                                .collect(),
                            rows: query_result.rows.iter()
                                .map(|row| {
                                    row.iter()
                                        .map(|cell| cell.clone().unwrap_or_else(|| "NULL".to_string()))
                                        .collect()
                                })
                                .collect(),
                        }));

                        let delegate_wrapper = DelegateWrapper {
                            inner: delegate.clone(),
                        };

                        // Create table entity in UI context
                        let table = cx.update(|cx| {
                            cx.update_window(cx.active_window().unwrap(), |_entity, window, cx| {
                                cx.new(|cx| TableState::new(delegate_wrapper, window, cx))
                            }).unwrap()
                        }).ok().unwrap();

                        total_rows += query_result.rows.len();
                        total_time += query_result.elapsed_ms as f64;

                        new_tabs.push(SqlResultTab {
                            sql: sql_text,
                            result: result.clone(),
                            execution_time: format!("{}ms", query_result.elapsed_ms),
                            rows_count: format!("{} rows", query_result.rows.len()),
                            table,
                        });
                    }
                    SqlResult::Exec(exec_result) => {
                        // Create a summary table for exec results
                        let delegate = Arc::new(RwLock::new(crate::tab_contents::ResultsDelegate {
                            columns: vec![
                                Column::new("Status", "Status"),
                                Column::new("Rows Affected", "Rows Affected"),
                            ],
                            rows: vec![vec![
                                exec_result.message.clone().unwrap_or_else(|| "Success".to_string()),
                                format!("{}", exec_result.rows_affected),
                            ]],
                        }));

                        let delegate_wrapper = DelegateWrapper {
                            inner: delegate.clone(),
                        };

                        let table = cx.update(|cx| {
                            cx.update_window(cx.active_window().unwrap(), |_entity, window, cx| {
                                cx.new(|cx| TableState::new(delegate_wrapper, window, cx))
                            }).unwrap()
                        }).ok().unwrap();

                        total_time += exec_result.elapsed_ms as f64;

                        new_tabs.push(SqlResultTab {
                            sql: sql_text,
                            result: result.clone(),
                            execution_time: format!("{}ms", exec_result.elapsed_ms),
                            rows_count: format!("{} rows affected", exec_result.rows_affected),
                            table,
                        });
                    }
                    SqlResult::Error(error) => {
                        // Create error table
                        let delegate = Arc::new(RwLock::new(crate::tab_contents::ResultsDelegate {
                            columns: vec![Column::new("Error", "Error")],
                            rows: vec![vec![error.message.clone()]],
                        }));

                        let delegate_wrapper = DelegateWrapper {
                            inner: delegate.clone(),
                        };

                        let table = cx.update(|cx| {
                            cx.update_window(cx.active_window().unwrap(), |_entity, window, cx| {
                                cx.new(|cx| TableState::new(delegate_wrapper, window, cx))
                            }).unwrap()
                        }).ok().unwrap();

                        new_tabs.push(SqlResultTab {
                            sql: sql_text,
                            result: result.clone(),
                            execution_time: "Error".to_string(),
                            rows_count: "Error".to_string(),
                            table,
                        });
                    }
                }
            }

            // Update result tabs
            *result_tabs.write().unwrap() = new_tabs;

            // Update status
            cx.update(|cx| {
                status_msg.update(cx, |msg, cx| {
                    *msg = format!(
                        "Executed {} statement(s), {} total rows in {:.2}ms",
                        results.len(),
                        total_rows,
                        total_time
                    );
                    cx.notify();
                });
            }).ok();
        })
            .detach();
    }

    fn handle_format_query(&self, _: &ClickEvent, window: &mut Window, cx: &mut App) {
        let text = self.get_sql_text(cx);
        let formatted = text
            .split('\n')
            .map(|l| l.trim().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        self.editor
            .update(cx, |s, cx| s.set_value(formatted, window, cx));
    }
}



impl TabContent for SqlEditorTabContent {
    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn icon(&self) -> Option<IconName> {
        Some(IconName::File)
    }

    fn closeable(&self) -> bool {
        true
    }

    fn content_type(&self) -> TabContentType {
        TabContentType::SqlEditor
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn render_content(&self, window: &mut Window, cx: &mut App) -> AnyElement {
        // Try to update dropdown if databases have been loaded
        self.try_update_dropdown(window, cx);

        let status_msg_render = self.status_msg.clone();
        let editor = self.editor.clone();
        let result_tabs = self.result_tabs.clone();
        let active_result_tab = self.active_result_tab.clone();
        let database_select = self.database_select.clone();

        // Build the main layout with resizable panels
        // Wrap in v_flex().size_full() to ensure proper containment within tab
        v_flex()
            .size_full()
            .child(v_resizable("sql-editor-resizable")
            .child(
                // Top panel: Toolbar and Editor
                resizable_panel()
                    .size(px(400.))
                    .size_range(px(200.)..px(800.))
                    .child(
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
                                        // Database selector
                                        Select::new(&database_select)
                                            .with_size(Size::Small)
                                            .placeholder("Select Database")
                                            .w(px(200.))
                                    )
                                    .child(
                                        Button::new("run-query")
                                            .with_size(Size::Small)
                                            .primary()
                                            .label("Run (⌘+Enter)")
                                            .icon(IconName::ArrowRight)
                                            .on_click({
                                                let this = self.clone();
                                                move |e, w, cx| this.handle_run_query(e, w, cx)
                                            }),
                                    )
                                    .child(
                                        Button::new("format-query")
                                            .with_size(Size::Small)
                                            .ghost()
                                            .label("Format")
                                            .icon(IconName::Star)
                                            .on_click({
                                                let this = self.clone();
                                                move |e, w, cx| this.handle_format_query(e, w, cx)
                                            }),
                                    )
                                    .child(
                                        Button::new("compress-query")
                                            .with_size(Size::Small)
                                            .ghost()
                                            .label("Compress")
                                            .on_click({
                                                let this = self.clone();
                                                move |_e, w, cx| {
                                                    let text = this.get_sql_text(cx);
                                                    let compressed = text.lines()
                                                        .map(|l| l.trim())
                                                        .filter(|l| !l.is_empty())
                                                        .collect::<Vec<_>>()
                                                        .join(" ");
                                                    this.editor.update(cx, |e, cx| e.set_value(compressed, w, cx));
                                                }
                                            }),
                                    )
                                    .child(
                                        Button::new("export-query")
                                            .with_size(Size::Small)
                                            .ghost()
                                            .label("Export")
                                            .on_click({
                                                move |_, _, _| {
                                                    // TODO: Implement export functionality
                                                }
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
                                // Editor
                                v_flex()
                                    .flex_1()
                                    .child(editor)
                            )
                    )
            )
            .child(
                // Bottom panel: Results with tabs
                resizable_panel()
                    .child({
                        let tabs = result_tabs.read().unwrap();
                        let active_idx = *active_result_tab.read().unwrap();

                        if tabs.is_empty() {
                            // Show empty state
                            v_flex()
                                .size_full()
                                .bg(cx.theme().background)
                                .border_1()
                                .border_color(cx.theme().border)
                                .rounded_md()
                                .items_center()
                                .justify_center()
                                .child(
                                    div()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("Execute a query to see results")
                                )
                        } else {
                            // Show tabs with results
                            v_flex()
                                .size_full()
                                .gap_0()
                                .child(
                                    // Tab bar for result tabs (摘要 + individual results)
                                    TabBar::new("result-tabs")
                                        .w_full()
                                        .with_size(Size::Small)
                                        .selected_index(active_idx)
                                        .on_click({
                                            let active_tab = active_result_tab.clone();
                                            move |_ix: &usize, _w, _cx| {
                                                *active_tab.write().unwrap() = *_ix;
                                            }
                                        })
                                        .child(
                                            // Summary tab
                                            Tab::new().label("摘要")
                                        )
                                        .children(tabs.iter().enumerate().map(|(idx, tab)| {
                                            Tab::new().label(format!("结果{} ({}, {})", idx + 1, tab.rows_count, tab.execution_time))
                                        }))
                                )
                                .child(
                                    // Active tab content
                                    v_flex()
                                        .flex_1()
                                        .bg(cx.theme().background)
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .rounded_md()
                                        .overflow_hidden()
                                        .child(
                                            if active_idx == 0 {
                                                // Show summary view
                                                render_summary_view(&tabs, cx)
                                            } else {
                                                // Show individual result table
                                                tabs.get(active_idx - 1)
                                                    .map(|tab| Table::new(&tab.table.clone()).into_any_element())
                                                    .unwrap_or_else(|| div().into_any_element())
                                            }
                                        )
                                )
                        }
                    })
            )
            .into_any_element())
            .into_any_element()
    }
}

// Render summary view function
fn render_summary_view(tabs: &[SqlResultTab], cx: &App) -> AnyElement {
    let mut total_rows = 0;
    let mut total_time = 0.0;
    let mut success_count = 0;
    let mut error_count = 0;

    for tab in tabs {
        match &tab.result {
            SqlResult::Query(q) => {
                total_rows += q.rows.len();
                total_time += q.elapsed_ms as f64;
                success_count += 1;
            }
            SqlResult::Exec(e) => {
                total_rows += e.rows_affected as usize;
                total_time += e.elapsed_ms as f64;
                success_count += 1;
            }
            SqlResult::Error(_) => {
                error_count += 1;
            }
        }
    }

    v_flex()
        .size_full()
        .p_4()
        .gap_3()
        .child(
            // Summary header
            h_flex()
                .gap_4()
                .items_center()
                .child(
                    div()
                        .text_lg()
                        .font_semibold()
                        .child("执行摘要")
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("共 {} 条语句", tabs.len()))
                )
        )
        .child(
            // Statistics
            h_flex()
                .gap_6()
                .child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("成功")
                        )
                        .child(
                            div()
                                .text_xl()
                                .font_semibold()
                                .text_color(cx.theme().success)
                                .child(format!("{}", success_count))
                        )
                )
                .child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("失败")
                        )
                        .child(
                            div()
                                .text_xl()
                                .font_semibold()
                                .text_color(cx.theme().danger)
                                .child(format!("{}", error_count))
                        )
                )
                .child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("总耗时")
                        )
                        .child(
                            div()
                                .text_xl()
                                .font_semibold()
                                .child(format!("{:.2}ms", total_time))
                        )
                )
                .child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("影响行数")
                        )
                        .child(
                            div()
                                .text_xl()
                                .font_semibold()
                                .child(format!("{}", total_rows))
                        )
                )
        )
        .child(
            // Divider
            div()
                .h(px(1.))
                .w_full()
                .bg(cx.theme().border)
        )
        .child(
            // Statement list
            v_flex()
                .gap_2()
                .flex_1()
                .overflow_y_hidden()
                .children(tabs.iter().enumerate().map(|(idx, tab)| {
                    let (status_icon, status_color, status_text) = match &tab.result {
                        SqlResult::Query(q) => (
                            IconName::Check,
                            cx.theme().success,
                            format!("{} rows", q.rows.len())
                        ),
                        SqlResult::Exec(e) => (
                            IconName::Check,
                            cx.theme().success,
                            format!("{} rows affected", e.rows_affected)
                        ),
                        SqlResult::Error(e) => (
                            IconName::Close,
                            cx.theme().danger,
                            e.message.clone()
                        ),
                    };

                    ListItem::new(idx)
                        .child(
                            h_flex()
                                .gap_3()
                                .items_center()
                                .w_full()
                                .child(
                                    // Status icon
                                    div()
                                        .flex_shrink_0()
                                        .text_color(status_color)
                                        .child(status_icon)
                                )
                                .child(
                                    // SQL preview
                                    div()
                                        .flex_1()
                                        .text_sm()
                                        .truncate()
                                        .child(format!("语句{}: {}", idx + 1, tab.sql))
                                )
                                .child(
                                    // Execution time
                                    div()
                                        .flex_shrink_0()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(tab.execution_time.clone())
                                )
                                .child(
                                    // Status text
                                    div()
                                        .flex_shrink_0()
                                        .text_xs()
                                        .text_color(status_color)
                                        .child(status_text)
                                )
                        )
                }))
        )
        .into_any_element()
}

// Make it Clone so we can use it in closures
impl Clone for SqlEditorTabContent {
    fn clone(&self) -> Self {
        Self {
            title: self.title.clone(),
            editor: self.editor.clone(),
            result_tabs: self.result_tabs.clone(),
            active_result_tab: self.active_result_tab.clone(),
            status_msg: self.status_msg.clone(),
            current_database: self.current_database.clone(),
            database_select: self.database_select.clone(),
            databases_loading: self.databases_loading.clone(),
            databases_cache: self.databases_cache.clone(),
            databases_loaded: self.databases_loaded.clone(),
        }
    }
}
