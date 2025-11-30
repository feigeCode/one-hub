use one_core::tab_container::TabContent;
use one_core::tab_container::TabContentType;
use std::any::Any;
use gpui::{div, App, Context, Entity, Focusable, FocusHandle, IntoElement, ParentElement, Styled, Window, AppContext, SharedString, AnyElement};
use gpui_component::{v_flex, ActiveTheme, Size};
use crate::object_detail::{ObjectDetailView, SelectedNode};
use db::types::DbNodeType;
use one_core::storage::DbConnectionConfig;

/// Panel that displays database object details based on tree selection
/// This replaces the old tab-based approach with a dynamic detail view
pub struct DatabaseObjectsPanel {
    connection_config: Entity<Option<DbConnectionConfig>>,
    detail_view: Entity<ObjectDetailView>,
    focus_handle: FocusHandle,
    status_msg: Entity<String>,
}

impl DatabaseObjectsPanel {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let connection_config = cx.new(|_| None);
        let detail_view = cx.new(|cx| ObjectDetailView::new(cx));
        let focus_handle = cx.focus_handle();
        let status_msg = cx.new(|_| "Select a database object to view details".to_string());

        Self {
            connection_config,
            detail_view,
            focus_handle,
            status_msg,
        }
    }

    /// Handle node selection event from the tree view
    pub fn handle_node_selected(
        &self,
        node_id: String,
        node_type: DbNodeType,
        config: DbConnectionConfig,
        cx: &mut App,
    ) {
        // Store connection config
        self.connection_config.update(cx, |c, cx| {
            *c = Some(config.clone());
            cx.notify();
        });

        // Parse node and update detail view
        let selected_node = SelectedNode::from_node_id(&node_id, node_type);

        self.detail_view.update(cx, |view, cx| {
            view.set_selected_node(selected_node, config, cx);
        });

        // Update status message
        self.status_msg.update(cx, |msg, cx| {
            *msg = format!("Viewing details for: {}", node_id);
            cx.notify();
        });
    }
}

impl TabContent for DatabaseObjectsPanel {
    fn title(&self) -> SharedString {
        SharedString::from("对象")
    }

    fn closeable(&self) -> bool {
        false
    }
    fn render_content(&self, _window: &mut Window, cx: &mut App) -> AnyElement {
        v_flex()
            .size_full()
            .child(
                // Status bar
                div()
                    .p_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().muted)
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(self.status_msg.read(cx).clone())
                    )
            )
            .child(
                // Detail view
                div()
                    .flex_1()
                    .size_full()
                    .child(self.detail_view.clone())
            ).into_any_element()
    }

    fn content_type(&self) -> TabContentType {
        TabContentType::TableData("Object".to_string())
    }

    fn width_size(&self) -> Option<Size> {
        Some(Size::XSmall)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Focusable for DatabaseObjectsPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Clone for DatabaseObjectsPanel {
    fn clone(&self) -> Self {
        Self {
            connection_config: self.connection_config.clone(),
            detail_view: self.detail_view.clone(),
            focus_handle: self.focus_handle.clone(),
            status_msg: self.status_msg.clone(),
        }
    }
}
