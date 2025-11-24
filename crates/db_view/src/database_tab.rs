use core::storage::StoredConnection;
use core::tab_container::{TabContainer, TabContent, TabContentType, TabItem};
use std::any::Any;

use crate::database_objects_tab::DatabaseObjectsPanel;
use crate::db_tree_view::DbTreeView;
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, AnyElement, App, AppContext, Context, Entity, FontWeight,
    Hsla, IntoElement, ParentElement, SharedString, Styled, Subscription, Window,
};
use gpui_component::button::ButtonVariants;
use gpui_component::resizable::{h_resizable, resizable_panel};
use gpui_component::{h_flex, v_flex, ActiveTheme, IconName};
use uuid::Uuid;

// Event handler for database tree view events
struct DatabaseEventHandler {
    _tree_subscription: Subscription,
}

impl DatabaseEventHandler {
    fn new(
        db_tree_view: &Entity<DbTreeView>,
        tab_container: Entity<TabContainer>,
        connection_info: StoredConnection,
        objects_panel: Entity<DatabaseObjectsPanel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        use crate::db_tree_view::DbTreeViewEvent;

        let tab_container_clone = tab_container.clone();
        let conn_info_clone = connection_info.clone();
        let objects_panel_clone = objects_panel.clone();
        let tree_view_clone = db_tree_view.clone();
        
        let tree_subscription = cx.subscribe_in(db_tree_view, window, move |_handler, _tree, event, window, cx| {
            match event {
                DbTreeViewEvent::NodeSelected { node_id } => {
                    
                    // 先从 tree 中提取节点信息
                    let node_info = tree_view_clone.update(cx, |tree, _cx| {
                        tree.get_node(node_id).cloned()
                    });
                    
                    // 然后根据节点类型更新 objects panel
                    if let Some(node) = node_info {
                        let config = conn_info_clone.to_db_connection();
                        objects_panel_clone.update(cx, |panel, cx| {
                            panel.handle_node_selected(node_id.clone(), node.node_type, config, cx);
                        });
                    }
                }
                DbTreeViewEvent::CreateNewQuery { database } => {
                    use crate::sql_editor_view::SqlEditorTabContent;

                    // Create new SQL editor with connection config
                    let config = conn_info_clone.to_db_connection();
                    let sql_editor = SqlEditorTabContent::new_with_config(
                        format!("{} - Query", database),
                        config.id,
                        Some(database.clone()),
                        window,
                        cx,
                    );

                    // Add to tab container
                    tab_container_clone.update(cx, |container, cx| {
                        let tab_id = format!("query-{}-{}", database, Uuid::new_v4());
                        let tab = TabItem::new(tab_id, sql_editor);
                        container.add_and_activate_tab(tab, cx);
                    });
                }
                DbTreeViewEvent::OpenTableData { database, table } => {
                    use crate::table_data_tab::TableDataTabContent;

                    let tab_id = format!("table-data-{}.{}", database, table);
                    let database_clone = database.clone();
                    let table_clone = table.clone();
                    let config = conn_info_clone.to_db_connection();

                    // Lazy load: only create tab content if tab doesn't exist
                    tab_container_clone.update(cx, |container, cx| {
                        container.activate_or_add_tab_lazy(
                            tab_id.clone(),
                            |window, cx| {
                                let table_data = TableDataTabContent::new(
                                    database_clone,
                                    table_clone,
                                    config.id,
                                    window,
                                    cx,
                                );
                                TabItem::new(tab_id, table_data)
                            },
                            window,
                            cx,
                        );
                    });
                }
                DbTreeViewEvent::OpenViewData { database, view } => {
                    use crate::table_data_tab::TableDataTabContent;

                    let tab_id = format!("view-data-{}.{}", database, view);
                    let database_clone = database.clone();
                    let view_clone = view.clone();
                    let config = conn_info_clone.to_db_connection();

                    // Lazy load: only create tab content if tab doesn't exist
                    tab_container_clone.update(cx, |container, cx| {
                        container.activate_or_add_tab_lazy(
                            tab_id.clone(),
                            |window, cx| {
                                let view_data = TableDataTabContent::new(
                                    database_clone,
                                    view_clone,
                                    config.id,
                                    window,
                                    cx,
                                );
                                TabItem::new(tab_id, view_data)
                            },
                            window,
                            cx,
                        );
                    });
                }
                DbTreeViewEvent::OpenTableStructure { database, table } => {
                    use crate::table_designer_view::TableDesignerView;

                    let tab_id = format!("table-designer-{}.{}", database, table);
                    let database_clone = database.clone();
                    let table_clone = table.clone();
                    let config = conn_info_clone.to_db_connection();

                    // Lazy load: only create tab content if tab doesn't exist
                    tab_container_clone.update(cx, |container, cx| {
                        container.activate_or_add_tab_lazy(
                            tab_id.clone(),
                            |window, cx| {
                                let table_designer = TableDesignerView::edit_table(
                                    database_clone,
                                    table_clone,
                                    config.id,
                                    config.database_type,
                                    window,
                                    cx,
                                );
                                TabItem::new(tab_id, table_designer.read(cx).clone())
                            },
                            window,
                            cx,
                        );
                    });
                }
                DbTreeViewEvent::ImportData { database, table: _ } => {
                    use crate::data_import_view::DataImportView;
                    use gpui_component::WindowExt;

                    eprintln!("Opening import dialog for database: {}", database);
                    
                    // Create data import view
                    let config = conn_info_clone.to_db_connection();
                    let import_view = DataImportView::new(
                        config.id,
                        database.clone(),
                        window,
                        cx,
                    );
                    
                    eprintln!("Import view created, opening dialog...");
                    
                    // Open as dialog
                    window.open_dialog(cx, move |dialog, _window, _cx| {
                        eprintln!("Dialog builder called");
                        dialog
                            .title("Import Data")
                            .child(import_view.clone())
                            .width(px(800.0))
                            .on_cancel(|_, _window, _cx| true)
                    });
                    
                    eprintln!("Dialog opened");
                }
                DbTreeViewEvent::ExportData { database, tables } => {
                    use crate::data_export_view::DataExportView;
                    use gpui_component::WindowExt;

                    // Create data export view
                    let config = conn_info_clone.to_db_connection();
                    let export_view = DataExportView::new(
                        config.id,
                        database.clone(),
                        window,
                        cx,
                    );

                    // Pre-fill tables if provided
                    if !tables.is_empty() {
                        let tables_str = tables.join(", ");
                        export_view.update(cx, |view, cx| {
                            view.tables.update(cx, |state, cx| {
                                state.set_value(tables_str, window, cx);
                            });
                        });
                    }
                    
                    // Open as dialog
                    window.open_dialog(cx, move |dialog, _window, _cx| {
                        dialog
                            .title("Export Data")
                            .child(export_view.clone())
                            .width(px(800.0))
                            .on_cancel(|_, _window, _cx| true)
                    });
                }
            }
        });

        Self {
            _tree_subscription: tree_subscription,
        }
    }
}

// Database connection tab content - using TabContainer architecture
pub struct DatabaseTabContent {
    connection_info: StoredConnection,
    tab_container: Entity<TabContainer>,
    db_tree_view: Entity<DbTreeView>,
    objects_panel: Entity<DatabaseObjectsPanel>,
    status_msg: Entity<String>,
    is_connected: Entity<bool>,
    event_handler: Option<Entity<DatabaseEventHandler>>,
}

impl DatabaseTabContent {
    pub fn new(stored_conn: StoredConnection, window: &mut Window, cx: &mut App) -> Self {
        // Create database tree view
        let db_tree_view = cx.new(|cx| {
            DbTreeView::new(stored_conn.clone(), window, cx)
        });

        // Create tab container
        let tab_container = cx.new(|cx| {
            TabContainer::new(window, cx)
                .with_tab_bar_colors(
                    Some(gpui::rgb(0xf5f5f5).into()),
                    Some(gpui::rgb(0xe0e0e0).into()),
                )
                .with_tab_item_colors(
                    Some(gpui::rgb(0xffffff).into()),
                    Some(gpui::rgb(0xe8e8e8).into()),
                )
                .with_tab_content_colors(
                    Some(gpui::rgb(0x333333).into()),
                    Some(gpui::rgb(0x666666).into()),
                )
        });

        // Create objects panel
        let objects_panel = cx.new(|cx| {
            DatabaseObjectsPanel::new(window, cx)
        });
        

        // Add objects panel to tab container
        tab_container.update(cx, |container, cx| {
            let panel_content = objects_panel.read(cx).clone();
            let tab = TabItem::new("objects-panel", panel_content);
            container.add_and_activate_tab(tab, cx);
        });

        let status_msg = cx.new(|_| "Connecting...".to_string());
        let is_connected = cx.new(|_| false);

        // Create event handler to handle tree view events
        let event_handler = cx.new(|cx| {
            DatabaseEventHandler::new(&db_tree_view, tab_container.clone(), stored_conn.clone(), objects_panel.clone(), window, cx)
        });

        let instance = Self {
            connection_info: stored_conn.clone(),
            tab_container,
            db_tree_view,
            objects_panel,
            status_msg,
            is_connected,
            event_handler: Some(event_handler),
        };

        // Automatically start connection
        instance.start_connection(stored_conn, cx);

        instance
    }

    fn start_connection(&self, conn: StoredConnection, cx: &mut App) {
        let status_msg = self.status_msg.clone();
        let is_connected = self.is_connected.clone();
        let db_tree_view = self.db_tree_view.clone();
        let objects_panel = self.objects_panel.clone();

        let global_state = cx.global::<db::GlobalDbState>().clone();
        let stored_conn_id = conn.id.unwrap_or(0).to_string();

        cx.spawn(async move |cx| {
            let config = db::DbConnectionConfig {
                id: stored_conn_id.clone(),
                database_type: conn.db_type,
                name: conn.name.clone(),
                host: conn.host.clone(),
                port: conn.port,
                username: conn.username.clone(),
                password: conn.password.clone(),
                database: conn.database.clone(),
            };

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

            match global_state.connection_pool.get_connection(config.clone(), &global_state.db_manager).await {
                Ok(conn_arc) => {
                    // Load databases and expand first one
                    let first_database =  {
                        let conn = conn_arc.read().await;
                        plugin.list_databases(&**conn).await.ok()
                            .and_then(|dbs| dbs.first().cloned())
                    };

                    cx.update(|cx| {
                        is_connected.update(cx, |flag, cx| {
                            *flag = true;
                            cx.notify();
                        });

                        status_msg.update(cx, |s, cx| {
                            *s = format!("Connected to {}", config.name);
                            cx.notify();
                        });

                        db_tree_view.update(cx, |tree, cx| {
                            tree.set_connection_name(config.name.clone());
                            // 直接刷新树以加载数据库列表
                            tree.refresh_tree(cx);
                        });

                        // Load objects for first database
                        if let Some(db) = first_database {
                            objects_panel.update(cx, |panel, cx| {
                                panel.handle_node_selected(
                                    db.clone(),
                                    db::types::DbNodeType::Database,
                                    config.clone(),
                                    cx
                                );
                            });
                        }
                    })
                        .ok();
                }
                Err(e) => {
                    cx.update(|cx| {
                        status_msg.update(cx, |s, cx| {
                            *s = format!("Connection failed: {}", e);
                            cx.notify();
                        });
                    })
                        .ok();
                }
            }
        })
            .detach();
    }

    fn render_connection_status(&self, cx: &mut App) -> AnyElement {
        let status_text = self.status_msg.read(cx).clone();
        let is_error = status_text.contains("Failed") || status_text.contains("failed");

        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_6()
            .child(
                // Loading animation or error icon
                div()
                    .w(px(64.0))
                    .h(px(64.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .w(px(48.0))
                            .h(px(48.0))
                            .rounded(px(24.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(!is_error, |this| {
                                // Loading animation - simple circle
                                this.border_4()
                                    .border_color(cx.theme().accent)
                                    .text_2xl()
                                    .text_color(cx.theme().accent)
                                    .child("⟳")
                            })
                            .when(is_error, |this| {
                                // Error state - red circle
                                this.bg(Hsla::red())
                                    .text_color(gpui::white())
                                    .text_2xl()
                                    .child("✕")
                            })
                    )
            )
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .child(format!("Database Connection: {}", self.connection_info.name))
            )
            .child(
                v_flex()
                    .gap_2()
                    .p_4()
                    .bg(cx.theme().muted)
                    .rounded(px(8.0))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Host:")
                            )
                            .child(self.connection_info.host.clone())
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Port:")
                            )
                            .child(format!("{}", self.connection_info.port))
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Username:")
                            )
                            .child(self.connection_info.username.clone())
                    )
                    .when_some(self.connection_info.database.as_ref(), |this, db| {
                        this.child(
                            h_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("Database:")
                                )
                                .child(db.clone())
                        )
                    })
            )
            .child(
                div()
                    .text_lg()
                    .when(!is_error, |this| {
                        this.text_color(cx.theme().accent)
                    })
                    .when(is_error, |this| {
                        this.text_color(Hsla::red())
                    })
                    .child(status_text)
            )
            .into_any_element()
    }

    fn render_toolbar(&self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        use gpui_component::button::Button;

        let db_tree_view = self.db_tree_view.clone();
        let tab_container = self.tab_container.clone();
        let connection_info = self.connection_info.clone();

        h_flex()
            .w_full()
            .h(px(36.0))
            .px_2()
            .gap_2()
            .items_center()
            .bg(cx.theme().background)
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("refresh-tree")
                    .icon(IconName::Loader)
                    .child("刷新")
                    .ghost()
                    .tooltip("刷新")
            )
            .child(
                Button::new("new-query")
                    .icon(IconName::File)
                    .child("新建查询")
                    .ghost()
                    .tooltip("新建查询")
            )
            .child(
                Button::new("new-table")
                    .icon(IconName::TABLE)
                    .child("新建表")
                    .ghost()
                    .tooltip("新建表")
                    .on_click(move |_, window, cx| {
                        use crate::table_designer_view::TableDesignerView;
                        
                        // 获取当前选中的数据库
                        let current_db = db_tree_view.read(cx).get_selected_database();
                        let database = current_db.unwrap_or_else(|| "default".to_string());
                        let config = connection_info.to_db_connection();
                        
                        let tab_id = format!("new-table-{}", Uuid::new_v4());
                        
                        tab_container.update(cx, |container, cx| {
                            let table_designer = TableDesignerView::new_table(
                                database,
                                config.id,
                                config.database_type,
                                window,
                                cx,
                            );
                            let tab = TabItem::new(tab_id, table_designer.read(cx).clone());
                            container.add_and_activate_tab(tab, cx);
                        });
                    })
            )
    }
}

impl TabContent for DatabaseTabContent {
    fn title(&self) -> SharedString {
        self.connection_info.name.clone().into()
    }

    fn icon(&self) -> Option<IconName> {
        Some(IconName::File)
    }

    fn closeable(&self) -> bool {
        true
    }

    fn render_content(&self, window: &mut Window, cx: &mut App) -> AnyElement {
        let is_connected_flag = *self.is_connected.read(cx);

        if !is_connected_flag {
            // Show loading/connection status
            self.render_connection_status(cx)
        } else {
            // Show layout with toolbar on top, resizable panels below
            v_flex()
                .size_full()
                .child(self.render_toolbar(window, cx))
                .child(
                    h_resizable("db-panels")
                        .child(
                            resizable_panel()
                                .size(px(280.0))
                                .size_range(px(200.0)..px(500.0))
                                .child(self.db_tree_view.clone())
                        )
                        .child(
                            resizable_panel()
                                .child(self.tab_container.clone())
                        )
                )
                .into_any_element()
        }
    }

    fn content_type(&self) -> TabContentType {
        TabContentType::Custom(format!("database-{}", self.connection_info.name))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Clone for DatabaseTabContent {
    fn clone(&self) -> Self {
        Self {
            connection_info: self.connection_info.clone(),
            tab_container: self.tab_container.clone(),
            db_tree_view: self.db_tree_view.clone(),
            objects_panel: self.objects_panel.clone(),
            status_msg: self.status_msg.clone(),
            is_connected: self.is_connected.clone(),
            event_handler: self.event_handler.clone(),
        }
    }
}
