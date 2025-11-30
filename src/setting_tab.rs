use std::any::Any;
use gpui::{div, AnyElement, App, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window};
use gpui_component::{v_flex, ActiveTheme, IconName};
use one_core::tab_container::{TabContent, TabContentType};

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