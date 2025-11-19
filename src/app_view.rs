use gpui::{div, px, AppContext as _, ClickEvent, Context, Entity, IntoElement, ParentElement, Render, Styled as _, Subscription, Window};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    resizable::{h_resizable, resizable_panel},
    v_flex,
    ActiveTheme as _, Sizable as _, Size,
};
use std::collections::HashMap;

use crate::connection_store::ConnectionStore;
use db::{GlobalDbState, DatabaseType, DbConnectionConfig};
use crate::db_connection_form::{DbConnectionForm, DbConnectionFormEvent, DbFormConfig};
use crate::db_tree_view::{DbTreeView, DbTreeViewEvent};
use crate::sql_editor_view::SqlEditorTabContent;
use crate::storage::StoredConnection;
use crate::tab_container::{TabContainer, TabItem};
use crate::tab_contents::{TableDataTabContent, TableStructureTabContent};

/// Main application view with database tree and tabbed interface
pub struct AppView {
    db_tree_view: Entity<DbTreeView>,
    tab_container: Entity<TabContainer>,
    status_msg: Entity<String>,
    connection_form: Option<Entity<DbConnectionForm>>,
    connection_store: ConnectionStore,

    // 多连接管理字段
    active_connections: HashMap<String, ConnectionInfo>,
    active_connection_id: Option<String>,

    _form_subscription: Option<Subscription>,
    _tree_subscription: Option<Subscription>,
}

/// 连接信息
#[derive(Clone, Debug)]
struct ConnectionInfo {
    id: String,
    name: String,
    db_type: DatabaseType,
    config: DbConnectionConfig,
    is_connected: bool,
}




impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let connection_store = ConnectionStore::new().expect("Failed to create connection store");
        let connections = connection_store.load_connections().unwrap();

        // 创建数据库树视图
        let db_tree_view = cx.new(|cx| DbTreeView::new(&connections, window, cx));

        // 创建标签容器
        let tab_container = cx.new(|cx| TabContainer::new(window, cx));

        let status_msg = cx.new(|_| "Not connected".to_string());

        // 订阅树视图事件
        let tab_container_for_event = tab_container.clone();
        let tree_subscription = cx.subscribe_in(&db_tree_view, window, move |view, _tree, event, window, cx| {
            match event {
                DbTreeViewEvent::ConnectToConnection {id, name } => {
                    // 处理连接选择 - 使用优化后的多连接管理
                    view.connect_to_stored(&id, &name, window, cx);
                }
                DbTreeViewEvent::CreateNewQuery { database } => {
                    // 为特定数据库创建新查询
                    let tab_count = view.tab_container.read(cx).tabs().len();
                    let sql_editor_content = SqlEditorTabContent::new_with_database(
                        format!("{} - Query {}", database, tab_count + 1),
                        Some(database.clone()),
                        window,
                        cx,
                    );

                    // 使用 load_databases 加载数据库列表
                    sql_editor_content.load_databases(window, cx);

                    let tab = TabItem::new(
                        format!("sql-editor-{}-{}", database, tab_count + 1),
                        sql_editor_content,
                    );

                    view.tab_container.update(cx, |tc, cx| {
                        tc.add_and_activate_tab(tab, cx);
                    });
                }
                DbTreeViewEvent::OpenTableData { database, table } => {
                    // Create unique tab ID and content type
                    let tab_id = format!("table-data-{}-{}", database, table);

                    tab_container_for_event.update(cx, |tc, cx| {
                        // Check if tab already exists
                        if let Some(index) = tc.tabs()
                            .iter()
                            .position(|t| &t.id() == &tab_id)
                        {
                            // Tab exists, just activate it
                            tc.set_active_index(index, window, cx);
                        } else {
                            // Create new tab
                            let tab_title = format!("{}.{}", database, table);
                            let tab = TabItem::new(
                                tab_id.clone(),
                                TableDataTabContent::new(
                                    tab_title,
                                    window,
                                    cx,
                                ),
                            );
                            tc.add_and_activate_tab(tab, cx);
                        }
                    });
                }
                DbTreeViewEvent::OpenViewData { database, view } => {
                    // Create unique tab ID and content type
                    let tab_id = format!("view-data-{}-{}", database, view);

                    tab_container_for_event.update(cx, |tc, cx| {
                        // Check if tab already exists
                        if let Some(index) = tc.tabs()
                            .iter()
                            .position(|t| &t.id() == &tab_id)
                        {
                            // Tab exists, just activate it
                            tc.set_active_index(index, window, cx);
                        } else {
                            // Create new tab
                            let tab_title = format!("{}.{}", database, view);
                            let tab = TabItem::new(
                                tab_id.clone(),
                                TableDataTabContent::new(
                                    tab_title,
                                    window,
                                    cx,
                                ),
                            );
                            tc.add_and_activate_tab(tab, cx);
                        }
                    });
                }
                DbTreeViewEvent::OpenTableStructure { database, table } => {
                    // Create unique tab ID and content type
                    let tab_id = format!("table-structure-{}-{}", database, table);

                    tab_container_for_event.update(cx, |tc, cx| {
                        // Check if tab already exists
                        if let Some(index) = tc.tabs()
                            .iter()
                            .position(|t| &t.id() == &tab_id)
                        {
                            // Tab exists, just activate it
                            tc.set_active_index(index, window, cx);
                        } else {
                            // Create new tab
                            let tab = TabItem::new(
                                tab_id.clone(),
                                TableStructureTabContent::new(
                                    database.clone(),
                                    table.clone(),
                                    window,
                                    cx,
                                ),
                            );
                            tc.add_and_activate_tab(tab, cx);
                        }
                    });
                }
            }
        });

        Self {
            db_tree_view,
            tab_container,
            status_msg,
            connection_form: None,
            connection_store,
            active_connections: HashMap::new(),
            active_connection_id: None,
            _form_subscription: None,
            _tree_subscription: Some(tree_subscription),
        }
    }

    /// 连接到存储的连接（使用优化后的多连接管理）
    fn connect_to_stored(&mut self, id: &str, name: &str, _window: &mut Window, cx: &mut Context<Self>) {
        let global_state = cx.global::<GlobalDbState>().clone();
        let db_tree_view = self.db_tree_view.clone();
        let status_msg = self.status_msg.clone();
        let stored = self.connection_store.get_connection(&id).ok().flatten();

        if let Some(stored_conn) = stored {
            let db_type = stored_conn.db_type;
            let config = stored_conn.to_db_connection();
            let connection_id = stored_conn.id.clone().unwrap().to_string();

            // 添加到活动连接
            let conn_info = ConnectionInfo {
                id: connection_id.clone(),
                name: config.name.clone(),
                db_type,
                config: config.clone(),
                is_connected: false,
            };
            self.active_connections.insert(connection_id.clone(), conn_info);

            // 异步连接
            let connection_id_clone = connection_id.clone();
            cx.spawn(async move |view, cx| {
                // 获取插件并创建连接
                let plugin = match global_state.db_manager.get_plugin(&db_type) {
                    Ok(p) => p,
                    Err(e) => {
                        view.update(cx, |view, cx| {
                            view.active_connections.remove(&connection_id_clone);
                            status_msg.update(cx, |msg, _| {
                                *msg = format!("Failed to get plugin: {}", e);
                            });
                            cx.notify();
                        }).ok();
                        return;
                    }
                };

                // 创建连接
                match plugin.create_connection(config.clone()).await {
                    Ok(connection) => {
                        // 添加到连接池
                        global_state.connection_pool
                            .add_connection(connection_id_clone.clone(), connection, config.clone())
                            .await;

                        // 设置为当前连接
                        global_state.connection_pool
                            .set_current_connection(connection_id_clone.clone())
                            .await;

                        // 设置当前数据库
                        if let Some(db) = config.database.as_ref() {
                            global_state.connection_pool
                                .set_current_database(Some(db.clone()))
                                .await;
                        }

                        // 更新UI
                        view.update(cx, |view, cx| {
                            // 更新连接状态
                            if let Some(conn) = view.active_connections.get_mut(&connection_id_clone) {
                                conn.is_connected = true;
                            }
                            view.active_connection_id = Some(connection_id_clone.clone());

                            // 更新状态消息
                            status_msg.update(cx, |msg, _| {
                                *msg = format!("Connected to {} ({} active connections)",
                                    config.name, view.active_connections.len());
                            });

                            // 更新树视图
                            db_tree_view.update(cx, |tree, cx| {
                                tree.set_connection_name(config.name.clone());
                                tree.update_connection_node(&connection_id_clone, cx);
                            });

                            // Update all open SQL editor tabs with databases
                            view.update_sql_editor_tabs_databases(cx);

                            cx.notify();
                        }).ok();
                    }
                    Err(e) => {
                        view.update(cx, |view, cx| {
                            // 连接失败，移除连接
                            view.active_connections.remove(&connection_id_clone);

                            status_msg.update(cx, |msg, _| {
                                *msg = format!("Connection failed: {}", e);
                            });

                            cx.notify();
                        }).ok();
                    }
                }
            }).detach();
        } else {
            self.status_msg.update(cx, |msg, _| {
                *msg = format!("Connection not found: {}", name);
            });
        }
    }

    /// 更新所有 SQL 编辑器标签的数据库列表
    fn update_sql_editor_tabs_databases(&mut self, _cx: &mut Context<Self>) {
        // For now, we'll skip this complex async update
        // The databases will be loaded when tabs are opened or when explicitly refreshed
        // This avoids the complex window handle passing issue
    }

    fn handle_connect(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        // Show connection form modal
        let form = cx.new(|cx| DbConnectionForm::new(DbFormConfig::mysql(), window, cx));

        let status_msg = self.status_msg.clone();
        let global_state = cx.global::<GlobalDbState>().clone();
        let form_clone = form.clone();
        let db_tree_view = self.db_tree_view.clone();

        let subscription = cx.subscribe_in(&form, window, move |view, _form, event, window, cx| {
            match event {
                DbConnectionFormEvent::TestConnection(db_type, config) => {
                    let form = form_clone.clone();
                    let global_state = global_state.clone();
                    let config = config.clone();
                    let db_type = *db_type;

                    // Spawn test connection task
                    cx.spawn(async move |_, cx| {
                        // 获取插件
                        let plugin = match global_state.db_manager.get_plugin(&db_type) {
                            Ok(p) => p,
                            Err(e) => {
                                form.update(cx, |comp, cx| {
                                    comp.set_test_result(Err(format!("Failed to get plugin: {}", e)), cx);
                                    cx.notify();
                                }).ok();
                                return;
                            }
                        };

                        // 尝试创建连接
                        let result = match plugin.create_connection(config.clone()).await {
                            Ok(mut conn) => {
                                // 测试连接
                                match conn.ping().await {
                                    Ok(_) => {
                                        // 断开连接
                                        let _ = conn.disconnect().await;
                                        Ok(true)
                                    }
                                    Err(e) => Err(format!("Connection test failed: {}", e))
                                }
                            }
                            Err(e) => Err(format!("Failed to create connection: {}", e)),
                        };

                        // Update form with test result
                        form.update(cx, |comp, cx| {
                            comp.set_test_result(result, cx);
                            cx.notify();
                        }).ok();
                    }).detach();
                }
                DbConnectionFormEvent::Save(db_type, config) => {
                    let config = config.clone();
                    let status_msg = status_msg.clone();
                    let global_state = global_state.clone();
                    let form = form_clone.clone();
                    let db_tree_view = db_tree_view.clone();
                    let db_type = *db_type;

                    // 保存连接到存储
                    let stored_conn = StoredConnection::new(db_type, config.clone());
                    if let Err(e) = view.connection_store.save_connection(stored_conn) {
                        eprintln!("Failed to save connection: {}", e);
                    }

                    // 创建新连接ID
                    let connection_id = config.id.clone();

                    // 异步创建连接
                    cx.spawn(async move |view, cx| {
                        // 获取插件
                        let plugin = match global_state.db_manager.get_plugin(&db_type) {
                            Ok(p) => p,
                            Err(e) => {
                                view.update(cx, |_, cx| {
                                    status_msg.update(cx, |msg, _| {
                                        *msg = format!("Failed to get plugin: {}", e);
                                    });
                                    cx.notify();
                                }).ok();
                                return;
                            }
                        };

                        // 创建连接
                        match plugin.create_connection(config.clone()).await {
                            Ok(connection) => {
                                // 添加到连接池
                                global_state.connection_pool
                                    .add_connection(connection_id.clone(), connection, config.clone())
                                    .await;

                                // 设置为当前连接
                                global_state.connection_pool
                                    .set_current_connection(connection_id.clone())
                                    .await;

                                if let Some(db) = config.database.as_ref() {
                                    global_state.connection_pool
                                        .set_current_database(Some(db.clone()))
                                        .await;
                                }

                                // 更新UI
                                view.update(cx, |view, cx| {
                                    // 添加到活动连接
                                    let conn_info = ConnectionInfo {
                                        id: connection_id.clone(),
                                        name: config.name.clone(),
                                        db_type,
                                        config: config.clone(),
                                        is_connected: true,
                                    };
                                    view.active_connections.insert(connection_id.clone(), conn_info);
                                    view.active_connection_id = Some(connection_id.clone());

                                    // 更新状态消息
                                    status_msg.update(cx, |msg, _| {
                                        *msg = format!("Connected to {}", config.name);
                                    });

                                    // 更新树视图
                                    db_tree_view.update(cx, |tree, cx| {
                                        tree.set_connection_name(config.name.clone());
                                        tree.update_connection_node(&connection_id, cx);
                                    });

                                    // 关闭表单 - Form will be hidden automatically
                                    // form.update(cx, |comp, cx| {
                                    //     comp.close(window, cx);
                                    // }).ok();

                                    cx.notify();
                                }).ok();
                            }
                            Err(e) => {
                                view.update(cx, |_, cx| {
                                    status_msg.update(cx, |msg, _| {
                                        *msg = format!("Connection failed: {}", e);
                                    });
                                    cx.notify();
                                }).ok();
                            }
                        }
                    }).detach();
                }
                DbConnectionFormEvent::Cancel => {
                    // Just close the form
                }
            }
        });

        self.connection_form = Some(form);
        self._form_subscription = Some(subscription);
    }

    fn handle_new_query(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        let tab_count = self.tab_container.read(cx).tabs().len();
        let sql_editor_content = SqlEditorTabContent::new(
            format!("Query {}", tab_count + 1),
            window,
            cx,
        );

        // 使用 load_databases 加载数据库列表
        sql_editor_content.load_databases(window, cx);

        let tab = TabItem::new(
            format!("sql-editor-{}", tab_count + 1),
            sql_editor_content,
        );

        self.tab_container.update(cx, |tc, cx| {
            tc.add_and_activate_tab(tab, cx);
        });
    }
}

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                        Button::new("connect")
                            .with_size(Size::Small)
                            .label("Connect")
                            .on_click(cx.listener(Self::handle_connect)),
                    )
                    .child(
                        Button::new("new-query")
                            .with_size(Size::Small)
                            .primary()
                            .label("New Query")
                            .on_click(cx.listener(Self::handle_new_query)),
                    )
                    .child(
                        // Status
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
                // Main content area with resizable panels
                div()
                    .flex_1()
                    .w_full()
                    .child(
                        h_resizable("app-main")
                            .child(
                                // Left panel: Database tree view
                                resizable_panel()
                                    .size(px(300.))
                                    .size_range(px(200.)..px(500.))
                                    .child(self.db_tree_view.clone()),
                            )
                            .child(
                                // Right panel: Tab container
                                resizable_panel().child(self.tab_container.clone()),
                            ),
                    ),
            )
            .children(self.connection_form.clone())
    }
}
