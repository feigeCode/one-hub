use std::{any::Any, sync::Arc};

use gpui::prelude::FluentBuilder;
use gpui::StatefulInteractiveElement as _;
use gpui::{div, px, AnyElement, App, AppContext, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Render, ScrollHandle, SharedString, Styled, Window};
use gpui_component::{h_flex, v_flex, ActiveTheme, IconName, Size, StyledExt};
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
// DragTab - Visual representation during drag
// ============================================================================

/// Represents a tab being dragged, used for visual feedback
#[derive(Clone)]
pub struct DragTab {
    pub tab_index: usize,
    pub title: SharedString,
}

impl DragTab {
    pub fn new(tab_index: usize, title: SharedString) -> Self {
        Self {
            tab_index,
            title,
        }
    }
}

impl Render for DragTab {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("drag-tab")
            .cursor_grabbing()
            .py_1()
            .px_3()
            .min_w(px(80.0))
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .border_1()
            .border_color(cx.theme().border)
            .rounded(px(6.0))
            .text_color(cx.theme().tab_foreground)
            .bg(cx.theme().tab_active)
            .opacity(0.85)
            .shadow_md()
            .text_sm()
            .child(self.title.clone())
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
    /// Optional background color for the tab bar (defaults to dark theme)
    tab_bar_bg_color: Option<gpui::Hsla>,
    /// Optional border color for the tab bar (defaults to dark theme)
    tab_bar_border_color: Option<gpui::Hsla>,
    /// Optional background color for active tab (defaults to dark theme)
    active_tab_bg_color: Option<gpui::Hsla>,
    /// Optional background color for inactive tab hover state (defaults to dark theme)
    inactive_tab_hover_color: Option<gpui::Hsla>,
    /// Optional text color for tabs (defaults to white)
    tab_text_color: Option<gpui::Hsla>,
    /// Optional close button color (defaults to gray)
    tab_close_button_color: Option<gpui::Hsla>,
    /// Optional left padding for macOS traffic lights (defaults to 0)
    left_padding: Option<gpui::Pixels>,
    /// Optional top padding for vertical centering (defaults to 0)
    top_padding: Option<gpui::Pixels>,
    /// Maximum number of visible tabs before showing overflow menu
    max_visible_tabs: Option<usize>,
    /// Whether to show overflow dropdown menu
    show_overflow_menu: bool,
    tab_bar_scroll_handle: ScrollHandle,
}

impl TabContainer {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let _ = (window, cx);
        Self {
            tabs: Vec::new(),
            active_index: 0,
            size: Size::Small,
            show_menu: false,
            tab_bar_bg_color: None,
            tab_bar_border_color: None,
            active_tab_bg_color: None,
            inactive_tab_hover_color: None,
            tab_text_color: None,
            tab_close_button_color: None,
            left_padding: None,
            top_padding: None,
            max_visible_tabs: None,
            show_overflow_menu: false,
            tab_bar_scroll_handle: ScrollHandle::new(),
        }
    }

    /// Set custom tab bar colors (background and border)
    pub fn with_tab_bar_colors(
        mut self,
        bg_color: impl Into<Option<gpui::Hsla>>,
        border_color: impl Into<Option<gpui::Hsla>>,
    ) -> Self {
        self.tab_bar_bg_color = bg_color.into();
        self.tab_bar_border_color = border_color.into();
        self
    }

    /// Set custom tab item colors (active and hover)
    pub fn with_tab_item_colors(
        mut self,
        active_color: impl Into<Option<gpui::Hsla>>,
        hover_color: impl Into<Option<gpui::Hsla>>,
    ) -> Self {
        self.active_tab_bg_color = active_color.into();
        self.inactive_tab_hover_color = hover_color.into();
        self
    }

    /// Set custom tab text and close button colors
    pub fn with_tab_content_colors(
        mut self,
        text_color: impl Into<Option<gpui::Hsla>>,
        close_button_color: impl Into<Option<gpui::Hsla>>,
    ) -> Self {
        self.tab_text_color = text_color.into();
        self.tab_close_button_color = close_button_color.into();
        self
    }

    /// Set left padding for macOS traffic lights
    ///
    /// Use this to reserve space for the red/yellow/green buttons on macOS.
    /// Common values: px(80.0) for standard macOS window controls.
    ///
    /// # Example
    /// ```
    /// TabContainer::new(window, cx)
    ///     .with_left_padding(px(80.0))
    /// ```
    pub fn with_left_padding(mut self, padding: gpui::Pixels) -> Self {
        self.left_padding = Some(padding);
        self
    }

    /// Set top padding for vertical centering
    ///
    /// Use this to vertically center content when using custom window controls.
    /// Common values: px(4.0) for macOS traffic lights.
    ///
    /// # Example
    /// ```
    /// TabContainer::new(window, cx)
    ///     .with_top_padding(px(4.0))
    /// ```
    pub fn with_top_padding(mut self, padding: gpui::Pixels) -> Self {
        self.top_padding = Some(padding);
        self
    }

    /// Set maximum number of visible tabs before showing overflow menu
    ///
    /// When the number of tabs exceeds this value, remaining tabs will be accessible
    /// through a dropdown menu.
    ///
    /// # Example
    /// ```
    /// TabContainer::new(window, cx)
    ///     .with_max_visible_tabs(10)
    /// ```
    pub fn with_max_visible_tabs(mut self, max: usize) -> Self {
        self.max_visible_tabs = Some(max);
        self
    }

    /// Set tab bar background color
    pub fn set_tab_bar_bg_color(&mut self, color: impl Into<Option<gpui::Hsla>>, cx: &mut Context<Self>) {
        self.tab_bar_bg_color = color.into();
        cx.notify();
    }

    /// Set tab bar border color
    pub fn set_tab_bar_border_color(&mut self, color: impl Into<Option<gpui::Hsla>>, cx: &mut Context<Self>) {
        self.tab_bar_border_color = color.into();
        cx.notify();
    }

    /// Set active tab background color
    pub fn set_active_tab_bg_color(&mut self, color: impl Into<Option<gpui::Hsla>>, cx: &mut Context<Self>) {
        self.active_tab_bg_color = color.into();
        cx.notify();
    }

    /// Set inactive tab hover color
    pub fn set_inactive_tab_hover_color(&mut self, color: impl Into<Option<gpui::Hsla>>, cx: &mut Context<Self>) {
        self.inactive_tab_hover_color = color.into();
        cx.notify();
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

            self.tab_bar_scroll_handle.scroll_to_item(index);

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

    /// Toggle overflow menu visibility
    pub fn toggle_overflow_menu(&mut self, cx: &mut Context<Self>) {
        self.show_overflow_menu = !self.show_overflow_menu;
        cx.notify();
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

    /// Render overflow menu with hidden tabs
    fn render_overflow_menu(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let text_color = self.tab_text_color.unwrap_or_else(|| gpui::white().into());
        let hover_tab_color = self.inactive_tab_hover_color.unwrap_or_else(|| gpui::rgb(0x3a3a3a).into());
        let active_tab_color = self.active_tab_bg_color.unwrap_or_else(|| gpui::rgb(0x4a4a4a).into());
        let border_color = self.tab_bar_border_color.unwrap_or_else(|| gpui::rgb(0x1e1e1e).into());

        // 计算溢出标签
        let overflow_tabs: Vec<(usize, String, bool, bool)> = if let Some(max_visible) = self.max_visible_tabs {
            self.tabs
                .iter()
                .enumerate()
                .skip(max_visible)
                .map(|(idx, tab)| (idx, tab.content.title().to_string(), idx == self.active_index, tab.content.closeable()))
                .collect()
        } else {
            Vec::new()
        };

        div()
            .absolute()
            .top(px(40.0))
            .right(px(8.0))
            .bg(gpui::rgb(0x2d2d2d))
            .border_1()
            .border_color(border_color)
            .rounded(px(6.0))
            .shadow_lg()
            .min_w(px(200.0))
            .max_h(px(400.0))
            .overflow_hidden()
            .children(overflow_tabs.into_iter().map(|(idx, title, is_active, closeable)| {
                let view_clone = view.clone();

                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .w_full()
                    .px_3()
                    .py_2()
                    .cursor_pointer()
                    .when(is_active, |div| div.bg(active_tab_color))
                    .when(!is_active, |div| div.hover(move |style| style.bg(hover_tab_color)))
                    .on_mouse_down(MouseButton::Left, {
                        let view_clone = view_clone.clone();
                        move |_event, window, cx| {
                            view_clone.update(cx, |this, cx| {
                                this.set_active_index(idx, window, cx);
                                this.show_overflow_menu = false;
                                cx.notify();
                            });
                        }
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .flex_1()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(text_color)
                                    .child(title)
                            )
                    )
                    .when(closeable, |el| {
                        let view_clone = view_clone.clone();
                        el.child(
                            div()
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
                                        .text_color(text_color)
                                })
                                .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                    view_clone.update(cx, |this, cx| {
                                        this.close_tab(idx, cx);
                                    });
                                })
                                .child("×")
                        )
                    })
            }))
    }

    pub fn render_tab_bar(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();

        // 使用自定义颜色或默认深色标签栏
        let bg_color = self.tab_bar_bg_color.unwrap_or_else(|| gpui::rgb(0x2d2d2d).into());
        let border_color = self.tab_bar_border_color.unwrap_or_else(|| gpui::rgb(0x1e1e1e).into());
        let active_tab_color = self.active_tab_bg_color.unwrap_or_else(|| gpui::rgb(0x4a4a4a).into());
        let hover_tab_color = self.inactive_tab_hover_color.unwrap_or_else(|| gpui::rgb(0x3a3a3a).into());
        let text_color = self.tab_text_color.unwrap_or_else(|| gpui::white().into());
        let close_btn_color = self.tab_close_button_color.unwrap_or_else(|| gpui::rgb(0xaaaaaa).into());
        let drag_border_color = cx.theme().drag_border;

        let active_index = self.active_index;

        h_flex()
            .w_full()
            .h(px(40.0))
            .bg(bg_color)
            .items_center()
            .border_b_1()
            .border_color(border_color)
            .child(
                // 标签滚动容器 - 使用 scrollable 实现水平滚动
                h_flex()
                    .id("tabs")
                    .flex_1()
                    .overflow_x_scroll()
                    .pl(self.left_padding.unwrap_or(px(8.0)))
                    .when_some(self.top_padding, |div, padding| div.pt(padding))
                    .pr_2()
                    .gap_1()
                    .track_scroll(&self.tab_bar_scroll_handle)
                    .children(self.tabs.iter().enumerate().map(|(idx, tab)| {
                        let title = tab.content.title();
                        let closeable = tab.content.closeable();
                        let is_active = idx == active_index;
                        let view_clone = view.clone();
                        let title_clone = title.clone();

                        div()
                            .id(idx)
                            .flex()
                            .flex_shrink_0()
                            .flex_wrap()
                            .overflow_hidden()
                            .items_center()
                            .h(px(32.0))
                            .min_w(px(120.0))
                            .max_w(px(200.0))
                            .px_3()
                            .rounded(px(6.0))
                            .cursor_grab()
                            .when(is_active, |el| el.bg(active_tab_color))
                            .when(!is_active, |el| el.hover(move |style| style.bg(hover_tab_color)))
                            // 使用 GPUI 原生拖放 API
                            .on_drag(
                                DragTab::new(idx, title.clone()),
                                |drag, _, _, cx| {
                                    cx.stop_propagation();
                                    cx.new(|_| drag.clone())
                                },
                            )
                            // 拖动经过时的样式
                            .drag_over::<DragTab>(move |el, _, _, _cx| {
                                el.border_l_2()
                                    .border_color(drag_border_color)
                            })
                            // 放下事件
                            .on_drop(cx.listener(move |this, drag: &DragTab, window, cx| {
                                let from_idx = drag.tab_index;
                                let to_idx = idx;
                                if from_idx != to_idx {
                                    this.move_tab(from_idx, to_idx, cx);
                                }
                                this.set_active_index(to_idx, window, cx);
                            }))
                            // 点击激活
                            .on_click(cx.listener(move |this, _event, window, cx| {
                                this.set_active_index(idx, window, cx);
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        // 标签文字
                                        div()
                                            .text_sm()
                                            .text_color(text_color)
                                            .child(title_clone.to_string())
                                    )
                                    .when(closeable, |element| {
                                        let view_clone = view_clone.clone();
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
                                                .text_color(close_btn_color)
                                                .hover(|style| {
                                                    style
                                                        .bg(gpui::rgb(0x5a5a5a))
                                                        .text_color(text_color)
                                                })
                                                .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                                    view_clone.update(cx, |this, cx| {
                                                        this.close_tab(idx, cx);
                                                    });
                                                })
                                                .child("×")
                                    )
                                })
                        )
                    }))
            )
    }
}

impl Render for TabContainer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let show_overflow_menu = self.show_overflow_menu && self.max_visible_tabs.is_some();

        // 渲染标签栏和内容
        div()
            .relative()
            .size_full()
            .child(
                v_flex()
                    .size_full()
                    .child(
                        // Tab bar
                        self.render_tab_bar(window, cx)
                    )
                    .child(
                        // Tab content
                        self.render_tab_content(window, cx)
                    )
            )
            .when(show_overflow_menu, |el| {
                el.child(
                    // Overflow menu overlay
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .w_full()
                        .h_full()
                        .on_mouse_down(MouseButton::Left, {
                            let view = cx.entity();
                            move |_event, _window, cx| {
                                view.update(cx, |this, cx| {
                                    this.show_overflow_menu = false;
                                    cx.notify();
                                });
                            }
                        })
                        .child(self.render_overflow_menu(cx))
                )
            })
    }
}
