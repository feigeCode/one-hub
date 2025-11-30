use one_core::tab_container::{TabContainer, TabContent, TabContentType, TabItem};
use one_core::storage::StoredConnection;
use std::any::Any;
use gpui::prelude::FluentBuilder;
use gpui::{div, px, AnyElement, App, AppContext, Context, Entity, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, Subscription, Window};
use gpui_component::resizable::{h_resizable, resizable_panel};
use gpui_component::{h_flex, v_flex, ActiveTheme, IconName};
use gpui_component::button::ButtonVariants;
use uuid::Uuid;
use db::{GlobalDbState, DbNode};
use one_core::gpui_tokio::Tokio;
use crate::database_objects_tab::DatabaseObjectsPanel;
use crate::db_tree_view::DbTreeView;

// Event handler for database tree view events
struct DatabaseEventHandler {
    _tree_subscription: Subscription,
}

impl DatabaseEventHandler {
    fn new(
        db_tree_view: &Entity<DbTreeView>,
        tab_container: Entity<TabContainer>,
        objects_panel: Entity<DatabaseObjectsPanel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        use crate::db_tree_view::DbTreeViewEvent;

        let tab_container_clone = tab_container.clone();
        let objects_panel_clone = objects_panel.clone();
        let global_state = cx.global::<GlobalDbState>().clone();

        let tree_subscription = cx.subscribe_in(db_tree_view, window, move |_handler, _tree, event, window, cx| {
            let global_state = global_state.clone();
            let tab_container = tab_container_clone.clone();
            let objects_panel = objects_panel_clone.clone();

            match event {
                DbTreeViewEvent::NodeSelected { node } => {
                    Self::handle_node_selected(node.clone(), global_state, objects_panel, cx);
                }
                DbTreeViewEvent::CreateNewQuery { node } => {
                    Self::handle_create_new_query(node.clone(), tab_container, window, cx);
                }
                DbTreeViewEvent::OpenTableData { node } => {
                    Self::handle_open_table_data(node.clone(), global_state, tab_container, window, cx);
                }
                DbTreeViewEvent::OpenViewData { node } => {
                    Self::handle_open_view_data(node.clone(), global_state, tab_container, window, cx);
                }
                DbTreeViewEvent::OpenTableStructure { node } => {
                    Self::handle_open_table_structure(node.clone(), global_state, tab_container, window, cx);
                }
                DbTreeViewEvent::ImportData { node } => {
                    Self::handle_import_data(node.clone(), global_state, window, cx);
                }
                DbTreeViewEvent::ExportData { node } => {
                    Self::handle_export_data(node.clone(), global_state, window, cx);
                }
            }
        });

        Self {
            _tree_subscription: tree_subscription,
        }
    }

    /// 处理节点选中事件
    fn handle_node_selected(
        node: DbNode,
        global_state: GlobalDbState,
        objects_panel: Entity<DatabaseObjectsPanel>,
        cx: &mut App,
    ) {
        let node_id = node.id.clone();
        let node_type = node.node_type.clone();
        let connection_id = node.connection_id.clone();

        let config = Tokio::block_on(cx, async move {
            global_state.get_config(&connection_id).await
        });

        if let Some(config) = config {
            objects_panel.update(cx, |panel, cx| {
                panel.handle_node_selected(node_id, node_type, config, cx);
            });
        }
    }

    /// 处理创建新查询事件
    fn handle_create_new_query(
        node: DbNode,
        tab_container: Entity<TabContainer>,
        window: &mut Window,
        cx: &mut App,
    ) {
        use crate::sql_editor_view::SqlEditorTabContent;

        let connection_id = node.connection_id.clone();
        // 获取数据库名：如果是数据库节点则用 name，否则用 parent_context
        let database = node.name.clone();
        let sql_editor = SqlEditorTabContent::new_with_config(
            format!("{} - Query", database),
            connection_id,
            Some(database.clone()),
            window,
            cx,
        );

        tab_container.update(cx, |container, cx| {
            let tab_id = format!("query-{}-{}", database, Uuid::new_v4());
            let tab = TabItem::new(tab_id, sql_editor);
            container.add_and_activate_tab(tab, cx);
        });
    }

    /// 处理打开表数据事件
    fn handle_open_table_data(
        node: DbNode,
        global_state: GlobalDbState,
        tab_container: Entity<TabContainer>,
        window: &mut Window,
        cx: &mut App,
    ) {
        use crate::table_data_tab::TableDataTabContent;

        let connection_id = node.connection_id.clone();
        let table = node.name.clone();
        let metadata = &node.metadata.unwrap();
        let database = metadata.get("database").unwrap();
        let tab_id = format!("table-data-{}.{}", database, table);

        let config = Tokio::block_on(cx, async move {
            global_state.get_config(&connection_id).await
        });
        if let Some(config) = config {
            let database_clone = database.clone();
            let table_clone = table.clone();
            let config_id = config.id.clone();
            let tab_id_clone = tab_id.clone();

            tab_container.update(cx, |container, cx| {
                container.activate_or_add_tab_lazy(
                    tab_id,
                    move |window, cx| {
                        let table_data = TableDataTabContent::new(
                            database_clone,
                            table_clone,
                            config_id,
                            window,
                            cx,
                        );
                        TabItem::new(tab_id_clone, table_data)
                    },
                    window,
                    cx,
                );
            });
        }
    }

    /// 处理打开视图数据事件
    fn handle_open_view_data(
        node: DbNode,
        global_state: GlobalDbState,
        tab_container: Entity<TabContainer>,
        window: &mut Window,
        cx: &mut App,
    ) {
        use crate::table_data_tab::TableDataTabContent;

        let connection_id = node.connection_id.clone();
        let view = node.name.clone();
        let metadata = &node.metadata.unwrap();
        let database = metadata.get("database").unwrap();
        let tab_id = format!("view-data-{}.{}", database, view);

        let config = Tokio::block_on(cx, async move {
            global_state.get_config(&connection_id).await
        });

        if let Some(config) = config {
            let database_clone = database.clone();
            let view_clone = view.clone();
            let config_id = config.id.clone();
            let tab_id_clone = tab_id.clone();

            tab_container.update(cx, |container, cx| {
                container.activate_or_add_tab_lazy(
                    tab_id,
                    move |window, cx| {
                        let view_data = TableDataTabContent::new(
                            database_clone,
                            view_clone,
                            config_id,
                            window,
                            cx,
                        );
                        TabItem::new(tab_id_clone, view_data)
                    },
                    window,
                    cx,
                );
            });
        }
    }

    /// 处理打开表结构事件
    fn handle_open_table_structure(
        node: DbNode,
        global_state: GlobalDbState,
        tab_container: Entity<TabContainer>,
        window: &mut Window,
        cx: &mut App,
    ) {
        use crate::table_designer_view::TableDesignerView;

        let connection_id = node.connection_id.clone();
        let table = node.name.clone();
        let metadata = &node.metadata.unwrap();
        let database = metadata.get("database").unwrap();
        let tab_id = format!("table-designer-{}.{}", database, table);

        let config = Tokio::block_on(cx, async move {
            global_state.get_config(&connection_id).await
        });

        if let Some(config) = config {
            let database_clone = database.clone();
            let table_clone = table.clone();
            let config_id = config.id.clone();
            let database_type = config.database_type;
            let tab_id_clone = tab_id.clone();

            tab_container.update(cx, |container, cx| {
                container.activate_or_add_tab_lazy(
                    tab_id,
                    move |window, cx| {
                        let table_designer = TableDesignerView::edit_table(
                            database_clone,
                            table_clone,
                            config_id,
                            database_type,
                            window,
                            cx,
                        );
                        TabItem::new(tab_id_clone, table_designer.read(cx).clone())
                    },
                    window,
                    cx,
                );
            });
        }
    }

    /// 处理导入数据事件
    fn handle_import_data(
        node: DbNode,
        global_state: GlobalDbState,
        window: &mut Window,
        cx: &mut App,
    ) {
        use crate::data_import_view::DataImportView;
        use gpui_component::WindowExt;

        let connection_id = node.connection_id.clone();
        // 获取数据库名：如果是数据库节点则用 name，否则用 parent_context
        let database = node.parent_context.clone().unwrap_or_else(|| node.name.clone());

        eprintln!("Opening import dialog for database: {}", database);

        let config = Tokio::block_on(cx, async move {
            global_state.get_config(&connection_id).await
        });

        if let Some(config) = config {
            let import_view = DataImportView::new(
                config.id,
                database.clone(),
                window,
                cx,
            );

            eprintln!("Import view created, opening dialog...");

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
    }

    /// 处理导出数据事件
    fn handle_export_data(
        node: DbNode,
        global_state: GlobalDbState,
        window: &mut Window,
        cx: &mut App,
    ) {
        use crate::data_export_view::DataExportView;
        use gpui_component::WindowExt;

        let connection_id = node.connection_id.clone();
        // 获取数据库名：如果是数据库节点则用 name，否则用 parent_context
        let database = node.parent_context.clone().unwrap_or_else(|| node.name.clone());
        // 如果是表节点，预填表名
        let table_name = if node.node_type == db::DbNodeType::Table {
            Some(node.name.clone())
        } else {
            None
        };

        let config = Tokio::block_on(cx, async move {
            global_state.get_config(&connection_id).await
        });

        if let Some(config) = config {
            let export_view = DataExportView::new(
                config.id,
                database.clone(),
                window,
                cx,
            );

            // 如果有表名则预填
            if let Some(table) = table_name {
                export_view.update(cx, |view, cx| {
                    view.tables.update(cx, |state, cx| {
                        state.set_value(table, window, cx);
                    });
                });
            }

            window.open_dialog(cx, move |dialog, _window, _cx| {
                dialog
                    .title("Export Data")
                    .child(export_view.clone())
                    .width(px(800.0))
                    .on_cancel(|_, _window, _cx| true)
            });
        }
    }
}

// Database connection tab content - using TabContainer architecture
pub struct DatabaseTabContent {
    connections: Vec<StoredConnection>,
    tab_container: Entity<TabContainer>,
    db_tree_view: Entity<DbTreeView>,
    objects_panel: Entity<DatabaseObjectsPanel>,
    status_msg: Entity<String>,
    is_connected: Entity<bool>,
    event_handler: Option<Entity<DatabaseEventHandler>>,
}

impl DatabaseTabContent {
    pub fn new(connections: Vec<StoredConnection>, window: &mut Window, cx: &mut App) -> Self {
        // Create database tree view
        let db_tree_view = cx.new(|cx| {
            DbTreeView::new(&connections, window, cx)
        });

        // Create tab container - use default theme colors for automatic theme switching
        let tab_container = cx.new(|cx| {
            TabContainer::new(window, cx)
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

        let status_msg = cx.new(|_| "Ready".to_string());
        let is_connected = cx.new(|_| true);

        // Create event handler to handle tree view events
        let event_handler = cx.new(|cx| {
            DatabaseEventHandler::new(&db_tree_view, tab_container.clone(), objects_panel.clone(), window, cx)
        });

        // 注册连接配置到 GlobalDbState，然后自动连接
        let global_state = cx.global::<GlobalDbState>().clone();
        let connections_clone = connections.clone();

        cx.spawn(async move |_cx| {
            // 先注册所有连接
            for conn in &connections_clone {
                let db_config = conn.to_db_connection();
                let _ = global_state.register_connection(db_config).await;
            }
        }).detach();

        Self {
            connections: connections.clone(),
            tab_container,
            db_tree_view,
            objects_panel,
            status_msg,
            is_connected,
            event_handler: Some(event_handler),
        }
    }

    fn render_connection_status(&self, cx: &mut App) -> AnyElement {
        let status_text = self.status_msg.read(cx).clone();
        let is_error = status_text.contains("Failed") || status_text.contains("failed");

        // 获取第一个连接信息用于显示
        let first_conn = self.connections.first();
        let conn_name = first_conn.map(|c| c.name.clone()).unwrap_or_else(|| "Unknown".to_string());
        let conn_host = first_conn.map(|c| c.host.clone()).unwrap_or_default();
        let conn_port = first_conn.map(|c| c.port).unwrap_or(0);
        let conn_username = first_conn.map(|c| c.username.clone()).unwrap_or_default();
        let conn_database = first_conn.and_then(|c| c.database.clone());

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
                    .child(format!("Database Connection: {}", conn_name))
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
                            .child(conn_host)
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Port:")
                            )
                            .child(format!("{}", conn_port))
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Username:")
                            )
                            .child(conn_username)
                    )
                    .when_some(conn_database, |this, db| {
                        this.child(
                            h_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("Database:")
                                )
                                .child(db)
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
        let first_conn = self.connections.first().cloned();

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
                    .icon(IconName::Table)
                    .child("新建表")
                    .ghost()
                    .tooltip("新建表")
                    .on_click(move |_, window, cx| {
                        use crate::table_designer_view::TableDesignerView;

                        if let Some(conn) = first_conn.as_ref() {
                            // 获取当前选中的数据库
                            let current_db = db_tree_view.read(cx).get_selected_database();
                            let database = current_db.unwrap_or_else(|| "default".to_string());
                            let config = conn.to_db_connection();

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
                        }
                    })
            )
    }
}

impl TabContent for DatabaseTabContent {
    fn title(&self) -> SharedString {
        self.connections.first()
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "Database".to_string())
            .into()
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
        let name = self.connections.first()
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "unknown".to_string());
        TabContentType::Custom(format!("database-{}", name))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Clone for DatabaseTabContent {
    fn clone(&self) -> Self {
        Self {
            connections: self.connections.clone(),
            tab_container: self.tab_container.clone(),
            db_tree_view: self.db_tree_view.clone(),
            objects_panel: self.objects_panel.clone(),
            status_msg: self.status_msg.clone(),
            is_connected: self.is_connected.clone(),
            event_handler: self.event_handler.clone(),
        }
    }
}
