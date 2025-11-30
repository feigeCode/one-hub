use crate::home::HomeTabContent;
use core::tab_container::{TabContainer, TabItem};
use gpui::{div, px, App, AppContext, Context, Entity, IntoElement, KeyBinding, ParentElement, Render, Styled, Window};
use gpui_component::dock::{ClosePanel, ToggleZoom};
use gpui_component::{ActiveTheme, Root};
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
    core::init(cx);
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
        // 创建标签容器，根据平台设置 padding
        let tab_container = cx.new(|cx| {
            let mut container = TabContainer::new(window, cx)
                .with_inactive_tab_bg_color(Some(gpui::rgb(0x3a3a3a).into()))
                .with_tab_icon_color(Some(gpui::rgb(0xffffff).into()));
            
            // macOS: 为红黄绿按钮留出空间并垂直居中
            #[cfg(target_os = "macos")]
            {
                container = container
                    .with_left_padding(px(80.0))
                    .with_top_padding(px(4.0))
            }
            
            container
        });

        // 添加主页标签
        tab_container.update(cx, |tc, cx| {
            let home_tab = TabItem::new("home", HomeTabContent::new(tab_container.clone(), window, cx));
            tc.add_and_activate_tab(home_tab, cx);
        });

        Self {
            tab_container,
        }
    }
}

impl Render for OneHupApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);
        div()
            .size_full()
            .bg(cx.theme().background)
            .child(self.tab_container.clone())
            .children(sheet_layer)
            .children(dialog_layer)
            .children(notification_layer)
    }
}
