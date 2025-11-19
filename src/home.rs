use std::any::Any;
use gpui::{div, px, AnyElement, App, Entity, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, Window};
use gpui::prelude::FluentBuilder;
use gpui_component::{h_flex, v_flex, ActiveTheme, IconName};
use crate::database_tab::DatabaseTabContent;
use crate::onehup_app::{ConnectionInfo, ConnectionType};
use crate::tab_container::{TabContainer, TabContent, TabContentType, TabItem};



// 首页内容
pub struct HomeTabContent {
    connections: Vec<ConnectionInfo>,
    tab_container: Entity<TabContainer>,
}

impl HomeTabContent {
    pub fn new(connections: Vec<ConnectionInfo>, tab_container: Entity<TabContainer>) -> Self {
        Self {
            connections,
            tab_container,
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
                        // 在 update 之前创建 DatabaseTabContent
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
            .into_any_element()
    }

    fn content_type(&self) -> TabContentType {
        TabContentType::Custom("home".to_string())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}