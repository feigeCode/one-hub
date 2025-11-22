use crate::home::HomeTabContent;
use crate::tab_container::{TabContainer, TabItem};
use crate::themes;
use gpui::{div, px, App, AppContext, Context, Entity, IntoElement, KeyBinding, ParentElement, Render, Styled, Window};
use gpui_component::dock::{ClosePanel, ToggleZoom};
use gpui_component::ActiveTheme;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use crate::connection_store::ConnectionStore;

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

pub struct OneHupApp {
    tab_container: Entity<TabContainer>,
}

impl OneHupApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 从存储加载连接
        let connection_store = ConnectionStore::new().expect("Failed to create connection store");
        let stored_connections = connection_store.load_connections().unwrap_or_else(|_| vec![]);

        // 创建标签容器，根据平台设置 padding
        let tab_container = cx.new(|cx| {
            let mut container = TabContainer::new(window, cx);
            
            // macOS: 为红黄绿按钮留出空间并垂直居中
            #[cfg(target_os = "macos")]
            {
                container = container
                    .with_left_padding(px(80.0))
                    .with_top_padding(px(4.0));
            }
            
            container
        });

        // 添加主页标签
        tab_container.update(cx, |tc, cx| {
            let home_tab = TabItem::new("home", HomeTabContent::new(stored_connections, tab_container.clone(), window, cx));
            tc.add_and_activate_tab(home_tab, cx);
        });

        Self {
            tab_container,
        }
    }
}

impl Render for OneHupApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(cx.theme().background)
            .child(self.tab_container.clone())
    }
}
