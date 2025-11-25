use std::any::Any;

use anyhow::Error;
use gpui::{div, px, AnyElement, App, AppContext, Context, Entity, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window};
use gpui::prelude::FluentBuilder;
use gpui_component::{
    button::Button, h_flex, v_flex, ActiveTheme, IconName, Selectable, ThemeMode,
};

use core::storage::{ConnectionRepository, ConnectionType, GlobalStorageState, StoredConnection};
use core::storage::traits::Repository;
use core::tab_container::{TabContainer, TabContent, TabContentType, TabItem};
use core::themes::SwitchThemeMode;
use db::{DatabaseType, DbConnectionConfig};
use db_view::database_tab::DatabaseTabContent;
use db_view::db_connection_form::{DbConnectionForm, DbConnectionFormEvent, DbFormConfig};

use crate::setting_tab::SettingsTabContent;

// HomePage Entity - 管理 home 页面的所有状态
pub struct HomePage {
    selected_filter: ConnectionType,
    connections: Vec<StoredConnection>,
    tab_container: Entity<TabContainer>,
    connection_form: Option<Entity<DbConnectionForm>>,
}

impl HomePage {
    pub fn new(tab_container: Entity<TabContainer>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut page = Self {
            selected_filter: ConnectionType::All,
            connections: Vec::new(),
            tab_container,
            connection_form: None,
        };

        // 异步加载连接列表
        page.load_connections(cx);
        page
    }

    fn load_connections(&mut self, cx: &mut Context<Self>) {
        let storage = cx.global::<GlobalStorageState>().storage.clone();

        let task = core::gpui_tokio::Tokio::spawn(cx, async move {
            let repo = storage.get::<ConnectionRepository>().await
                .ok_or_else(|| anyhow::anyhow!("ConnectionRepository not found"))?;
            let pool = storage.get_pool().await?;
            let result: anyhow::Result<Vec<StoredConnection>> = repo.list(&pool).await;
            result
        });

        cx.spawn(async move |this, cx| {
            let task_result = task.await;
            match task_result {
                Ok(result) => match result {
                    Ok(connections) => {
                        _ = this.update(cx, |this, cx| {
                            this.connections = connections;
                            cx.notify();
                        });
                    }
                    Err(e) => {
                        tracing::error!("Failed to load connections: {}", e);
                    }
                }
                Err(e) => {
                    tracing::error!("Task join error: {}", e);
                }
            }
        }).detach();
    }

    fn show_connection_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let form = cx.new(|cx| {
            DbConnectionForm::new(DbFormConfig::mysql(), window, cx)
        });

        // 订阅表单事件
        cx.subscribe_in(&form, window, |this, form, event, window, cx| {
            match event {
                DbConnectionFormEvent::TestConnection(db_type, config) => {
                    this.handle_test_connection(form.clone(), *db_type, config.clone(), window, cx);
                }
                DbConnectionFormEvent::Save(db_type, config) => {
                    this.handle_save_connection(*db_type, config.clone(), window, cx);
                }
                DbConnectionFormEvent::Cancel => {
                    this.connection_form = None;
                    cx.notify();
                }
            }
        }).detach();

        self.connection_form = Some(form);
        cx.notify();
    }

    fn handle_test_connection(
        &mut self,
        form: Entity<DbConnectionForm>,
        db_type: DatabaseType,
        config: DbConnectionConfig,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let global_state = cx.global::<db::GlobalDbState>().clone();
        cx.spawn(async move |_, cx| {
            let manager = global_state.db_manager;

            // Test connection and collect result
            let test_result = async {
                let db_plugin = manager.get_plugin(&db_type)?;
                let conn = db_plugin.create_connection(config).await?;
                conn.ping().await?;
                Ok::<bool, Error>(true)
            }.await;

            match test_result {
                Ok(_) => {
                    form.update(cx, |form, cx1| {
                        form.set_test_result(Ok(true), cx1)
                    })
                }
                Err(_) => {
                    form.update(cx, |form, cx1| {
                        form.set_test_result(Err("测试连接失败".to_string()), cx1)
                    })
                }
            }
        }).detach();
    }

    fn handle_save_connection(
        &mut self,
        _db_type: DatabaseType,
        config: DbConnectionConfig,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut stored = StoredConnection {
            id: None,
            name: config.name.clone(),
            db_type: config.database_type.clone(),
            connection_type: ConnectionType::Database,
            host: config.host.clone(),
            port: config.port,
            username: config.username.clone(),
            password: config.password.clone(),
            database: config.database.clone(),
            created_at: None,
            updated_at: None,
        };

        let storage = cx.global::<GlobalStorageState>().storage.clone();

        let task = core::gpui_tokio::Tokio::spawn(cx, async move {
            let repo = storage.get::<ConnectionRepository>().await
                .ok_or_else(|| anyhow::anyhow!("ConnectionRepository not found"))?;
            let pool = storage.get_pool().await?;
            repo.insert(&pool, &mut stored).await?;
            let result: anyhow::Result<StoredConnection> = Ok(stored);
            result
        });

        cx.spawn(async move |this, cx| {
            let task_result = task.await;
            match task_result {
                Ok(result) => match result {
                    Ok(saved_conn) => {
                        _ = this.update(cx, |this, cx| {
                            this.connections.push(saved_conn);
                            this.connection_form = None;
                            cx.notify();
                        });
                    }
                    Err(e) => {
                        tracing::error!("Failed to save connection: {}", e);
                    }
                }
                Err(e) => {
                    tracing::error!("Task join error: {}", e);
                }
            }
        }).detach();
    }

    pub fn add_settings_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.tab_container.update(cx, |tc, cx| {
            tc.activate_or_add_tab_lazy("settings", |_, _| {
                TabItem::new("settings", SettingsTabContent::new())
            }, window, cx);
        });
    }

    fn render_toolbar(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .p_4()
            .border_b_1()
            .border_color(cx.theme().border)
            .justify_between()
            .items_center()
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("new_connect")
                            .icon(IconName::Plus)
                            .label("NEW CONNECT")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.show_connection_form(window, cx);
                            }))
                    )
            )
    }

    fn render_sidebar(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let filter_types = vec![
            ConnectionType::All,
            ConnectionType::Database,
            ConnectionType::SshSftp,
            ConnectionType::Redis,
            ConnectionType::MongoDB,
        ];

        v_flex()
            .w(px(200.0))
            .h_full()
            .bg(cx.theme().muted)
            .border_r_1()
            .border_color(cx.theme().border)
            .child(
                // 侧边栏过滤选项
                v_flex()
                    .flex_1()
                    .w_full()
                    .p_4()
                    .gap_2()
                    .children(
                        filter_types.into_iter().map(|filter_type| {
                            let is_selected = self.selected_filter == filter_type;
                            let filter_type_clone = filter_type.clone();

                            Button::new(filter_type.label())
                                .icon(filter_type.icon())
                                .label(filter_type.label())
                                .w_full()
                                .justify_start()
                                .when(is_selected, |this| {
                                    this.selected(true)
                                })
                                .on_click(cx.listener(move |this: &mut HomePage, _, _, cx| {
                                    this.selected_filter = filter_type_clone.clone();
                                    cx.notify();
                                }))
                        })
                    )
            )
            .child(
                // 底部区域：主题切换和用户头像
                v_flex()
                    .w_full()
                    .p_4()
                    .gap_3()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new("theme_toggle")
                            .icon(IconName::Palette)
                            .label("切换主题")
                            .w_full()
                            .justify_start()
                            .on_click(cx.listener(|_this: &mut HomePage, _, _window, cx| {
                                // 切换主题模式
                                let current_mode = cx.theme().mode;
                                let new_mode = match current_mode {
                                    ThemeMode::Light => ThemeMode::Dark,
                                    ThemeMode::Dark => ThemeMode::Light,
                                };
                                cx.dispatch_action(&SwitchThemeMode(new_mode));
                            }))
                    )
                    .child(
                        Button::new("open_settings")
                            .icon(IconName::Settings)
                            .label("设置")
                            .w_full()
                            .justify_start()
                            .on_click(cx.listener(|this: &mut HomePage, _, window, cx| {
                                this.add_settings_tab(window, cx);
                            }))
                    )
            )
    }

    fn render_connection_cards(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tab_container = self.tab_container.clone();

        let connection_cards: Vec<_> = self.connections.iter().map(|conn| {
            let icon_bg = match conn.connection_type {
                ConnectionType::Database => Hsla::blue(),
                ConnectionType::SshSftp => cx.theme().accent,
                ConnectionType::Redis => Hsla::red(),
                ConnectionType::MongoDB => Hsla::green(),
                _ => cx.theme().accent,
            };

            let conn_clone = conn.clone();
            let tab_container_clone = tab_container.clone();

            div()
                .id(SharedString::from(format!("conn-card-{}", conn.name)))
                .w_full()
                .p_4()
                .rounded(px(12.0))
                .bg(cx.theme().background)
                .border_1()
                .border_color(cx.theme().border)
                .cursor_pointer()
                .hover(|style| {
                    style
                        .bg(cx.theme().muted)
                        .border_color(cx.theme().accent)
                })
                .on_click(move |_, window_inner, cx_inner| {
                    // 打开数据库标签
                    let tab_id = format!("database-{}", conn_clone.name);
                    let tab_type = TabContentType::Custom(tab_id.clone());

                    // 先检查标签是否已存在
                    let should_create = !tab_container_clone.read(cx_inner).has_tab_type(&tab_type);

                    if should_create {
                        // 创建 DatabaseTabContent
                        let db_content = DatabaseTabContent::new(conn_clone.clone(), window_inner, cx_inner);
                        let db_tab = TabItem::new(tab_id.clone(), db_content);

                        tab_container_clone.update(cx_inner, |tc, cx| {
                            tc.add_and_activate_tab(db_tab, cx);
                        });
                    } else {
                        tab_container_clone.update(cx_inner, |tc, cx| {
                            tc.set_active_by_id(&tab_id, window_inner, cx);
                        });
                    }
                })
                .child(
                    h_flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .w(px(48.0))
                                .h(px(48.0))
                                .rounded(px(10.0))
                                .bg(icon_bg)
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(gpui::white())
                                .font_weight(FontWeight::BOLD)
                                .text_lg()
                                .child(
                                    match conn.connection_type {
                                        ConnectionType::Database => "DB",
                                        ConnectionType::SshSftp => "SSH",
                                        ConnectionType::Redis => "R",
                                        ConnectionType::MongoDB => "M",
                                        _ => "?",
                                    }
                                )
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .gap_1()
                                .child(
                                    div()
                                        .text_base()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(cx.theme().foreground)
                                        .child(conn.name.clone())
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("{}@{}:{}", conn.username, conn.host, conn.port))
                                )
                                .when_some(conn.database.as_ref(), |this, db| {
                                    this.child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!("数据库: {}", db))
                                    )
                                })
                        )
                )
        }).collect();

        div()
            .id("home-content")
            .size_full()
            .overflow_scroll()
            .p_6()
            .child(
                div()
                    .grid()
                    .grid_cols(3)
                    .gap_4()
                    .children(connection_cards)
            )
    }
}

impl Render for HomePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let form = self.connection_form.clone();

        h_flex()
            .size_full()
            .child(self.render_sidebar(window, cx))
            .child(
                v_flex()
                    .flex_1()
                    .h_full()
                    .bg(cx.theme().background)
                    .child(self.render_toolbar(window, cx))
                    .child(
                        div()
                            .flex_1()
                            .w_full()
                            .overflow_hidden()
                            .child(self.render_connection_cards(cx))
                    )
            )
            .when_some(form, |this, form| {
                this.child(form)
            })
    }
}

// HomeTabContent - TabContent 的薄包装层
pub struct HomeTabContent {
    home_page: Entity<HomePage>,
}

impl HomeTabContent {
    pub fn new(tab_container: Entity<TabContainer>, window: &mut Window, cx: &mut App) -> Self {
        let home_page = cx.new(|cx| HomePage::new(tab_container, window, cx));
        Self {
            home_page,
        }
    }
}

impl TabContent for HomeTabContent {
    fn title(&self) -> SharedString {
        "首页".into()
    }

    fn icon(&self) -> Option<IconName> {
        Some(IconName::LayoutDashboard)
    }

    fn closeable(&self) -> bool {
        false // 首页不可关闭
    }

    fn render_content(&self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        self.home_page.clone().into_any_element()
    }

    fn content_type(&self) -> TabContentType {
        TabContentType::Custom("home".to_string())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
