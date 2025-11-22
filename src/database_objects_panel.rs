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
    pub active_tab: Entity<usize>, // 0=Tables, 1=Views, 2=Functions, 3=Procedures
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
    pub fn set_database(&self, database: String, config: db::DbConnectionConfig, cx: &mut App) {
        self.current_database.update(cx, |db, cx| {
            *db = Some(database.clone());
            cx.notify();
        });

        self.status_msg.update(cx, |msg, cx| {
            *msg = format!("Loading objects for {}...", database);
            cx.notify();
        });

        self.load_objects(database, config, cx);
    }

    fn load_objects(&self, database: String, config: db::DbConnectionConfig, cx: &mut App) {
        let global_state = cx.global::<db::GlobalDbState>().clone();
        let tables = self.tables.clone();
        let views = self.views.clone();
        let functions = self.functions.clone();
        let procedures = self.procedures.clone();
        let status_msg = self.status_msg.clone();

        cx.spawn(async move |cx| {
            // Get plugin
            let plugin = match global_state.db_manager.get_plugin(&config.database_type) {
                Ok(p) => p,
                Err(e) => {
                    cx.update(|cx| {
                        status_msg.update(cx, |msg, cx| {
                            *msg = format!("Failed to get plugin: {}", e);
                            cx.notify();
                        });
                    }).ok();
                    return;
                }
            };

            // Get connection
            let conn_arc = match global_state.connection_pool.get_connection(config, &global_state.db_manager).await {
                Ok(c) => c,
                Err(e) => {
                    cx.update(|cx| {
                        status_msg.update(cx, |msg, cx| {
                            *msg = format!("Failed to get connection: {}", e);
                            cx.notify();
                        });
                    }).ok();
                    return;
                }
            };

            let conn = conn_arc.read().await;

            // Load tables
            let tables_list = plugin.list_tables(&**conn, &database).await.unwrap_or_default();
            
            // Load views
            let views_list = plugin.list_views(&**conn, &database).await
                .unwrap_or_default()
                .into_iter()
                .map(|v| v.name)
                .collect::<Vec<_>>();
            
            // Load functions
            let functions_list = plugin.list_functions(&**conn, &database).await
                .unwrap_or_default()
                .into_iter()
                .map(|f| f.name)
                .collect::<Vec<_>>();
            
            // Load procedures
            let procedures_list = plugin.list_procedures(&**conn, &database).await
                .unwrap_or_default()
                .into_iter()
                .map(|p| p.name)
                .collect::<Vec<_>>();

            // Update UI
            cx.update(|cx| {
                tables.update(cx, |t, cx| {
                    *t = tables_list;
                    cx.notify();
                });

                views.update(cx, |v, cx| {
                    *v = views_list;
                    cx.notify();
                });

                functions.update(cx, |f, cx| {
                    *f = functions_list;
                    cx.notify();
                });

                procedures.update(cx, |p, cx| {
                    *p = procedures_list;
                    cx.notify();
                });

                status_msg.update(cx, |msg, cx| {
                    *msg = format!("Loaded objects for {}", database);
                    cx.notify();
                });
            }).ok();
        }).detach();
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
        _cx: &mut Context<Self>,
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
