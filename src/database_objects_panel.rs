use gpui::{
    div, AnyElement, App, AppContext, Context, Entity, EventEmitter, Focusable, FocusHandle,
    IntoElement, ParentElement, Render, SharedString, Styled, WeakEntity, Window,
};
use gpui_component::{
    h_flex, v_flex, ActiveTheme, IconName, Sizable, Size,
    button::{Button, ButtonVariants},
    dock::{Panel, PanelControl, PanelEvent, PanelState, TabPanel, TitleStyle},
    list::ListItem,
    menu::PopupMenu,
    input::{Input, InputState},
};

/// Panel that displays database objects (tables, views, functions, etc.) for the current database
pub struct DatabaseObjectsPanel {
    current_database: Entity<Option<String>>,
    tables: Entity<Vec<String>>,
    views: Entity<Vec<String>>,
    functions: Entity<Vec<String>>,
    procedures: Entity<Vec<String>>,
    active_tab: Entity<usize>, // 0=Tables, 1=Views, 2=Functions, 3=Procedures
    search_input: Entity<InputState>,
    focus_handle: FocusHandle,
    status_msg: Entity<String>,
}

impl DatabaseObjectsPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let current_database = cx.new(|_| None);
        let tables = cx.new(|_| Vec::new());
        let views = cx.new(|_| Vec::new());
        let functions = cx.new(|_| Vec::new());
        let procedures = cx.new(|_| Vec::new());
        let active_tab = cx.new(|_| 0);
        let focus_handle = cx.focus_handle();
        let status_msg = cx.new(|_| "Select a database to view objects".to_string());
        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Search objects...")
        });

        Self {
            current_database,
            tables,
            views,
            functions,
            procedures,
            active_tab,
            search_input,
            focus_handle,
            status_msg,
        }
    }

    /// Set the current database and load its objects
    pub fn set_database(&self, database: String, cx: &mut App) {
        self.current_database.update(cx, |db, cx| {
            *db = Some(database.clone());
            cx.notify();
        });

        self.status_msg.update(cx, |msg, cx| {
            *msg = format!("Loading objects for {}...", database);
            cx.notify();
        });

        self.load_objects(database, cx);
    }

    fn load_objects(&self, database: String, cx: &mut App) {
        let global_state = cx.global::<db::GlobalDbState>().clone();
        let tables = self.tables.clone();
        let views = self.views.clone();
        let functions = self.functions.clone();
        let procedures = self.procedures.clone();
        let status_msg = self.status_msg.clone();

        cx.spawn(async move |cx| {
            // Get current connection and config
            let conn_arc = match global_state.connection_pool.get_current_connection().await {
                Some(c) => c,
                None => {
                    cx.update(|cx| {
                        status_msg.update(cx, |msg, cx| {
                            *msg = "No active connection".to_string();
                            cx.notify();
                        });
                    })
                    .ok();
                    return;
                }
            };

            let config = match global_state.connection_pool.get_current_connection_config().await {
                Some(c) => c,
                None => {
                    cx.update(|cx| {
                        status_msg.update(cx, |msg, cx| {
                            *msg = "No connection config".to_string();
                            cx.notify();
                        });
                    })
                    .ok();
                    return;
                }
            };

            // Get plugin
            let plugin = match global_state.db_manager.get_plugin(&config.database_type) {
                Ok(p) => p,
                Err(e) => {
                    cx.update(|cx| {
                        status_msg.update(cx, |msg, cx| {
                            *msg = format!("Failed to get plugin: {}", e);
                            cx.notify();
                        });
                    })
                    .ok();
                    return;
                }
            };

            // Load objects
            let conn = conn_arc.read().await;

            let tables_result = plugin.list_tables(&**conn, &database).await;
            let views_result = plugin.list_views(&**conn, &database).await;
            let functions_result = plugin.list_functions(&**conn, &database).await;
            let procedures_result = plugin.list_procedures(&**conn, &database).await;

            cx.update(|cx| {
                if let Ok(table_list) = tables_result {
                    tables.update(cx, |t, cx| {
                        *t = table_list;
                        cx.notify();
                    });
                }

                if let Ok(view_list) = views_result {
                    views.update(cx, |v, cx| {
                        *v = view_list.into_iter().map(|vi| vi.name).collect();
                        cx.notify();
                    });
                }

                if let Ok(func_list) = functions_result {
                    functions.update(cx, |f, cx| {
                        *f = func_list.into_iter().map(|fi| fi.name).collect();
                        cx.notify();
                    });
                }

                if let Ok(proc_list) = procedures_result {
                    procedures.update(cx, |p, cx| {
                        *p = proc_list.into_iter().map(|pi| pi.name).collect();
                        cx.notify();
                    });
                }

                status_msg.update(cx, |msg, cx| {
                    *msg = format!("Loaded objects for {}", database);
                    cx.notify();
                });
            })
            .ok();
        })
        .detach();
    }

    fn render_tab_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let active_idx = *self.active_tab.read(cx);
        let tables_count = self.tables.read(cx).len();
        let views_count = self.views.read(cx).len();
        let functions_count = self.functions.read(cx).len();
        let procedures_count = self.procedures.read(cx).len();

        h_flex()
            .gap_1()
            .p_1()
            .bg(cx.theme().muted)
            .border_b_1()
            .border_color(cx.theme().border)
            .child(self.render_tab_button("Tables", 0, tables_count, active_idx, cx))
            .child(self.render_tab_button("Views", 1, views_count, active_idx, cx))
            .child(self.render_tab_button("Functions", 2, functions_count, active_idx, cx))
            .child(self.render_tab_button("Procedures", 3, procedures_count, active_idx, cx))
    }

    fn render_tab_button(
        &self,
        label: &str,
        index: usize,
        count: usize,
        active_idx: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_active = index == active_idx;
        let active_tab = self.active_tab.clone();

        let mut btn = Button::new(("tab", index))
            .with_size(Size::Small)
            .label(format!("{} ({})", label, count));
        
        if is_active {
            btn = btn.primary();
        } else {
            btn = btn.ghost();
        }
        
        btn.on_click(move |_, _, cx| {
            active_tab.update(cx, |tab, cx| {
                *tab = index;
                cx.notify();
            });
        })
    }

    fn render_object_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let active_idx = *self.active_tab.read(cx);
        let current_db = self.current_database.read(cx).clone();
        let search_text = self.search_input.read(cx).text().to_string().to_lowercase();

        let mut objects = match active_idx {
            0 => self.tables.read(cx).clone(),
            1 => self.views.read(cx).clone(),
            2 => self.functions.read(cx).clone(),
            3 => self.procedures.read(cx).clone(),
            _ => Vec::new(),
        };

        // Filter objects by search text
        if !search_text.is_empty() {
            objects.retain(|obj| obj.to_lowercase().contains(&search_text));
        }

        if objects.is_empty() {
            let message = if !search_text.is_empty() {
                "No matching objects found"
            } else {
                "No objects found"
            };
            
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child(message),
                )
                .into_any_element();
        }

        v_flex()
            .size_full()
            .overflow_hidden()
            .children(objects.iter().enumerate().map(|(idx, obj)| {
                let obj_name = obj.clone();
                let db_name = current_db.clone();

                ListItem::new(idx)
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .w_full()
                            .child(IconName::Folder)
                            .child(div().flex_1().child(obj_name.clone())),
                    )
                    .on_click(move |_, _, _| {
                        // TODO: Emit event to open table/view/function
                        eprintln!("Clicked on {} in {:?}", obj_name, db_name);
                    })
            }))
            .into_any_element()
    }
}

impl EventEmitter<PanelEvent> for DatabaseObjectsPanel {}

impl Render for DatabaseObjectsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(self.render_tab_buttons(cx))
            .child(
                // Search input box
                div()
                    .p_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(Input::new(&self.search_input).w_full())
            )
            .child(self.render_object_list(cx))
    }
}

impl Focusable for DatabaseObjectsPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for DatabaseObjectsPanel {
    fn panel_name(&self) -> &'static str {
        "DatabaseObjects"
    }

    fn tab_name(&self, cx: &App) -> Option<SharedString> {
        if let Some(db) = self.current_database.read(cx).as_ref() {
            Some(format!("Objects - {}", db).into())
        } else {
            Some("Objects".into())
        }
    }

    fn title(&self, _window: &Window, cx: &App) -> AnyElement {
        h_flex()
            .items_center()
            .gap_2()
            .child(IconName::Folder)
            .child(if let Some(db) = self.current_database.read(cx).as_ref() {
                format!("Objects - {}", db)
            } else {
                "Objects".to_string()
            })
            .into_any_element()
    }

    fn title_style(&self, _cx: &App) -> Option<TitleStyle> {
        None
    }

    fn title_suffix(&self, _window: &mut Window, _cx: &mut App) -> Option<AnyElement> {
        None
    }

    fn closable(&self, _cx: &App) -> bool {
        false
    }

    fn zoomable(&self, _cx: &App) -> Option<PanelControl> {
        None
    }

    fn visible(&self, _cx: &App) -> bool {
        true
    }

    fn set_active(&mut self, _active: bool, _window: &mut Window, _cx: &mut App) {}

    fn set_zoomed(&mut self, _zoomed: bool, _window: &mut Window, _cx: &mut App) {}

    fn on_added_to(&mut self, _tab_panel: WeakEntity<TabPanel>, _window: &mut Window, _cx: &mut App) {}

    fn on_removed(&mut self, _window: &mut Window, _cx: &mut App) {}

    fn dropdown_menu(&self, this: PopupMenu, _window: &Window, _cx: &App) -> PopupMenu {
        this
    }

    fn toolbar_buttons(&self, _window: &mut Window, _cx: &mut App) -> Option<Vec<Button>> {
        None
    }

    fn dump(&self, _cx: &App) -> PanelState {
        PanelState::new(self)
    }

    fn inner_padding(&self, _cx: &App) -> bool {
        false
    }
}
