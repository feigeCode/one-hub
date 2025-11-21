use std::any::Any;
use std::sync::Arc;
use gpui::{
    div, px, AnyElement, App, AppContext, Context, Entity, FontWeight, Hsla, IntoElement,
    ParentElement, SharedString, Styled, Window, WeakEntity, Edges, Task, Subscription,
};
use gpui::prelude::FluentBuilder;
use gpui_component::{h_flex, v_flex, ActiveTheme, IconName};
use gpui_component::dock::{DockArea, DockAreaState, DockItem, PanelView};
use crate::onehup_app::ConnectionInfo;
use crate::tab_container::{TabContent, TabContentType};

// Constants for dock area
const DATABASE_TAB_DOCK_VERSION: usize = 1;

// Event handler for database tree view events
struct DatabaseEventHandler {
    _tree_subscription: Subscription,
}

impl DatabaseEventHandler {
    fn new(
        db_tree_view: &Entity<crate::db_tree_view::DbTreeView>,
        dock_area: WeakEntity<DockArea>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        use crate::db_tree_view::DbTreeViewEvent;

        let dock_area_clone = dock_area.clone();
        let tree_subscription = cx.subscribe_in(db_tree_view, window, move |_handler, _tree, event, window, cx| {
            match event {
                DbTreeViewEvent::CreateNewQuery { database } => {
                    use crate::sql_editor_view::SqlEditorTabContent;

                    // Create new SQL editor
                    let sql_editor = cx.new(|cx| {
                        let editor = SqlEditorTabContent::new_with_database(
                            format!("{} - Query", database),
                            Some(database.clone()),
                            window,
                            cx,
                        );
                        editor
                    });

                    // Add to center area of DockArea
                    if let Ok(_) = dock_area_clone.update(cx, |dock_area, cx| {
                        let panel: Arc<dyn PanelView> = Arc::new(sql_editor.clone());
                        dock_area.add_panel(panel, gpui_component::dock::DockPlacement::Center, None, window, cx);
                    }) {
                        // Successfully added
                    }
                }
                DbTreeViewEvent::OpenTableData { database, table } => {
                    use crate::tab_contents::TableDataTabContent;

                    // Create table data panel
                    let table_data = cx.new(|cx| {
                        TableDataTabContent::new(
                            format!("{}.{}", database, table),
                            window,
                            cx,
                        )
                    });

                    // Add to center area of DockArea
                    if let Ok(_) = dock_area_clone.update(cx, |dock_area, cx| {
                        let panel: Arc<dyn PanelView> = Arc::new(table_data.clone());
                        dock_area.add_panel(panel, gpui_component::dock::DockPlacement::Center, None, window, cx);
                    }) {
                        // Successfully added
                    }
                }
                DbTreeViewEvent::OpenViewData { database, view } => {
                    use crate::tab_contents::TableDataTabContent;

                    // Create view data panel (reuse TableDataTabContent)
                    let view_data = cx.new(|cx| {
                        TableDataTabContent::new(
                            format!("{}.{}", database, view),
                            window,
                            cx,
                        )
                    });

                    // Add to center area of DockArea
                    if let Ok(_) = dock_area_clone.update(cx, |dock_area, cx| {
                        let panel: Arc<dyn PanelView> = Arc::new(view_data.clone());
                        dock_area.add_panel(panel, gpui_component::dock::DockPlacement::Center, None, window, cx);
                    }) {
                        // Successfully added
                    }
                }
                DbTreeViewEvent::OpenTableStructure { database, table } => {
                    use crate::tab_contents::TableStructureTabContent;

                    // Create table structure panel
                    let table_structure = cx.new(|cx| {
                        TableStructureTabContent::new(
                            database.clone(),
                            table.clone(),
                            window,
                            cx,
                        )
                    });

                    // Add to center area of DockArea
                    if let Ok(_) = dock_area_clone.update(cx, |dock_area, cx| {
                        let panel: Arc<dyn PanelView> = Arc::new(table_structure.clone());
                        dock_area.add_panel(panel, gpui_component::dock::DockPlacement::Center, None, window, cx);
                    }) {
                        // Successfully added
                    }
                }
                DbTreeViewEvent::ConnectToConnection { .. } => {
                    // Already connected, ignore
                }
            }
        });

        Self {
            _tree_subscription: tree_subscription,
        }
    }
}

// Database connection tab content - using DockArea architecture
pub struct DatabaseTabContent {
    connection_info: ConnectionInfo,
    dock_area: Entity<DockArea>,
    last_layout_state: Option<DockAreaState>,
    _save_layout_task: Option<Task<()>>,
    db_tree_view: Entity<crate::db_tree_view::DbTreeView>,
    objects_panel: Entity<crate::database_objects_panel::DatabaseObjectsPanel>,
    status_msg: Entity<String>,
    is_connected: Entity<bool>,
    event_handler: Option<Entity<DatabaseEventHandler>>,
}

impl DatabaseTabContent {
    pub fn new(connection_info: ConnectionInfo, window: &mut Window, cx: &mut App) -> Self {
        use crate::storage::StoredConnection;

        // Create a temporary StoredConnection for DbTreeView initialization
        let stored_conn = StoredConnection {
            id: connection_info.id,
            name: connection_info.name.clone(),
            db_type: connection_info.db_type,
            host: connection_info.host.clone(),
            port: connection_info.port,
            username: connection_info.username.clone(),
            password: connection_info.password.clone(),
            database: connection_info.database.clone(),
            created_at: None,
            updated_at: None,
        };

        // Create database tree view
        let db_tree_view = cx.new(|cx| {
            crate::db_tree_view::DbTreeView::new(&vec![stored_conn], window, cx)
        });

        // Create DockArea for this database connection
        let dock_id = format!("db-dock-{}", connection_info.id.unwrap_or(0));
        let dock_area = cx.new(|cx| {
            DockArea::new(dock_id, Some(DATABASE_TAB_DOCK_VERSION), window, cx)
        });

        // Create objects panel as first tab in center
        let objects_panel = cx.new(|cx| {
            crate::database_objects_panel::DatabaseObjectsPanel::new(window, cx)
        });

        // Setup the dock layout - tree view on left, objects panel + sql editor in center
        let weak_dock_area = dock_area.downgrade();
        dock_area.update(cx, |dock_area, cx| {
            // Add tree view to left dock
            let panel_view: Arc<dyn PanelView> = Arc::new(db_tree_view.clone());
            let left_dock_item = DockItem::tabs(vec![panel_view], Some(0), &weak_dock_area, window, cx);
            dock_area.set_left_dock(left_dock_item, Some(px(280.0)), true, window, cx);

            // Add objects panel and SQL editor to center area (objects panel first)
            let objects_panel_view: Arc<dyn PanelView> = Arc::new(objects_panel.clone());
            // let sql_editor_panel: Arc<dyn PanelView> = Arc::new(sql_editor.clone());
            let center_dock_item = DockItem::tabs(
                vec![objects_panel_view], 
                Some(0), // Objects panel is active by default
                &weak_dock_area, 
                window, 
                cx
            );
            dock_area.set_center(center_dock_item, window, cx);

            // Set collapsible edges
            dock_area.set_dock_collapsible(
                Edges {
                    left: true,
                    bottom: false,
                    right: false,
                    ..Default::default()
                },
                window,
                cx,
            );
        });

        let status_msg = cx.new(|_| "Connecting...".to_string());
        let is_connected = cx.new(|_| false);

        // Create event handler to handle tree view events
        let event_handler = cx.new(|cx| {
            DatabaseEventHandler::new(&db_tree_view, weak_dock_area.clone(), window, cx)
        });

        let instance = Self {
            connection_info: connection_info.clone(),
            dock_area,
            last_layout_state: None,
            _save_layout_task: None,
            db_tree_view,
            objects_panel,
            status_msg,
            is_connected,
            event_handler: Some(event_handler),
        };

        // Automatically start connection
        instance.start_connection(connection_info, cx);

        instance
    }

    fn start_connection(&self, conn: ConnectionInfo, cx: &mut App) {
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

            match plugin.create_connection(config.clone()).await {
                Ok(connection) => {
                    global_state
                        .connection_pool
                        .add_connection(stored_conn_id.clone(), connection, config.clone())
                        .await;

                    global_state
                        .connection_pool
                        .set_current_connection(stored_conn_id.clone())
                        .await;

                    if let Some(db) = config.database.as_ref() {
                        global_state
                            .connection_pool
                            .set_current_database(Some(db.clone()))
                            .await;
                    }

                    // Load databases and expand first one
                    let conn_arc = global_state.connection_pool.get_current_connection().await;
                    let first_database = if let Some(conn_arc) = conn_arc {
                        let conn = conn_arc.read().await;
                        plugin.list_databases(&**conn).await.ok()
                            .and_then(|dbs| dbs.first().cloned())
                    } else {
                        None
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
                            tree.update_connection_node(&stored_conn_id, cx);
                            
                            // Auto-expand first database if available
                            if let Some(ref db) = first_database {
                                tree.expand_database(&stored_conn_id, db, cx);
                            }
                        });

                        // Load objects for first database
                        if let Some(db) = first_database {
                            objects_panel.update(cx, |panel, cx| {
                                panel.set_database(db, cx);
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

    fn render_content(&self, _window: &mut Window, cx: &mut App) -> AnyElement {
        let is_connected_flag = *self.is_connected.read(cx);

        if !is_connected_flag {
            // Show loading/connection status
            self.render_connection_status(cx)
        } else {
            // Show DockArea - it manages the entire layout
            div()
                .size_full()
                .child(self.dock_area.clone())
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
            dock_area: self.dock_area.clone(),
            last_layout_state: self.last_layout_state.clone(),
            _save_layout_task: None, // Don't clone tasks
            db_tree_view: self.db_tree_view.clone(),
            objects_panel: self.objects_panel.clone(),
            status_msg: self.status_msg.clone(),
            is_connected: self.is_connected.clone(),
            event_handler: self.event_handler.clone(),
        }
    }
}
