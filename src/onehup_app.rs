use crate::connection_store::ConnectionStore;
use crate::home::HomeTabContent;
use crate::setting_tab::SettingsTabContent;
use crate::tab_container::{TabContainer, TabContentType, TabItem};
use crate::themes;
use crate::themes::SwitchThemeMode;
use db::DatabaseType;
use gpui::{div, prelude::FluentBuilder, px, App, AppContext, Context, Entity, IntoElement, KeyBinding, ParentElement, Render, Styled, Window};
use gpui_component::dock::{ClosePanel, ToggleZoom};
use gpui_component::ThemeMode;
use gpui_component::{button::Button, h_flex, v_flex, ActiveTheme, IconName, Selectable};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub fn init(cx: &mut App) {
   
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("gpui_component=trace".parse().unwrap()),
        )
        .init();

    gpui_component::init(cx);
    themes::init(cx);
    cx.bind_keys(vec![
        KeyBinding::new("shift-escape", ToggleZoom, None),
        KeyBinding::new("ctrl-w", ClosePanel, None),
    ]);

    cx.activate(true);
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionType {
    All,
    Database,
    SshSftp,
    Redis,
    MongoDB,
}
impl ConnectionType {
    fn label(&self) -> &'static str {
        match self {
            ConnectionType::All => "全部",
            ConnectionType::Database => "数据库",
            ConnectionType::SshSftp => "SSH/SFTP",
            ConnectionType::Redis => "Redis",
            ConnectionType::MongoDB => "MongoDB",
        }
    }

    fn icon(&self) -> IconName {
        match self {
            ConnectionType::All => IconName::Menu,
            ConnectionType::Database => IconName::File,
            ConnectionType::SshSftp => IconName::File,
            ConnectionType::Redis => IconName::File,
            ConnectionType::MongoDB => IconName::File,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub id: Option<i64>,
    pub name: String,
    pub connection_type: ConnectionType,
    pub db_type: DatabaseType,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: Option<String>,
    pub status: String,
}

pub struct OneHupApp {
    selected_filter: ConnectionType,
    connections: Vec<ConnectionInfo>,
    tab_container: Entity<TabContainer>,
}

impl OneHupApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 从存储加载连接
        let connection_store = ConnectionStore::new().expect("Failed to create connection store");
        let stored_connections = connection_store.load_connections().unwrap_or_else(|_| vec![]);

        let connections: Vec<ConnectionInfo> = stored_connections.into_iter().map(|stored| {
            let connection_type = match stored.db_type {
                DatabaseType::MySQL | DatabaseType::PostgreSQL => ConnectionType::Database,
            };

            ConnectionInfo {
                id: stored.id,
                name: stored.name.clone(),
                connection_type,
                db_type: stored.db_type,
                host: stored.host.clone(),
                port: stored.port,
                username: stored.username.clone(),
                password: stored.password.clone(),
                database: stored.database.clone(),
                status: "未连接".to_string(),
            }
        }).collect();

        // 创建标签容器
        let tab_container = cx.new(|cx| TabContainer::new(window, cx));

        // 添加主页标签
        tab_container.update(cx, |tc, cx| {
            let home_tab = TabItem::new("home", HomeTabContent::new(connections.clone(), tab_container.clone()));
            tc.add_and_activate_tab(home_tab, cx);
        });

        Self {
            selected_filter: ConnectionType::All,
            connections,
            tab_container,
        }
    }

    pub fn add_settings_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.tab_container.update(cx, |tc, cx| {
            // 检查设置标签是否已存在
            let settings_type = TabContentType::Custom("settings".to_string());
            if !tc.has_tab_type(&settings_type) {
                let settings_tab = TabItem::new("settings", SettingsTabContent::new());
                tc.add_and_activate_tab(settings_tab, cx);
            } else {
                // 如果已存在，则激活它
                tc.activate_or_create(&settings_type, || {
                    TabItem::new("settings", SettingsTabContent::new())
                }, window, cx);
            }
        });
    }





    fn render_active_tab_content(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tab_container = self.tab_container.read(cx);
        if let Some(active_tab) = tab_container.active_tab() {
            let content = active_tab.content().clone();
            let is_home_tab = active_tab.content().content_type() == TabContentType::Custom("home".to_string());
            let _ = tab_container; // 释放借用

            // 主内容区域：垂直布局，包含工具栏和内容
            v_flex()
                .size_full()
                .when(is_home_tab, |this| {
                    // 只在主页显示工具栏
                    this.child(self.render_toolbar(window, cx))
                })
                .child(
                    div()
                        .flex_1()
                        .w_full()
                        .overflow_hidden()
                        .child(content.render_content(window, cx))
                )
        } else {
            v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .child("没有活动标签")
        }
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
                                .on_click(cx.listener(move |this: &mut OneHupApp, _, _, cx| {
                                    this.selected_filter = filter_type_clone.clone();
                                    cx.notify(); // 直接触发重新渲染
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
                            .on_click(cx.listener(|_this: &mut OneHupApp, _, _window, cx| {
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
                            .on_click(cx.listener(|this: &mut OneHupApp, _, window, cx| {
                                this.add_settings_tab(window, cx);
                            }))
                    )
            )
    }




}

impl Render for OneHupApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 检查当前活动标签是否是主页
        let is_home_tab = self.tab_container.read(cx)
            .active_tab()
            .map(|tab| tab.content().content_type() == TabContentType::Custom("home".to_string()))
            .unwrap_or(false);

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(
                // 顶部标签栏 - 避开 macOS 操作栏
                div()
                    .bg(gpui::rgb(0x2d2d2d)) // 深色背景
                    .w_full()
                    .pl(px(80.0)) // 为 macOS 红绿黄按钮留出空间
                    .child(self.tab_container.clone())
            )
            .child(
                // 下面是侧边栏和主内容的水平布局
                h_flex()
                    .flex_1()
                    .w_full()
                    .when(is_home_tab, |this| {
                        // 只在主页显示侧边栏
                        this.child(self.render_sidebar(window, cx))
                    })
                    .child(
                        // 主内容区域 - 显示活动标签的内容
                        div()
                            .flex_1()
                            .h_full()
                            .bg(cx.theme().background)
                            .child(self.render_active_tab_content(window, cx))
                    )
            )
    }
}

