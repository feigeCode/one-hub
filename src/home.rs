use std::any::Any;

use anyhow::Error;
use gpui::{div, px, AnyElement, App, AppContext, Context, Entity, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window};
use gpui::prelude::FluentBuilder;
use gpui_component::{button::{Button, DropdownButton}, h_flex, input::{Input, InputEvent, InputState}, menu::PopupMenuItem, v_flex, ActiveTheme, IconName, InteractiveElementExt, Selectable, Sizable, Size, ThemeMode};

use core::storage::{ConnectionRepository, ConnectionType, GlobalStorageState, StoredConnection};
use core::storage::traits::Repository;
use core::tab_container::{TabContainer, TabContent, TabContentType, TabItem};
use core::themes::SwitchThemeMode;
use db::{DatabaseType, DbConnectionConfig};
use db_view::database_tab::DatabaseTabContent;
use db_view::db_connection_form::{DbConnectionForm, DbConnectionFormEvent, DbFormConfig};
use gpui_component::menu::DropdownMenu;
use crate::setting_tab::SettingsTabContent;

// HomePage Entity - 管理 home 页面的所有状态
pub struct HomePage {
    selected_filter: ConnectionType,
    connections: Vec<StoredConnection>,
    tab_container: Entity<TabContainer>,
    connection_form: Option<Entity<DbConnectionForm>>,
    search_input: Entity<InputState>,
    search_query: Entity<String>,
    editing_connection_id: Option<i64>,
    selected_connection_id: Option<i64>,
}

impl HomePage {
    pub fn new(tab_container: Entity<TabContainer>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_query = cx.new(|_| String::new());
        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("搜索连接...")
        });

        // 订阅搜索输入变化
        let query_clone = search_query.clone();
        cx.subscribe_in(&search_input, window, move |_this, _input, event, _window, cx| {
            if let InputEvent::Change = event {
                query_clone.update(cx, |q, cx| {
                    *q = _input.read(cx).text().to_string();
                    cx.notify();
                });
                cx.notify();
            }
        })
        .detach();

        let mut page = Self {
            selected_filter: ConnectionType::All,
            connections: Vec::new(),
            tab_container,
            connection_form: None,
            search_input,
            search_query,
            editing_connection_id: None,
            selected_connection_id: None,
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

    fn show_connection_form(&mut self, db_type: DatabaseType, window: &mut Window, cx: &mut Context<Self>) {
        let config = match db_type {
            DatabaseType::MySQL => DbFormConfig::mysql(),
            DatabaseType::PostgreSQL => DbFormConfig::postgres(),
        };

        let form = cx.new(|cx| {
            DbConnectionForm::new(config, window, cx)
        });

        // 如果是编辑模式，加载现有连接数据
        if let Some(editing_id) = self.editing_connection_id {
            if let Some(conn) = self.connections.iter().find(|c| c.id == Some(editing_id)) {
                form.update(cx, |f, cx| {
                    f.load_connection(conn, window, cx);
                });
            }
        }

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
                    this.editing_connection_id = None;
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
        let editing_id = self.editing_connection_id;
        let mut stored = StoredConnection {
            id: editing_id,
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
            
            if editing_id.is_some() {
                repo.update(&pool, &mut stored).await?;
            } else {
                repo.insert(&pool, &mut stored).await?;
            }
            
            let result: anyhow::Result<StoredConnection> = Ok(stored);
            result
        });

        cx.spawn(async move |this, cx| {
            let task_result = task.await;
            match task_result {
                Ok(result) => match result {
                    Ok(saved_conn) => {
                        _ = this.update(cx, |this, cx| {
                            if let Some(editing_id) = editing_id {
                                if let Some(pos) = this.connections.iter().position(|c| c.id == Some(editing_id)) {
                                    this.connections[pos] = saved_conn;
                                }
                            } else {
                                this.connections.push(saved_conn);
                            }
                            this.connection_form = None;
                            this.editing_connection_id = None;
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

    fn add_item_to_tab(&mut self, conn: &StoredConnection, window: &mut Window, cx: &mut Context<Self>) {
        self.tab_container.update(cx, |tc, cx| {
            let tab_id = format!("database-{}", conn.name);
            tc.activate_or_add_tab_lazy(
                tab_id.clone(),
                {
                    let conn = conn.clone();
                    move |window, cx| {
                        let db_content = DatabaseTabContent::new(conn.clone(), window, cx);
                        TabItem::new(tab_id.clone(), db_content)
                    }
                },
                window,
                cx
            )
        });
    }

    fn render_toolbar(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let has_selection = self.selected_connection_id.is_some();
        
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
                        Button::new("new-connect-button")
                            .icon(IconName::Plus)
                            .with_size(Size::Large)
                            .dropdown_menu(move |menu, window, _cx| {
                                menu.item(
                                    PopupMenuItem::new("工作区")
                                                .icon(IconName::WindowRestore)
                                                .on_click(window.listener_for(&view, move |this, _, window, cx| {
                                                    this.editing_connection_id = None;
                                                    this.show_connection_form(DatabaseType::MySQL, window, cx);
                                                }))
                                ).item(
                                    PopupMenuItem::new("MySQL")
                                        .icon(IconName::DATABASE)
                                        .on_click(window.listener_for(&view, move |this, _, window, cx| {
                                            this.editing_connection_id = None;
                                            this.show_connection_form(DatabaseType::MySQL, window, cx);
                                        }))
                                ).item(
                                    PopupMenuItem::new("PostgreSQL")
                                        .icon(IconName::DATABASE)
                                        .on_click(window.listener_for(&view, move |this, _, window, cx| {
                                            this.editing_connection_id = None;
                                            this.show_connection_form(DatabaseType::PostgreSQL, window, cx);
                                        }))
                                )
                            })
                    )
                    .when(has_selection, |this| {
                        this.child(
                            Button::new("edit-selected")
                                .icon(IconName::Settings)
                                .label("编辑")
                                .with_size(Size::Medium)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if let Some(conn_id) = this.selected_connection_id {
                                        if let Some(conn) = this.connections.iter().find(|c| c.id == Some(conn_id)) {
                                            this.editing_connection_id = Some(conn_id);
                                            this.show_connection_form(conn.db_type, window, cx);
                                        }
                                    }
                                }))
                        )
                    })
            )
            .child(
                h_flex()
                    .gap_2()
                    .w(px(300.0))
                    .child(Input::new(&self.search_input).w_full())
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

    fn render_connection_cards(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let search_query = self.search_query.read(cx).to_lowercase();
        // 过滤连接列表
        let filtered_connections: Vec<_> = self.connections.iter()
            .filter(|conn| {
                if search_query.is_empty() {
                    return true;
                }
                conn.name.to_lowercase().contains(&search_query)
                    || conn.host.to_lowercase().contains(&search_query)
                    || conn.username.to_lowercase().contains(&search_query)
                    || conn.database.as_ref().map_or(false, |db| db.to_lowercase().contains(&search_query))
            })
            .cloned()
            .collect();

        let selected_id = self.selected_connection_id;
        let theme = cx.theme();
        let accent_color = theme.accent;
        let muted_color = theme.muted;
        let border_color = theme.border;
        let bg_color = theme.background;
        
        let connection_cards: Vec<_> = filtered_connections.into_iter().map(|conn| {
            let icon_bg = match conn.connection_type {
                ConnectionType::Database => Hsla::blue(),
                ConnectionType::SshSftp => accent_color,
                ConnectionType::Redis => Hsla::red(),
                ConnectionType::MongoDB => Hsla::green(),
                _ => accent_color,
            };

            let conn_id = conn.id;
            let clone_conn = conn.clone();
            let is_selected = selected_id == conn.id;
            div()
                .id(SharedString::from(format!("conn-card-{}", conn.id.unwrap_or(0))))
                .w_full()
                .p_4()
                .rounded(px(12.0))
                .bg(bg_color)
                .border_1()
                .when(is_selected, |this| {
                    this.border_color(accent_color)
                        .bg(muted_color)
                })
                .when(!is_selected, |this| {
                    this.border_color(border_color)
                })
                .cursor_pointer()
                .hover(|style| {
                    style
                        .bg(muted_color)
                        .border_color(accent_color)
                })
                .on_double_click(cx.listener(move |this, _, w, cx| {
                    this.add_item_to_tab(&clone_conn, w, cx);
                    cx.notify()
                }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    // 单击选中
                    this.selected_connection_id = conn_id;
                    cx.notify();
                }))
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

    fn width_size(&self) -> Option<Size> {
        Some(Size::Small)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
