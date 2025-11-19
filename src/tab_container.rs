use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement,
    Render, SharedString, Styled, Window, div, px,
    InteractiveElement, MouseButton,
};
use gpui::prelude::FluentBuilder;
use gpui_component::{
    IconName, Size,
};
use std::any::Any;
use std::sync::Arc;

// ============================================================================
// TabContent Trait - Strategy Pattern Interface
// ============================================================================

/// Trait that defines how tab content should be rendered.
/// Different tab types implement this trait to provide their own rendering logic.
pub trait TabContent: Send + Sync {
    /// Get the tab title
    fn title(&self) -> SharedString;

    /// Get optional icon for the tab
    fn icon(&self) -> Option<IconName> {
        None
    }

    /// Check if tab can be closed
    fn closeable(&self) -> bool {
        true
    }

    /// Render the content of this tab
    fn render_content(&self, window: &mut Window, cx: &mut App) -> AnyElement;

    /// Called when tab becomes active
    fn on_activate(&self, _window: &mut Window, _cx: &mut App) {}

    /// Called when tab becomes inactive
    fn on_deactivate(&self, _window: &mut Window, _cx: &mut App) {}

    /// Get tab content type for identification
    fn content_type(&self) -> TabContentType;

    /// Enable downcasting to concrete types
    fn as_any(&self) -> &dyn Any;
}

/// Type-safe enum for different tab content types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TabContentType {
    SqlEditor,
    TableData(String),    // Table name
    TableForm(String),    // Table name
    QueryResult(String),  // Query ID
    Custom(String),       // Custom type identifier
}

// ============================================================================
// TabItem - Represents a single tab with its content
// ============================================================================

pub struct TabItem {
    id: String,
    content: Arc<dyn TabContent>,
}

impl TabItem {
    pub fn new(id: impl Into<String>, content: impl TabContent + 'static) -> Self {
        Self {
            id: id.into(),
            content: Arc::new(content),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn content(&self) -> &Arc<dyn TabContent> {
        &self.content
    }
}

// ============================================================================
// TabContainer - Main container component
// ============================================================================

pub struct TabContainer {
    tabs: Vec<TabItem>,
    active_index: usize,
    size: Size,
    show_menu: bool,
    dragging_index: Option<usize>,
}

impl TabContainer {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let _ = (window, cx);
        Self {
            tabs: Vec::new(),
            active_index: 0,
            size: Size::Small,
            show_menu: false,
            dragging_index: None,
        }
    }

    /// Add a new tab
    pub fn add_tab(&mut self, tab: TabItem, cx: &mut Context<Self>) {
        self.tabs.push(tab);
        cx.notify();
    }

    /// Add a new tab and activate it
    pub fn add_and_activate_tab(&mut self, tab: TabItem, cx: &mut Context<Self>) {
        self.tabs.push(tab);
        self.active_index = self.tabs.len() - 1;
        cx.notify();
    }

    /// Close a tab by index
    pub fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.tabs.len() && self.tabs[index].content.closeable() {
            self.tabs.remove(index);

            // Adjust active index if needed
            if self.active_index >= self.tabs.len() && !self.tabs.is_empty() {
                self.active_index = self.tabs.len() - 1;
            }

            cx.notify();
        }
    }

    /// Close a tab by ID
    pub fn close_tab_by_id(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(index) = self.tabs.iter().position(|t| t.id() == id) {
            self.close_tab(index, cx);
        }
    }

    /// Set the active tab by index
    pub fn set_active_index(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index < self.tabs.len() {
            // Deactivate old tab
            if let Some(old_tab) = self.tabs.get(self.active_index) {
                old_tab.content.on_deactivate(window, cx);
            }

            self.active_index = index;

            // Activate new tab
            if let Some(new_tab) = self.tabs.get(self.active_index) {
                new_tab.content.on_activate(window, cx);
            }

            cx.notify();
        }
    }

    /// Set the active tab by ID
    pub fn set_active_by_id(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.tabs.iter().position(|t| t.id() == id) {
            self.set_active_index(index, window, cx);
        }
    }

    /// Get the active tab
    pub fn active_tab(&self) -> Option<&TabItem> {
        self.tabs.get(self.active_index)
    }

    /// Find tab by content type
    pub fn find_tab_by_type(&self, content_type: &TabContentType) -> Option<&TabItem> {
        self.tabs
            .iter()
            .find(|t| &t.content.content_type() == content_type)
    }

    /// Check if a tab with the given type exists
    pub fn has_tab_type(&self, content_type: &TabContentType) -> bool {
        self.find_tab_by_type(content_type).is_some()
    }

    /// Activate existing tab or create new one
    pub fn activate_or_create<F>(
        &mut self,
        content_type: &TabContentType,
        create_fn: F,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) where
        F: FnOnce() -> TabItem,
    {
        if let Some(index) = self
            .tabs
            .iter()
            .position(|t| &t.content.content_type() == content_type)
        {
            self.set_active_index(index, window, cx);
        } else {
            let tab = create_fn();
            self.add_and_activate_tab(tab, cx);
        }
    }

    /// Set tab bar size
    pub fn set_size(&mut self, size: Size, cx: &mut Context<Self>) {
        self.size = size;
        cx.notify();
    }

    /// Set whether to show more menu
    pub fn set_show_menu(&mut self, show: bool, cx: &mut Context<Self>) {
        self.show_menu = show;
        cx.notify();
    }

    /// Get all tabs
    pub fn tabs(&self) -> &[TabItem] {
        &self.tabs
    }

    /// Get active index
    pub fn active_index(&self) -> usize {
        self.active_index
    }

    /// Move tab from one position to another
    pub fn move_tab(&mut self, from_index: usize, to_index: usize, cx: &mut Context<Self>) {
        if from_index < self.tabs.len() && to_index < self.tabs.len() && from_index != to_index {
            let tab = self.tabs.remove(from_index);
            self.tabs.insert(to_index, tab);

            // Adjust active index if needed
            if self.active_index == from_index {
                self.active_index = to_index;
            } else if from_index < self.active_index && to_index >= self.active_index {
                self.active_index -= 1;
            } else if from_index > self.active_index && to_index <= self.active_index {
                self.active_index += 1;
            }

            cx.notify();
        }
    }

    /// Start dragging a tab
    pub fn start_drag(&mut self, index: usize, cx: &mut Context<Self>) {
        self.dragging_index = Some(index);
        cx.notify();
    }

    /// End dragging
    pub fn end_drag(&mut self, cx: &mut Context<Self>) {
        self.dragging_index = None;
        cx.notify();
    }

    pub fn render_tab_bar(&self, window: &mut Window, cx: &App) -> impl IntoElement {
        // Custom tab bar with dark background
        div()
            .w_full()
            .h(px(44.0))
            .bg(gpui::rgb(0x2d2d2d)) // 深色背景
            .flex()
            .items_center()
            .px_2()
            .gap_1()
            .children(self.tabs.iter().enumerate().map(|(idx, tab)| {
                let is_active = idx == self.active_index;
                let closeable = tab.content.closeable();
                
                div()
                    .flex()
                    .items_center()
                    .h(px(32.0))
                    .px_3()
                    .rounded(px(6.0))
                    .when(is_active, |div| {
                        div.bg(gpui::rgb(0x4a4a4a)) // 活动标签背景
                    })
                    .when(!is_active, |div| {
                        div.hover(|style| style.bg(gpui::rgb(0x3a3a3a))) // 悬停效果
                    })
                    .cursor_pointer()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                // 图标
                                div()
                                    .when_some(tab.content.icon(), |element, _icon| {
                                        element.child(
                                            div()
                                                .w(px(16.0))
                                                .h(px(16.0))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_color(gpui::white())
                                                .child("📁") // 临时使用 emoji，实际应该使用图标
                                        )
                                    })
                            )
                            .child(
                                // 标签文字
                                div()
                                    .text_sm()
                                    .text_color(gpui::white())
                                    .child(tab.content.title().to_string())
                            )
                            .when(closeable, |element| {
                                element.child(
                                    // 关闭按钮
                                    div()
                                        .ml_2()
                                        .w(px(16.0))
                                        .h(px(16.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(px(2.0))
                                        .cursor_pointer()
                                        .text_color(gpui::rgb(0xaaaaaa))
                                        .hover(|style| {
                                            style
                                                .bg(gpui::rgb(0x5a5a5a))
                                                .text_color(gpui::white())
                                        })
                                        .child("×")
                                )
                            })
                    )
            }))
            .child(
                // 添加新标签按钮
                div()
                    .ml_2()
                    .w(px(32.0))
                    .h(px(32.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.0))
                    .cursor_pointer()
                    .text_color(gpui::rgb(0xaaaaaa))
                    .hover(|style| {
                        style
                            .bg(gpui::rgb(0x3a3a3a))
                            .text_color(gpui::white())
                    })
                    .child("+")
            )
    }

    pub fn render_tab_content(&self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Active tab content
        div()
            .flex_1()
            .w_full()
            .overflow_hidden()
            .when_some(self.active_tab(), |el, tab| {
                el.child(tab.content.render_content(window, cx))
            })
    }

    pub fn render_tab_bar_only(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();

        // 深色标签栏
        div()
            .w_full()
            .h(px(40.0))
            .bg(gpui::rgb(0x2d2d2d))
            .flex()
            .items_center()
            .px_2()
            .gap_1()
            .border_b_1()
            .border_color(gpui::rgb(0x1e1e1e))
            .children(self.tabs.iter().enumerate().map(|(idx, tab)| {
                let is_active = idx == self.active_index;
                let closeable = tab.content.closeable();
                let view_clone = view.clone();
                
                div()
                    .flex()
                    .items_center()
                    .h(px(32.0))
                    .px_3()
                    .rounded(px(6.0))
                    .when(is_active, |div| {
                        div.bg(gpui::rgb(0x4a4a4a))
                    })
                    .when(!is_active, |div| {
                        div.hover(|style| style.bg(gpui::rgb(0x3a3a3a)))
                    })
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, {
                        let view_clone = view_clone.clone();
                        move |_event, window, cx| {
                            view_clone.update(cx, |this, cx| {
                                this.set_active_index(idx, window, cx);
                            });
                        }
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                // 标签文字
                                div()
                                    .text_sm()
                                    .text_color(gpui::white())
                                    .child(tab.content.title().to_string())
                            )
                            .when(closeable, |element| {
                                element.child(
                                    // 关闭按钮
                                    div()
                                        .ml_2()
                                        .w(px(16.0))
                                        .h(px(16.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(px(2.0))
                                        .cursor_pointer()
                                        .text_color(gpui::rgb(0xaaaaaa))
                                        .hover(|style| {
                                            style
                                                .bg(gpui::rgb(0x5a5a5a))
                                                .text_color(gpui::white())
                                        })
                                        .on_mouse_down(MouseButton::Left, {
                                            let view_clone = view_clone.clone();
                                            move |_event, _window, cx| {
                                                view_clone.update(cx, |this, cx| {
                                                    this.close_tab(idx, cx);
                                                });
                                            }
                                        })
                                        .child("×")
                                )
                            })
                    )
            }))
    }
}

impl Render for TabContainer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 只渲染标签栏，内容由父组件处理
        self.render_tab_bar_only(window, cx)
    }
}
