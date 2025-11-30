use crate::sql_editor::SqlEditor;
use crate::sql_result_tab::SqlResultTabContainer;
use one_core::tab_container::{TabContent, TabContentType};
use db::{ExecOptions, GlobalDbState};
use gpui::{div, px, AnyElement, App, AppContext, ClickEvent, Entity, FocusHandle, Focusable, IntoElement, ParentElement, SharedString, Styled, Window};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::resizable::{resizable_panel, v_resizable};
use gpui_component::select::{SearchableVec, Select, SelectState};
use gpui_component::{h_flex, v_flex, ActiveTheme, IconName, Sizable, Size};
use std::any::Any;
use std::sync::{Arc, RwLock};

pub struct SqlEditorTabContent {
    title: SharedString,
    editor: Entity<SqlEditor>,
    connection_id: String,
    // Multiple result tabs
    sql_result_tab_container: Entity<SqlResultTabContainer> ,
    status_msg: Entity<String>,
    current_database: Arc<RwLock<Option<String>>>,
    database_select: Entity<SelectState<SearchableVec<String>>>,
    // Add focus handle
    focus_handle: FocusHandle,
}

impl SqlEditorTabContent {
    pub fn new(
        title: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        // Create with empty connection_id - should not be used in practice
        Self::new_with_config(title, "", None, window, cx)
    }

    pub fn new_with_config(
        title: impl Into<SharedString>,
        connection_id: impl Into<String>,
        initial_database: Option<String>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let editor = cx.new(|cx| SqlEditor::new(window, cx));
        let focus_handle = cx.focus_handle();

        let result_tabs = Arc::new(RwLock::new(Vec::new()));
        let active_result_tab = Arc::new(RwLock::new(0));

        let status_msg = cx.new(|_| "Ready to execute query".to_string());

        let current_database = Arc::new(RwLock::new(initial_database.clone()));

        // Create database select with empty items initially
        let database_select = cx.new(|cx| {
            SelectState::new(SearchableVec::new(vec![]), None, window, cx)
        });

        let instance = Self {
            title: title.into(),
            editor: editor.clone(),
            connection_id: connection_id.into(),
            sql_result_tab_container: cx.new(|cx| SqlResultTabContainer::new(result_tabs, active_result_tab,cx)),
            status_msg,
            current_database: current_database.clone(),
            database_select: database_select.clone(),
            focus_handle,
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

                let instance = instance_clone.clone();
                let db = db_name.clone();

                cx.spawn(async move |cx| {
                    // Update editor schema
                    cx.update(|cx| {
                        instance.update_schema_for_db(&db, cx);
                    }).ok();
                }).detach();
            }
        }).detach();

        // If initial database is provided, load schema
        if let Some(db) = initial_database {
            let instance_for_schema = instance.clone();
            let db_clone = db.clone();

            cx.spawn(async move |cx| {
                // Update editor schema
                cx.update(|cx| {
                    instance_for_schema.update_schema_for_db(&db_clone, cx);
                }).ok();
            }).detach();
        }

        // Load databases in background
        instance.load_databases_async(cx);

        instance
    }

    pub fn set_sql(&self, sql: String, window: &mut Window, cx: &mut App) {
        self.editor.update(cx, |e, cx| e.set_value(sql, window, cx));
    }

    /// Load databases into the select dropdown
    fn load_databases_async(&self, cx: &mut App) {
        let global_state = cx.global::<GlobalDbState>().clone();
        let connection_id = self.connection_id.clone();
        let current_database = self.current_database.clone();
        let database_select = self.database_select.clone();

        // Spawn async task to load databases
        cx.spawn(async move |cx| {
            let (plugin, conn_arc) = match global_state.get_plugin_and_connection(&connection_id).await {
                Ok(result) => result,
                Err(e) => {
                    eprintln!("Failed to get connection: {}", e);
                    return;
                }
            };

            // List databases
            let conn = conn_arc.read().await;
            let databases = match plugin.list_databases(&**conn).await {
                Ok(dbs) => dbs,
                Err(e) => {
                    eprintln!("Failed to list databases: {}", e);
                    return;
                }
            };

            // Get current database
            let current_db = current_database.read().unwrap().clone();

            eprintln!("Loaded {} databases from server", databases.len());
            eprintln!("Current database: {:?}", current_db);

            // Update UI with loaded databases
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

    /// Update SQL editor schema with tables and columns from current database
    pub fn update_schema_for_db(&self, database: &str, cx: &mut App) {
        use crate::sql_editor::SqlSchema;

        let global_state = cx.global::<GlobalDbState>().clone();
        let connection_id = self.connection_id.clone();
        let editor = self.editor.clone();
        let db = database.to_string();

        cx.spawn(async move |cx| {
            let (plugin, conn_arc) = match global_state.get_plugin_and_connection(&connection_id).await {
                Ok(result) => result,
                Err(e) => {
                    eprintln!("Failed to get connection: {}", e);
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
                .map(|t| {
                    let description = if let Some(comment) = &t.comment {
                        format!("Table: {} - {}", t.name, comment)
                    } else {
                        format!("Table: {}", t.name)
                    };
                    (t.name.clone(), description)
                })
                .collect();
            schema = schema.with_tables(table_items);

            // Load columns for each table
            for table in &tables {
                if let Ok(columns) = plugin.list_columns(&**conn, &db, &table.name).await {
                    let column_items: Vec<(String, String)> = columns.iter()
                        .map(|c| (c.name.clone(), format!("{} - {}",
c.data_type,
                            c.comment.as_ref().unwrap_or(&String::new()))))
                        .collect();
                    schema = schema.with_table_columns(&table.name, column_items);
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
        let status_msg = self.status_msg.clone();
        let global_state = cx.global::<GlobalDbState>().clone();
        let connection_id = self.connection_id.clone();
        let current_database = self.current_database.clone();
        let sql_result_tab_container = self.sql_result_tab_container.clone();
        
        cx.spawn(async move |cx| {
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
            
            // Get connection with current database
            let config = match global_state.get_config(&connection_id).await {
                Some(mut cfg) => {
                    cfg.database = current_database.read().ok().and_then(|guard| guard.clone());
                    cfg
                }
                None => {
                    cx.update(|cx| {
                        status_msg.update(cx, |msg, cx| {
                            *msg = "Connection not found".to_string();
                            cx.notify();
                        });
                    }).ok();
                    return;
                }
            };
            
            let conn_arc = match global_state.connection_pool.get_connection(config, &global_state.db_manager).await {
                Ok(c) => c,
                Err(e) => {
                    cx.update(|cx| {
                        status_msg.update(cx, |msg, cx| {
                            *msg = format!("Failed to get connection: {}", e);
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

            let results_len = results.len();
            let sql_clone = sql.clone();

            // Update result tabs
            let _ = cx.update(|cx| {
                if let Some(window_id) = cx.active_window() {
                    let _ = cx.update_window(window_id, |_entity, window, cx| {
                        sql_result_tab_container.update(cx, |state, cx| {
                            state.set_result(&sql_clone, results, window, cx);
                        });
                    });
                }
            });

            // Update status
            cx.update(|cx| {
                status_msg.update(cx, |msg, cx| {
                    *msg = format!("Executed {} statement(s)", results_len);
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

    fn render_content(&self, _window: &mut Window, cx: &mut App) -> AnyElement {
        let status_msg_render = self.status_msg.clone();
        let editor = self.editor.clone();
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
                    .child(self.sql_result_tab_container.clone())
            )
            .into_any_element())
            .into_any_element()
    }
}


// Make it Clone so we can use it in closures
impl Clone for SqlEditorTabContent {
    fn clone(&self) -> Self {
        Self {
            title: self.title.clone(),
            editor: self.editor.clone(),
            connection_id: self.connection_id.clone(),
            sql_result_tab_container: self.sql_result_tab_container.clone(),
            status_msg: self.status_msg.clone(),
            current_database: self.current_database.clone(),
            database_select: self.database_select.clone(),
            focus_handle: self.focus_handle.clone(),
        }
    }
}

impl Focusable for SqlEditorTabContent {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
