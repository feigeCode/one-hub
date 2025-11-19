use gpui::{div, App, Context, IntoElement, KeyBinding, ParentElement, Render, Styled, Window, InteractiveElement, Hsla, px, FontWeight, prelude::FluentBuilder, AnyElement, SharedString, Entity, AppContext, StatefulInteractiveElement};
use gpui_component::{ActiveTheme, IconName, Selectable, button::Button, h_flex, v_flex, StyledExt};
use gpui_component::dock::{ClosePanel, ToggleZoom};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use crate::themes;
use crate::themes::SwitchThemeMode;
use gpui_component::ThemeMode;
use crate::tab_container::{TabContainer, TabContent, TabContentType, TabItem};
use std::any::Any;

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
    // TODO 这里初始化

    // let http_client = std::sync::Arc::new(
    //     reqwest_client::ReqwestClient::user_agent("gpui-component/story").unwrap(),
    // );
    // cx.set_http_client(http_client);

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

// 首页内容
pub struct HomeTabContent {
    connections: Vec<ConnectionInfo>,
}

impl HomeTabContent {
    pub fn new(connections: Vec<ConnectionInfo>) -> Self {
        Self {
            connections,
        }
    }
}

impl TabContent for HomeTabContent {
    fn title(&self) -> SharedString {
        "首页".into()
    }

    fn icon(&self) -> Option<IconName> {
        Some(IconName::File)
    }

    fn closeable(&self) -> bool {
        false // 首页不可关闭
    }

    fn render_content(&self, _window: &mut Window, cx: &mut App) -> AnyElement {
        let connection_cards: Vec<_> = self.connections.iter().map(|conn| {
            let icon_bg = match conn.connection_type {
                ConnectionType::Database => Hsla::blue(),
                ConnectionType::SshSftp => cx.theme().accent,
                ConnectionType::Redis => Hsla::red(),
                ConnectionType::MongoDB => Hsla::green(),
                _ => cx.theme().accent,
            };

            div()
                .p_4()
                .rounded(px(8.0))
                .bg(cx.theme().background)
                .border_1()
                .border_color(cx.theme().border)
                .hover(|style| style.border_color(cx.theme().accent))
                .cursor_pointer()
                .child(
                    h_flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .w(px(40.0))
                                .h(px(40.0))
                                .rounded(px(8.0))
                                .bg(icon_bg)
                                .flex()
                                .items_center()
                                .justify_center()
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
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(conn.name.clone())
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("{}, {}", conn.host, conn.status))
                                )
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
            .into_any_element()
    }

    fn content_type(&self) -> TabContentType {
        TabContentType::Custom("home".to_string())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

// 连接页面内容
pub struct ConnectionsTabContent {
    connections: Vec<ConnectionInfo>,
    selected_filter: ConnectionType,
}

impl ConnectionsTabContent {
    pub fn new(connections: Vec<ConnectionInfo>) -> Self {
        Self {
            connections,
            selected_filter: ConnectionType::All,
        }
    }

    pub fn set_filter(&mut self, filter: ConnectionType) {
        self.selected_filter = filter;
    }
}

impl TabContent for ConnectionsTabContent {
    fn title(&self) -> SharedString {
        "连接".into()
    }

    fn icon(&self) -> Option<IconName> {
        Some(IconName::File)
    }

    fn closeable(&self) -> bool {
        true
    }

    fn render_content(&self, _window: &mut Window, cx: &mut App) -> AnyElement {
        let filtered_connections: Vec<_> = self.connections.iter()
            .filter(|conn| {
                self.selected_filter == ConnectionType::All || conn.connection_type == self.selected_filter
            })
            .cloned()
            .collect();

        let connection_cards: Vec<_> = filtered_connections.into_iter().map(|conn| {
            let icon_bg = match conn.connection_type {
                ConnectionType::Database => Hsla::blue(),
                ConnectionType::SshSftp => cx.theme().accent,
                ConnectionType::Redis => Hsla::red(),
                ConnectionType::MongoDB => Hsla::green(),
                _ => cx.theme().accent,
            };

            div()
                .p_4()
                .rounded(px(8.0))
                .bg(cx.theme().background)
                .border_1()
                .border_color(cx.theme().border)
                .hover(|style| style.border_color(cx.theme().accent))
                .cursor_pointer()
                .child(
                    h_flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .w(px(40.0))
                                .h(px(40.0))
                                .rounded(px(8.0))
                                .bg(icon_bg)
                                .flex()
                                .items_center()
                                .justify_center()
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
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(conn.name.clone())
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("{}, {}", conn.host, conn.status))
                                )
                        )
                )
        }).collect();

        div()
            .id("connections-content")
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
            .into_any_element()
    }

    fn content_type(&self) -> TabContentType {
        TabContentType::Custom("connections".to_string())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

// 设置页面内容
pub struct SettingsTabContent;

impl SettingsTabContent {
    pub fn new() -> Self {
        Self
    }
}

impl TabContent for SettingsTabContent {
    fn title(&self) -> SharedString {
        "设置".into()
    }

    fn icon(&self) -> Option<IconName> {
        Some(IconName::Settings)
    }

    fn closeable(&self) -> bool {
        true
    }

    fn render_content(&self, _window: &mut Window, cx: &mut App) -> AnyElement {
        div()
            .flex_1()
            .p_6()
            .child(
                v_flex()
                    .gap_4()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .child("设置")
                    )
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child("应用程序设置和配置")
                    )
            )
            .into_any_element()
    }

    fn content_type(&self) -> TabContentType {
        TabContentType::Custom("settings".to_string())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
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
    pub host: String,
    pub port: u16,
    pub username: String,
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
        let connections = vec![
                ConnectionInfo {
                    name: "comiserver 开发 2".to_string(),
                    connection_type: ConnectionType::SshSftp,
                    host: "ssh, root, 公网".to_string(),
                    port: 22,
                    status: "已连接".to_string(),
                },
                ConnectionInfo {
                    name: "comi一体化".to_string(),
                    connection_type: ConnectionType::SshSftp,
                    host: "ssh, root, 公网".to_string(),
                    port: 22,
                    status: "已连接".to_string(),
                },
                ConnectionInfo {
                    name: "风雪comi".to_string(),
                    connection_type: ConnectionType::SshSftp,
                    host: "ssh, root, 公网".to_string(),
                    port: 22,
                    status: "已连接".to_string(),
                },
                ConnectionInfo {
                    name: "A82".to_string(),
                    connection_type: ConnectionType::Database,
                    host: "ssh, root, 公网".to_string(),
                    port: 3306,
                    status: "已连接".to_string(),
                },
                ConnectionInfo {
                    name: "国产机".to_string(),
                    connection_type: ConnectionType::Database,
                    host: "ssh, root, 公网".to_string(),
                    port: 3306,
                    status: "已连接".to_string(),
                },
                ConnectionInfo {
                    name: "深圳环境builder".to_string(),
                    connection_type: ConnectionType::SshSftp,
                    host: "ssh, root, 公网".to_string(),
                    port: 22,
                    status: "已连接".to_string(),
                },
            ];

        // 创建标签容器
        let tab_container = cx.new(|cx| TabContainer::new(window, cx));

        // 添加主页标签
        tab_container.update(cx, |tc, cx| {
            let home_tab = TabItem::new("home", HomeTabContent::new(connections.clone()));
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
            let _ = tab_container; // 释放借用

            // 主内容区域：垂直布局，包含工具栏和内容
            v_flex()
                .size_full()
                .child(self.render_toolbar(window, cx))
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
                        Button::new("new_host")
                            .icon(IconName::Plus)
                            .label("NEW HOST")
                    )
                    .child(
                        Button::new("terminal")
                            .icon(IconName::File)
                            .label("TERMINAL")
                    )
                    .child(
                        Button::new("serial")
                            .icon(IconName::File)
                            .label("SERIAL")
                    )
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(Button::new("grid_view").icon(IconName::Menu))
                    .child(Button::new("list_view").icon(IconName::Menu))
                    .child(Button::new("settings").icon(IconName::Settings))
                    .child(
                        div()
                            .w(px(32.0))
                            .h(px(32.0))
                            .rounded(px(16.0))
                            .bg(cx.theme().accent)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child("T")
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
                            .on_click(cx.listener(|this: &mut OneHupApp, _, window, cx| {
                                this.add_settings_tab(window, cx);
                            }))
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .w(px(32.0))
                                    .h(px(32.0))
                                    .rounded(px(16.0))
                                    .bg(cx.theme().accent)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child("U") // 用户头像占位符
                            )
                            .child(
                                v_flex()
                                    .child("用户")
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("在线")
                                    )
                            )
                    )
            )
    }




}

impl Render for OneHupApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                    .child(self.render_sidebar(window, cx))
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

