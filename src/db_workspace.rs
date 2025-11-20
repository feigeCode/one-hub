use anyhow::{Context as _, Result};
use gpui::*;
use gpui_component::{
    Root, Sizable, StyledExt,
    button::{Button, ButtonVariants as _},
    dock::{ClosePanel, DockArea, DockAreaState, DockEvent, DockItem, DockPlacement, ToggleZoom},
    menu::DropdownMenu, h_flex, v_flex, ActiveTheme, IconName,
};

use serde::Deserialize;
use std::{ time::Duration, sync::Arc};
use crate::onehup_app::ConnectionInfo;
use crate::tab_container::{TabContainer, TabItem};

#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = story, no_json)]
pub struct AddPanel(DockPlacement);

#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = story, no_json)]
pub struct TogglePanelVisible(SharedString);

actions!(onehub, [ToggleDockToggleButton,About,
        Open,
        Quit,
        CloseWindow,
        ToggleSearch,
        TestAction,
        Tab,
        TabPrev,
        ShowPanelInfo]);
const MAIN_DOCK_AREA: DockAreaTab = DockAreaTab {
    id: "main-dock",
    version: 5,
};

#[cfg(debug_assertions)]
const STATE_FILE: &str = "target/docks.json";
#[cfg(not(debug_assertions))]
const STATE_FILE: &str = "docks.json";


pub struct AppState {
    pub invisible_panels: Entity<Vec<SharedString>>,
}
impl AppState {
    fn init(cx: &mut App) {
        let state = Self {
            invisible_panels: cx.new(|_| Vec::new()),
        };
        cx.set_global::<AppState>(state);
    }

    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }
}
impl Global for AppState {}
pub fn init(cx: &mut App) {
    cx.on_action(|_action: &Open, _cx: &mut App| {});

    cx.bind_keys(vec![
        KeyBinding::new("shift-escape", ToggleZoom, None),
        KeyBinding::new("ctrl-w", ClosePanel, None),
    ]);

    cx.activate(true);
}

pub struct DbWorkspace {
    dock_area: Entity<DockArea>,
    last_layout_state: Option<DockAreaState>,
    toggle_button_visible: bool,
    _save_layout_task: Option<Task<()>>,
    // Database workspace specific fields
    connection_info: Option<ConnectionInfo>,
    db_tree_view: Option<Entity<crate::db_tree_view::DbTreeView>>,
    inner_tab_container: Option<Entity<TabContainer>>,
    status_msg: Entity<String>,
    is_connected: Entity<bool>,
    event_handler: Option<Entity<DatabaseEventHandler>>,
}

// Event handler for database tree view events
struct DatabaseEventHandler {
    inner_tab_container: Entity<TabContainer>,
    _tree_subscription: Option<gpui::Subscription>,
}

impl DatabaseEventHandler {
    fn new(
        db_tree_view: &Entity<crate::db_tree_view::DbTreeView>,
        inner_tab_container: Entity<TabContainer>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        use crate::db_tree_view::DbTreeViewEvent;
        let inner_tab_container_clone = inner_tab_container.clone();
        let tree_subscription = cx.subscribe_in(db_tree_view, window, move |_handler, _tree, event, window, cx| {
            match event {
                DbTreeViewEvent::CreateNewQuery { database } => {
                    use crate::sql_editor_view::SqlEditorTabContent;

                    let tab_count = inner_tab_container_clone.read(cx).tabs().len();
                    let sql_editor_content = SqlEditorTabContent::new_with_database(
                        format!("{} - Query {}", database, tab_count + 1),
                        Some(database.clone()),
                        window,
                        cx,
                    );

                    sql_editor_content.load_databases(window, cx);

                    let tab = TabItem::new(
                        format!("sql-editor-{}-{}", database, tab_count + 1),
                        sql_editor_content,
                    );

                    inner_tab_container_clone.update(cx, |tc, cx| {
                        tc.add_and_activate_tab(tab, cx);
                    });
                }
                DbTreeViewEvent::OpenTableData { database, table } => {
                    use crate::tab_contents::TableDataTabContent;

                    let tab_id = format!("table-data-{}-{}", database, table);

                    inner_tab_container_clone.update(cx, |tc, cx| {
                        if let Some(index) = tc.tabs().iter().position(|t| t.id() == tab_id) {
                            tc.set_active_index(index, window, cx);
                        } else {
                            let tab_title = format!("{}.{}", database, table);
                            let tab = TabItem::new(
                                tab_id.clone(),
                                TableDataTabContent::new(tab_title, window, cx),
                            );
                            tc.add_and_activate_tab(tab, cx);
                        }
                    });
                }
                DbTreeViewEvent::OpenViewData { database, view } => {
                    use crate::tab_contents::TableDataTabContent;

                    let tab_id = format!("view-data-{}-{}", database, view);

                    inner_tab_container_clone.update(cx, |tc, cx| {
                        if let Some(index) = tc.tabs().iter().position(|t| t.id() == tab_id) {
                            tc.set_active_index(index, window, cx);
                        } else {
                            let tab_title = format!("{}.{}", database, view);
                            let tab = TabItem::new(
                                tab_id.clone(),
                                TableDataTabContent::new(tab_title, window, cx),
                            );
                            tc.add_and_activate_tab(tab, cx);
                        }
                    });
                }
                DbTreeViewEvent::OpenTableStructure { database, table } => {
                    use crate::tab_contents::TableStructureTabContent;

                    let tab_id = format!("table-structure-{}-{}", database, table);

                    inner_tab_container_clone.update(cx, |tc, cx| {
                        if let Some(index) = tc.tabs().iter().position(|t| t.id() == tab_id) {
                            tc.set_active_index(index, window, cx);
                        } else {
                            let tab = TabItem::new(
                                tab_id.clone(),
                                TableStructureTabContent::new(
                                    database.clone(),
                                    table.clone(),
                                    window,
                                    cx,
                                ),
                            );
                            tc.add_and_activate_tab(tab, cx);
                        }
                    });
                }
                DbTreeViewEvent::ConnectToConnection { .. } => {
                    // Already connected, ignore
                }
            }
        });

        Self {
            inner_tab_container,
            _tree_subscription: Some(tree_subscription),
        }
    }
}

struct DockAreaTab {
    id: &'static str,
    version: usize,
}

impl DbWorkspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let dock_area =
            cx.new(|cx| DockArea::new(MAIN_DOCK_AREA.id, Some(MAIN_DOCK_AREA.version), window, cx));
        let weak_dock_area = dock_area.downgrade();

        match Self::load_layout(dock_area.clone(), window, cx) {
            Ok(_) => {
                println!("load layout success");
            }
            Err(err) => {
                eprintln!("load layout error: {:?}", err);
                Self::reset_default_layout(weak_dock_area, window, cx);
            }
        };

        cx.subscribe_in(
            &dock_area,
            window,
            |this, dock_area, ev: &DockEvent, window, cx| match ev {
                DockEvent::LayoutChanged => this.save_layout(dock_area, window, cx),
                _ => {}
            },
        )
        .detach();

        cx.on_app_quit({
            let dock_area = dock_area.clone();
            move |_, cx| {
                let state = dock_area.read(cx).dump(cx);
                cx.background_executor().spawn(async move {
                    // Save layout before quitting
                    Self::save_state(&state).unwrap();
                })
            }
        })
        .detach();

        let status_msg = cx.new(|_| "Not connected".to_string());
        let is_connected = cx.new(|_| false);

        Self {
            dock_area,
            last_layout_state: None,
            toggle_button_visible: true,
            _save_layout_task: None,
            connection_info: None,
            db_tree_view: None,
            inner_tab_container: None,
            status_msg,
            is_connected,
            event_handler: None,
        }
    }

    /// Set up database workspace with a connection
    pub fn setup_database_workspace(
        &mut self,
        connection_info: ConnectionInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::storage::StoredConnection;

        // Create stored connection for DbTreeView
        let stored_conn = StoredConnection {
            id: connection_info.id,
            name: connection_info.name.clone(),
            db_type: connection_info.db_type,
            host: connection_info.host.clone(),
            port: connection_info.port,
            username: connection_info.username.clone(),
            password: connection_info.password.clone(),
            database: connection_info.database.clone(),
            created_at: None,
            updated_at: None,
        };

        // Create database tree view
        let db_tree_view = cx.new(|cx| {
            crate::db_tree_view::DbTreeView::new(&vec![stored_conn], window, cx)
        });

        // Create inner tab container with light theme colors
        let inner_tab_container = cx.new(|cx| {
            TabContainer::new(window, cx)
                .with_tab_bar_colors(
                    Some(gpui::rgb(0xf8f9fa).into()),    // Very light gray background
                    Some(gpui::rgb(0xdee2e6).into()),    // Medium gray border
                )
                .with_tab_item_colors(
                    Some(gpui::rgb(0xd0d7de).into()),    // Light blue-gray active tab
                    Some(gpui::rgb(0xe9ecef).into()),    // Very light gray hover
                )
                .with_tab_content_colors(
                    Some(gpui::rgb(0x24292f).into()),    // Dark gray text (near black)
                    Some(gpui::rgb(0x57606a).into()),    // Medium gray close button
                )
        });

        // Create event handler
        let event_handler = cx.new(|cx| {
            DatabaseEventHandler::new(&db_tree_view, inner_tab_container.clone(), window, cx)
        });

        self.connection_info = Some(connection_info.clone());
        self.db_tree_view = Some(db_tree_view.clone());
        self.inner_tab_container = Some(inner_tab_container);
        self.event_handler = Some(event_handler);

        // Add DbTreeView to the left dock
        let weak_dock_area = self.dock_area.downgrade();
        self.dock_area.update(cx, |dock_area, cx| {
            let panel_view: Arc<dyn gpui_component::dock::PanelView> = Arc::new(db_tree_view.clone());
            let dock_item = DockItem::tabs(vec![panel_view], Some(0), &weak_dock_area, window, cx);
            dock_area.set_left_dock(dock_item, Some(px(300.0)), true, window, cx);
        });

        // Start connection
        self.start_connection(connection_info, cx);
    }

    fn start_connection(&self, conn: ConnectionInfo, cx: &mut Context<Self>) {
        let status_msg = self.status_msg.clone();
        let is_connected = self.is_connected.clone();
        let db_tree_view = self.db_tree_view.clone();

        let global_state = cx.global::<db::GlobalDbState>().clone();
        let stored_conn_id = conn.id.unwrap_or(0).to_string();

        status_msg.update(cx, |s, cx| {
            *s = "Connecting...".to_string();
            cx.notify();
        });

        cx.spawn(async move |this, mut cx| {
            let config = db::DbConnectionConfig {
                id: stored_conn_id.clone(),
                database_type: conn.db_type,
                name: conn.name.clone(),
                host: conn.host.clone(),
                port: conn.port,
                username: conn.username.clone(),
                password: conn.password.clone(),
                database: conn.database.clone(),
            };

            let plugin = match global_state.db_manager.get_plugin(&config.database_type) {
                Ok(p) => p,
                Err(e) => {
                    cx.update(|cx| {
                        status_msg.update(cx, |s, cx| {
                            *s = format!("Failed to get plugin: {}", e);
                            cx.notify();
                        });
                    })
                        .ok();
                    return;
                }
            };

            match plugin.create_connection(config.clone()).await {
                Ok(connection) => {
                    global_state
                        .connection_pool
                        .add_connection(stored_conn_id.clone(), connection, config.clone())
                        .await;

                    global_state
                        .connection_pool
                        .set_current_connection(stored_conn_id.clone())
                        .await;

                    if let Some(db) = config.database.as_ref() {
                        global_state
                            .connection_pool
                            .set_current_database(Some(db.clone()))
                            .await;
                    }

                    cx.update(|cx| {
                        is_connected.update(cx, |flag, cx| {
                            *flag = true;
                            cx.notify();
                        });

                        status_msg.update(cx, |s, cx| {
                            *s = format!("Connected to {}", config.name);
                            cx.notify();
                        });

                        if let Some(tree_view) = db_tree_view {
                            tree_view.update(cx, |tree, cx| {
                                tree.set_connection_name(config.name.clone());
                                tree.update_connection_node(&stored_conn_id, cx);
                            });
                        }
                    })
                        .ok();
                }
                Err(e) => {
                    cx.update(|cx| {
                        status_msg.update(cx, |s, cx| {
                            *s = format!("Connection failed: {}", e);
                            cx.notify();
                        });
                    })
                        .ok();
                }
            }
        })
            .detach();
    }

    fn save_layout(
        &mut self,
        dock_area: &Entity<DockArea>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let dock_area = dock_area.clone();
        self._save_layout_task = Some(cx.spawn_in(window, async move |story, window| {
            Timer::after(Duration::from_secs(10)).await;

            _ = story.update_in(window, move |this, _, cx| {
                let dock_area = dock_area.read(cx);
                let state = dock_area.dump(cx);

                let last_layout_state = this.last_layout_state.clone();
                if Some(&state) == last_layout_state.as_ref() {
                    return;
                }

                Self::save_state(&state).unwrap();
                this.last_layout_state = Some(state);
            });
        }));
    }

    fn save_state(state: &DockAreaState) -> Result<()> {
        println!("Save layout...");
        let json = serde_json::to_string_pretty(state)?;
        std::fs::write(STATE_FILE, json)?;
        Ok(())
    }

    fn load_layout(
        dock_area: Entity<DockArea>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        let json = std::fs::read_to_string(STATE_FILE)?;
        let state = serde_json::from_str::<DockAreaState>(&json)?;

        // Check if the saved layout version is different from the current version
        // Notify the user and ask if they want to reset the layout to default.
        if state.version != Some(MAIN_DOCK_AREA.version) {
            let answer = window.prompt(
                PromptLevel::Info,
                "The default main layout has been updated.\n\
                Do you want to reset the layout to default?",
                None,
                &["Yes", "No"],
                cx,
            );

            let weak_dock_area = dock_area.downgrade();
            cx.spawn_in(window, async move |this, window| {
                if answer.await == Ok(0) {
                    _ = this.update_in(window, |_, window, cx| {
                        Self::reset_default_layout(weak_dock_area, window, cx);
                    });
                }
            })
            .detach();
        }

        dock_area.update(cx, |dock_area, cx| {
            dock_area.load(state, window, cx).context("load layout")?;
            dock_area.set_dock_collapsible(
                Edges {
                    left: true,
                    bottom: true,
                    right: true,
                    ..Default::default()
                },
                window,
                cx,
            );

            Ok::<(), anyhow::Error>(())
        })
    }

    fn reset_default_layout(dock_area: WeakEntity<DockArea>, window: &mut Window, cx: &mut App) {

        _ = dock_area.update(cx, |view, cx| {
            view.set_version(MAIN_DOCK_AREA.version, window, cx);
            // view.set_center(dock_item, window, cx);
            // view.set_left_dock(left_panels, Some(px(350.)), true, window, cx);
            // view.set_bottom_dock(bottom_panels, Some(px(200.)), true, window, cx);
            // view.set_right_dock(right_panels, Some(px(320.)), true, window, cx);
            Self::save_state(&view.dump(cx)).unwrap();
        });
    }

    fn on_action_add_panel(
        &mut self,
        action: &AddPanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {


        self.dock_area.update(cx, |dock_area, cx| {
            // dock_area.add_panel(panel, action.0, None, window, cx);
        });
    }

    fn on_action_toggle_panel_visible(
        &mut self,
        action: &TogglePanelVisible,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let panel_name = action.0.clone();
        let invisible_panels = AppState::global(cx).invisible_panels.clone();
        invisible_panels.update(cx, |names, cx| {
            if names.contains(&panel_name) {
                names.retain(|id| id != &panel_name);
            } else {
                names.push(panel_name);
            }
            cx.notify();
        });
        cx.notify();
    }

    fn on_action_toggle_dock_toggle_button(
        &mut self,
        _: &ToggleDockToggleButton,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_button_visible = !self.toggle_button_visible;

        self.dock_area.update(cx, |dock_area, cx| {
            dock_area.set_toggle_button_visible(self.toggle_button_visible, cx);
        });
    }
}


impl Render for DbWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        let is_connected_flag = *self.is_connected.read(cx);

        div()
            .id("db-workspace")
            .on_action(cx.listener(Self::on_action_add_panel))
            .on_action(cx.listener(Self::on_action_toggle_panel_visible))
            .on_action(cx.listener(Self::on_action_toggle_dock_toggle_button))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .child(if is_connected_flag {
                // Connected - show dock area
                self.dock_area.clone().into_any_element()
            } else {
                // Not connected - show connection status
                self.render_connection_status(cx)
            })
            .children(sheet_layer)
            .children(dialog_layer)
            .children(notification_layer)
    }
}

impl DbWorkspace {
    fn render_connection_status(&self, cx: &mut Context<Self>) -> AnyElement {
        let status_text = self.status_msg.read(cx).clone();
        let is_error = status_text.contains("failed") || status_text.contains("Failed");

        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_6()
            .child(
                div()
                    .w(px(64.0))
                    .h(px(64.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(if is_error {
                        div()
                            .w(px(48.0))
                            .h(px(48.0))
                            .rounded(px(24.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(Hsla::red())
                            .text_color(gpui::white())
                            .text_2xl()
                            .child("✕")
                    } else {
                        div()
                            .w(px(48.0))
                            .h(px(48.0))
                            .rounded(px(24.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .border_4()
                            .border_color(cx.theme().accent)
                            .text_2xl()
                            .text_color(cx.theme().accent)
                            .child("⟳")
                    })
            )
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .child(if let Some(ref info) = self.connection_info {
                        format!("Database Connection: {}", info.name)
                    } else {
                        "Database Connection".to_string()
                    })
            )
            .child(if let Some(ref info) = self.connection_info {
                v_flex()
                    .gap_2()
                    .p_4()
                    .bg(cx.theme().muted)
                    .rounded(px(8.0))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Host:"))
                            .child(info.host.clone())
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Port:"))
                            .child(format!("{}", info.port))
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Username:"))
                            .child(info.username.clone())
                    )
                    .child(if let Some(ref db) = info.database {
                        h_flex()
                            .gap_2()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Database:"))
                            .child(db.clone())
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    })
                    .into_any_element()
            } else {
                div().into_any_element()
            })
            .child(
                div()
                    .text_lg()
                    .text_color(if is_error { Hsla::red() } else { cx.theme().accent })
                    .child(status_text)
            )
            .into_any_element()
    }
}
