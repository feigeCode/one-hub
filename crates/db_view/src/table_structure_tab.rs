use std::any::Any;
use std::sync::Arc;

use gpui::{
    div, AnyElement, App, AppContext, ClickEvent, Context, Entity, Focusable, FocusHandle,
    IntoElement, ParentElement, Render, SharedString, Styled, Window, InteractiveElement,
    StatefulInteractiveElement,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputState},
    v_flex, ActiveTheme as _, IconName, Sizable as _, Size, StyledExt as _,
};

use db::{ColumnInfo, DbConnectionConfig, GlobalDbState};
use core::tab_container::{TabContent, TabContentType};

// ============================================================================
// Table Structure Tab Content - Edit table structure
// ============================================================================

#[derive(Clone, Debug)]
struct FieldRow {
    id: usize,
    name_input: Entity<InputState>,
    type_input: Entity<InputState>,
    nullable: Entity<bool>,
}

pub struct TableStructureTabContent {
    database_name: String,
    table_name: String,
    config: DbConnectionConfig,
    fields: Arc<std::sync::RwLock<Vec<FieldRow>>>,
    next_id: Arc<std::sync::RwLock<usize>>,
    status_msg: Entity<String>,
    columns_loaded: Arc<std::sync::RwLock<bool>>,
    loaded_columns: Arc<std::sync::RwLock<Vec<ColumnInfo>>>,
    focus_handle: FocusHandle,
}

impl TableStructureTabContent {
    pub fn new(
        database_name: impl Into<String>,
        table_name: impl Into<String>,
        config: DbConnectionConfig,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let database_name = database_name.into();
        let table_name = table_name.into();
        let status_msg = cx.new(|_| "Loading table structure...".to_string());
        let fields = Arc::new(std::sync::RwLock::new(Vec::new()));
        let next_id = Arc::new(std::sync::RwLock::new(0));
        let columns_loaded = Arc::new(std::sync::RwLock::new(false));
        let loaded_columns = Arc::new(std::sync::RwLock::new(Vec::new()));
        let focus_handle = cx.focus_handle();

        let result = Self {
            database_name: database_name.clone(),
            table_name: table_name.clone(),
            config: config.clone(),
            fields: fields.clone(),
            next_id: next_id.clone(),
            status_msg: status_msg.clone(),
            columns_loaded: columns_loaded.clone(),
            loaded_columns: loaded_columns.clone(),
            focus_handle,
        };

        // Start loading structure in background
        result.load_structure(window, cx);
        result
    }

    fn load_structure(&self, _window: &mut Window, cx: &mut App) {
        let global_state = cx.global::<GlobalDbState>().clone();
        let config = self.config.clone();
        let table_name = self.table_name.clone();
        let database_name = self.database_name.clone();
        let status_msg = self.status_msg.clone();
        let columns_loaded = self.columns_loaded.clone();
        let loaded_columns = self.loaded_columns.clone();
        let fields = self.fields.clone();
        let next_id = self.next_id.clone();

        cx.spawn(async move |cx| {
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

            let conn_arc = match global_state
                .connection_pool
                .get_connection(config.clone(), &global_state.db_manager)
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    cx.update(|cx| {
                        status_msg.update(cx, |s, cx| {
                            *s = format!("Connection failed: {}", e);
                            cx.notify();
                        });
                    })
                    .ok();
                    return;
                }
            };

            let conn = conn_arc.read().await;

            // Load table columns
            let result = plugin.list_columns(&**conn, &database_name, &table_name).await;

            match result {
                Ok(columns) => {
                    let column_count = columns.len();

                    cx.update(|cx| {
                        *columns_loaded.write().unwrap() = true;
                        *loaded_columns.write().unwrap() = columns.clone();

                        if let Some(window_id) = cx.active_window() {
                            cx.update_window(window_id, |_entity, window, cx| {
                                // Populate fields from loaded columns
                                let mut next_id_val = next_id.write().unwrap();
                                let mut fields_vec = fields.write().unwrap();
                                fields_vec.clear();

                                for column in columns {
                                    let field_id = *next_id_val;
                                    *next_id_val += 1;

                                    let name_input = cx.new(|cx| {
                                        let mut input = InputState::new(window, cx).placeholder("field_name");
                                        input.set_value(column.name.clone(), window, cx);
                                        input
                                    });
                                    let type_input = cx.new(|cx| {
                                        let mut input = InputState::new(window, cx).placeholder("data type");
                                        input.set_value(column.data_type.clone(), window, cx);
                                        input
                                    });
                                    let nullable = cx.new(|_| column.is_nullable);

                                    fields_vec.push(FieldRow {
                                        id: field_id,
                                        name_input,
                                        type_input,
                                        nullable,
                                    });
                                }

                                drop(fields_vec);
                                drop(next_id_val);

                                status_msg.update(cx, |s, cx| {
                                    *s = format!("Loaded {} columns", column_count);
                                    cx.notify();
                                });
                            }).ok();
                        }
                    })
                    .ok();
                }
                Err(e) => {
                    cx.update(|cx| {
                        status_msg.update(cx, |s, cx| {
                            *s = format!("Failed to load columns: {}", e);
                            cx.notify();
                        });
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn add_field(&self, window: &mut Window, cx: &mut App) {
        let mut next_id_val = self.next_id.write().unwrap();
        let field_id = *next_id_val;
        *next_id_val += 1;
        drop(next_id_val);

        let name_input = cx.new(|cx| InputState::new(window, cx).placeholder("field_name"));
        let type_input = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder("VARCHAR(255)");
            input.set_value("VARCHAR(255)".to_string(), window, cx);
            input
        });
        let nullable = cx.new(|_| true);

        let mut fields_vec = self.fields.write().unwrap();
        fields_vec.push(FieldRow {
            id: field_id,
            name_input,
            type_input,
            nullable,
        });
        drop(fields_vec);

        self.status_msg.update(cx, |s, cx| {
            *s = "Added new field (unsaved)".to_string();
            cx.notify();
        });
    }

    fn delete_field(&self, field_id: usize, _window: &mut Window, cx: &mut App) {
        let mut fields_vec = self.fields.write().unwrap();
        if let Some(pos) = fields_vec.iter().position(|f| f.id == field_id) {
            fields_vec.remove(pos);
            drop(fields_vec);

            self.status_msg.update(cx, |s, cx| {
                *s = "Deleted field (unsaved)".to_string();
                cx.notify();
            });
        }
    }

    fn handle_save(&self, _: &ClickEvent, _: &mut Window, cx: &mut App) {
        // Collect field definitions
        let fields_vec = self.fields.read().unwrap();
        let fields: Vec<(String, String, bool)> = fields_vec
            .iter()
            .map(|f| {
                (
                    f.name_input.read(cx).text().to_string(),
                    f.type_input.read(cx).text().to_string(),
                    *f.nullable.read(cx),
                )
            })
            .collect();
        drop(fields_vec);

        // Validate fields
        for (i, (name, data_type, _)) in fields.iter().enumerate() {
            if name.trim().is_empty() {
                self.status_msg.update(cx, |s, cx| {
                    *s = format!("Error: Field {} has empty name", i + 1);
                    cx.notify();
                });
                return;
            }
            if data_type.trim().is_empty() {
                self.status_msg.update(cx, |s, cx| {
                    *s = format!("Error: Field '{}' has empty data type", name);
                    cx.notify();
                });
                return;
            }
        }

        let global_state = cx.global::<GlobalDbState>().clone();
        let config = self.config.clone();
        let table_name = self.table_name.clone();
        let database_name = self.database_name.clone();
        let status_msg = self.status_msg.clone();
        let loaded_columns = self.loaded_columns.read().unwrap().clone();

        cx.spawn(async move |cx| {
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

            let conn_arc = match global_state
                .connection_pool
                .get_connection(config.clone(), &global_state.db_manager)
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    cx.update(|cx| {
                        status_msg.update(cx, |s, cx| {
                            *s = format!("Connection failed: {}", e);
                            cx.notify();
                        });
                    })
                    .ok();
                    return;
                }
            };

            let conn = conn_arc.read().await;

            // Generate ALTER TABLE statements
            let mut alter_statements = Vec::new();

            // Compare with loaded columns to detect changes
            let old_columns: std::collections::HashMap<String, &ColumnInfo> = loaded_columns
                .iter()
                .map(|col| (col.name.clone(), col))
                .collect();

            let new_columns: std::collections::HashMap<String, (String, bool)> = fields
                .iter()
                .map(|(name, data_type, nullable)| (name.clone(), (data_type.clone(), *nullable)))
                .collect();

            // Detect added columns
            for (name, (data_type, nullable)) in &new_columns {
                if !old_columns.contains_key(name) {
                    let null_clause = if *nullable { "NULL" } else { "NOT NULL" };
                    alter_statements.push(format!(
                        "ALTER TABLE `{}`.`{}` ADD COLUMN `{}` {} {}",
                        database_name, table_name, name, data_type, null_clause
                    ));
                }
            }

            // Detect removed columns
            for old_name in old_columns.keys() {
                if !new_columns.contains_key(old_name) {
                    alter_statements.push(format!(
                        "ALTER TABLE `{}`.`{}` DROP COLUMN `{}`",
                        database_name, table_name, old_name
                    ));
                }
            }

            // Detect modified columns
            for (name, (new_type, new_nullable)) in &new_columns {
                if let Some(old_col) = old_columns.get(name) {
                    if &old_col.data_type != new_type || old_col.is_nullable != *new_nullable {
                        let null_clause = if *new_nullable { "NULL" } else { "NOT NULL" };
                        alter_statements.push(format!(
                            "ALTER TABLE `{}`.`{}` MODIFY COLUMN `{}` {} {}",
                            database_name, table_name, name, new_type, null_clause
                        ));
                    }
                }
            }

            if alter_statements.is_empty() {
                cx.update(|cx| {
                    status_msg.update(cx, |s, cx| {
                        *s = "No changes to save".to_string();
                        cx.notify();
                    });
                })
                .ok();
                return;
            }

            // Execute ALTER TABLE statements
            for statement in &alter_statements {
                let result = plugin.execute_query(&**conn, &database_name, statement, None).await;
                if let Err(e) = result {
                    cx.update(|cx| {
                        status_msg.update(cx, |s, cx| {
                            *s = format!("Failed to execute: {} - Error: {}", statement, e);
                            cx.notify();
                        });
                    })
                    .ok();
                    return;
                }
            }

            cx.update(|cx| {
                status_msg.update(cx, |s, cx| {
                    *s = format!("Successfully saved {} changes", alter_statements.len());
                    cx.notify();
                });
            })
            .ok();
        })
        .detach();
    }
}

impl TabContent for TableStructureTabContent {
    fn title(&self) -> SharedString {
        format!("{}.{} - Structure", self.database_name, self.table_name).into()
    }

    fn icon(&self) -> Option<IconName> {
        Some(IconName::Settings)
    }

    fn closeable(&self) -> bool {
        true
    }

    fn render_content(&self, _window: &mut Window, cx: &mut App) -> AnyElement {
        let status_msg_render = self.status_msg.clone();
        let fields_vec = self.fields.read().unwrap();

        v_flex()
            .size_full()
            .gap_2()
            .child(
                // Toolbar
                h_flex()
                    .gap_2()
                    .p_2()
                    .bg(cx.theme().muted)
                    .rounded_md()
                    .items_center()
                    .w_full()
                    .child(
                        div()
                            .text_lg()
                            .font_semibold()
                            .child(format!("Table Structure: {}.{}", self.database_name, self.table_name)),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("add-field")
                            .with_size(Size::Small)
                            .primary()
                            .label("Add Field")
                            .icon(IconName::Plus)
                            .on_click({
                                let this = self.clone();
                                move |_e, w, cx| {
                                    this.add_field(w, cx);
                                }
                            }),
                    )
                    .child(
                        Button::new("save-structure")
                            .with_size(Size::Small)
                            .primary()
                            .label("Save")
                            .icon(IconName::Check)
                            .on_click({
                                let this = self.clone();
                                move |e, w, cx| this.handle_save(e, w, cx)
                            }),
                    )
                    .child(
                        div()
                            .px_2()
                            .text_color(cx.theme().muted_foreground)
                            .text_sm()
                            .child(status_msg_render.read(cx).clone()),
                    ),
            )
            .child(
                // Fields list
                v_flex()
                    .id("fields-list")
                    .flex_1()
                    .gap_2()
                    .p_4()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .overflow_scroll()
                    .children(fields_vec.iter().map(|field| {
                        let field_clone = field.clone();
                        let this_clone = self.clone();

                        h_flex()
                            .gap_2()
                            .p_2()
                            .bg(cx.theme().muted)
                            .rounded_md()
                            .items_center()
                            .child(
                                // Field name input
                                v_flex()
                                    .gap_1()
                                    .flex_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("Field Name"),
                                    )
                                    .child(Input::new(&field.name_input).w_full()),
                            )
                            .child(
                                // Data type input
                                v_flex()
                                    .gap_1()
                                    .flex_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("Data Type"),
                                    )
                                    .child(Input::new(&field.type_input).w_full()),
                            )
                            .child(
                                // Delete button
                                Button::new(("delete-field", field.id))
                                    .with_size(Size::Small)
                                    .ghost()
                                    .icon(IconName::Delete)
                                    .on_click(move |_e, w, cx| {
                                        this_clone.delete_field(field_clone.id, w, cx);
                                    }),
                            )
                    })),
            )
            .into_any_element()
    }

    fn content_type(&self) -> TabContentType {
        TabContentType::TableForm(format!("{}.{}", self.database_name, self.table_name))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Clone for TableStructureTabContent {
    fn clone(&self) -> Self {
        Self {
            database_name: self.database_name.clone(),
            table_name: self.table_name.clone(),
            config: self.config.clone(),
            fields: self.fields.clone(),
            next_id: self.next_id.clone(),
            status_msg: self.status_msg.clone(),
            columns_loaded: self.columns_loaded.clone(),
            loaded_columns: self.loaded_columns.clone(),
            focus_handle: self.focus_handle.clone(),
        }
    }
}


impl Render for TableStructureTabContent {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.render_content(window, cx))
    }
}

impl Focusable for TableStructureTabContent {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
