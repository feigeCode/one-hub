use std::any::Any;
use gpui::{div, px, AnyElement, App, AppContext, Context, Entity, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, Window};
use gpui::prelude::FluentBuilder;
use gpui_component::{h_flex, v_flex, ActiveTheme, IconName};
use gpui_component::button::{Button, ButtonVariants};
use crate::onehup_app::ConnectionInfo;
use crate::tab_container::{TabContainer, TabContent, TabContentType, TabItem};

// 数据库连接页面内容 - 参考 AppView 实现完整的数据库管理界面
pub struct DatabaseTabContent {
    connection_info: ConnectionInfo,
    db_tree_view: Entity<crate::db_tree_view::DbTreeView>,
    inner_tab_container: Entity<TabContainer>,
    status_msg: Entity<String>,
    is_connected: Entity<bool>,
    event_handler: Entity<DatabaseEventHandler>,
}

// 事件处理器 - 用于订阅树视图事件
struct DatabaseEventHandler {
    inner_tab_container: Entity<TabContainer>,
    _tree_subscription: Option<gpui::Subscription>,
}

impl DatabaseEventHandler {
    fn new(
        db_tree_view: &Entity<crate::db_tree_view::DbTreeView>,
        inner_tab_container: Entity<TabContainer>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        use crate::db_tree_view::DbTreeViewEvent;

        let inner_tab_container_clone = inner_tab_container.clone();
        let tree_subscription = cx.subscribe_in(db_tree_view, window, move |_handler, _tree, event, window, cx| {
            match event {
                DbTreeViewEvent::CreateNewQuery { database } => {
                    use crate::sql_editor_view::SqlEditorTabContent;

                    let tab_count = inner_tab_container_clone.read(cx).tabs().len();
                    let sql_editor_content = SqlEditorTabContent::new_with_database(
                        format!("{} - Query {}", database, tab_count + 1),
                        Some(database.clone()),
                        window,
                        cx,
                    );

                    sql_editor_content.load_databases(window, cx);

                    let tab = TabItem::new(
                        format!("sql-editor-{}-{}", database, tab_count + 1),
                        sql_editor_content,
                    );

                    inner_tab_container_clone.update(cx, |tc, cx| {
                        tc.add_and_activate_tab(tab, cx);
                    });
                }
                DbTreeViewEvent::OpenTableData { database, table } => {
                    use crate::tab_contents::TableDataTabContent;

                    let tab_id = format!("table-data-{}-{}", database, table);

                    inner_tab_container_clone.update(cx, |tc, cx| {
                        if let Some(index) = tc.tabs().iter().position(|t| t.id() == tab_id) {
                            tc.set_active_index(index, window, cx);
                        } else {
                            let tab_title = format!("{}.{}", database, table);
                            let tab = TabItem::new(
                                tab_id.clone(),
                                TableDataTabContent::new(tab_title, window, cx),
                            );
                            tc.add_and_activate_tab(tab, cx);
                        }
                    });
                }
                DbTreeViewEvent::OpenViewData { database, view } => {
                    use crate::tab_contents::TableDataTabContent;

                    let tab_id = format!("view-data-{}-{}", database, view);

                    inner_tab_container_clone.update(cx, |tc, cx| {
                        if let Some(index) = tc.tabs().iter().position(|t| t.id() == tab_id) {
                            tc.set_active_index(index, window, cx);
                        } else {
                            let tab_title = format!("{}.{}", database, view);
                            let tab = TabItem::new(
                                tab_id.clone(),
                                TableDataTabContent::new(tab_title, window, cx),
                            );
                            tc.add_and_activate_tab(tab, cx);
                        }
                    });
                }
                DbTreeViewEvent::OpenTableStructure { database, table } => {
                    use crate::tab_contents::TableStructureTabContent;

                    let tab_id = format!("table-structure-{}-{}", database, table);

                    inner_tab_container_clone.update(cx, |tc, cx| {
                        if let Some(index) = tc.tabs().iter().position(|t| t.id() == tab_id) {
                            tc.set_active_index(index, window, cx);
                        } else {
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
                DbTreeViewEvent::ConnectToConnection { .. } => {
                    // 已经连接，忽略
                }
            }
        });

        Self {
            inner_tab_container,
            _tree_subscription: Some(tree_subscription),
        }
    }
}

impl DatabaseTabContent {
    pub fn new(connection_info: ConnectionInfo, window: &mut Window, cx: &mut App) -> Self {
        use crate::storage::StoredConnection;

        // 创建一个临时的 StoredConnection 用于初始化 DbTreeView
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

        // 创建数据库树视图
        let db_tree_view = cx.new(|cx| {
            crate::db_tree_view::DbTreeView::new(&vec![stored_conn], window, cx)
        });

        // 创建内部标签容器
        let inner_tab_container = cx.new(|cx| TabContainer::new(window, cx));

        // 创建事件处理器
        let event_handler = cx.new(|cx| {
            DatabaseEventHandler::new(&db_tree_view, inner_tab_container.clone(), window, cx)
        });

        let status_msg = cx.new(|_| "正在连接...".to_string());
        let is_connected = cx.new(|_| false);

        let instance = Self {
            connection_info,
            db_tree_view,
            inner_tab_container,
            status_msg,
            is_connected,
            event_handler,
        };

        // 自动开始连接
        instance.start_connection(cx);

        instance
    }

    fn start_connection(&self, cx: &mut App) {
        let status_msg = self.status_msg.clone();
        let is_connected = self.is_connected.clone();
        let conn = self.connection_info.clone();
        let db_tree_view = self.db_tree_view.clone();

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
                            *s = format!("获取插件失败: {}", e);
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

                    cx.update(|cx| {
                        is_connected.update(cx, |flag, cx| {
                            *flag = true;
                            cx.notify();
                        });

                        status_msg.update(cx, |s, cx| {
                            *s = format!("已连接到 {}", config.name);
                            cx.notify();
                        });

                        db_tree_view.update(cx, |tree, cx| {
                            tree.set_connection_name(config.name.clone());
                            tree.update_connection_node(&stored_conn_id, cx);
                        });
                    })
                        .ok();
                }
                Err(e) => {
                    cx.update(|cx| {
                        status_msg.update(cx, |s, cx| {
                            *s = format!("连接失败: {}", e);
                            cx.notify();
                        });
                    })
                        .ok();
                }
            }
        })
            .detach();
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
        let status_msg_render = self.status_msg.clone();
        let is_connected_flag = *self.is_connected.read(cx);

        if !is_connected_flag {
            // 显示加载动画
            let status_text = status_msg_render.read(cx).clone();
            let is_error = status_text.contains("失败");

            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_6()
                .child(
                    // 加载动画或错误图标
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
                                    // 加载动画 - 简单的圆圈
                                    this.border_4()
                                        .border_color(cx.theme().accent)
                                        .text_2xl()
                                        .text_color(cx.theme().accent)
                                        .child("⟳")
                                })
                                .when(is_error, |this| {
                                    // 错误状态 - 红色圆圈
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
                        .child(format!("数据库连接: {}", self.connection_info.name))
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
                                        .child("主机:")
                                )
                                .child(self.connection_info.host.clone())
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("端口:")
                                )
                                .child(format!("{}", self.connection_info.port))
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("用户名:")
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
                                            .child("数据库:")
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
                .into_any_element();
        }

        // 已连接 - 显示完整的数据库管理界面（类似 AppView）
        use gpui_component::resizable::{h_resizable, resizable_panel};
        use gpui_component::{Sizable, Size};

        v_flex()
            .size_full()
            .gap_2()
            .child(
                // 工具栏
                h_flex()
                    .gap_2()
                    .p_2()
                    .bg(cx.theme().muted)
                    .rounded_md()
                    .items_center()
                    .w_full()
                    .child(
                        Button::new("new-query")
                            .with_size(Size::Small)
                            .primary()
                            .label("新建查询")
                            .on_click({
                                let inner_tab_container = self.inner_tab_container.clone();
                                move |_, window, cx| {
                                    use crate::sql_editor_view::SqlEditorTabContent;

                                    let tab_count = inner_tab_container.read(cx).tabs().len();
                                    let sql_editor_content = SqlEditorTabContent::new(
                                        format!("Query {}", tab_count + 1),
                                        window,
                                        cx,
                                    );

                                    sql_editor_content.load_databases(window, cx);

                                    let tab = TabItem::new(
                                        format!("sql-editor-{}", tab_count + 1),
                                        sql_editor_content,
                                    );

                                    inner_tab_container.update(cx, |tc, cx| {
                                        tc.add_and_activate_tab(tab, cx);
                                    });
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
                // 主内容区域 - 左侧树视图，右侧标签容器
                div()
                    .flex_1()
                    .w_full()
                    .child(
                        h_resizable(SharedString::from(format!("db-main-{}", self.connection_info.name)))
                            .child(
                                resizable_panel()
                                    .size(px(300.))
                                    .size_range(px(200.)..px(500.))
                                    .child(self.db_tree_view.clone()),
                            )
                            .child(
                                resizable_panel()
                                    .child({
                                        // 获取活动标签的内容
                                        let active_tab_content = self.inner_tab_container.read(cx)
                                            .active_tab()
                                            .map(|tab| tab.content().clone());

                                        // 垂直布局：标签栏 + 内容
                                        v_flex()
                                            .size_full()
                                            .child(self.inner_tab_container.clone())
                                            .child(
                                                // 标签内容区域
                                                div()
                                                    .flex_1()
                                                    .w_full()
                                                    .overflow_hidden()
                                                    .when_some(active_tab_content, |el, content| {
                                                        el.child(content.render_content(window, cx))
                                                    })
                                            )
                                    }),
                            ),
                    ),
            )
            .into_any_element()
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
            db_tree_view: self.db_tree_view.clone(),
            inner_tab_container: self.inner_tab_container.clone(),
            status_msg: self.status_msg.clone(),
            is_connected: self.is_connected.clone(),
            event_handler: self.event_handler.clone(),
        }
    }
}