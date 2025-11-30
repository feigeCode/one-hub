use std::any::Any;
use std::sync::Arc;

use one_core::gpui_tokio::Tokio;
use one_core::tab_container::{TabContent, TabContentType};
use db::{ColumnInfo, DataTypeCategory, DataTypeInfo, GlobalDbState};
use gpui::{div, px, AnyElement, App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement, Render, SharedString, Styled, Window};
use gpui_component::{
    button::{Button, ButtonVariants as _, DropdownButton},
    h_flex,
    input::{Input, InputState},
    menu::PopupMenuItem,
    switch::Switch,
    v_flex, ActiveTheme, IconName, Sizable, StyledExt as _,
};
use one_core::storage::DatabaseType;

/// 字段行
#[derive(Clone)]
struct FieldRow {
    id: usize,
    name_input: Entity<InputState>,
    type_input: Entity<InputState>,
    nullable: Entity<bool>,
    primary_key: Entity<bool>,
    default_value: Entity<InputState>,
    comment: Entity<InputState>,
    selected_type: Entity<Option<String>>,
}

/// 表设计器视图
/// Visual table designer for creating and editing database tables
pub struct TableDesignerView {
    database_name: String,
    table_name: Option<String>,
    connection_id: String,
    database_type: DatabaseType,
    table_name_input: Entity<InputState>,
    fields: Arc<std::sync::RwLock<Vec<FieldRow>>>,
    next_id: Arc<std::sync::RwLock<usize>>,
    data_types: Arc<Vec<DataTypeInfo>>,
    status_msg: Entity<String>,
    preview_sql: Entity<String>,
    focus_handle: FocusHandle,
    is_new_table: bool,
}

impl TableDesignerView {
    /// 创建新表
    pub fn new_table(
        database_name: impl Into<String>,
        connection_id: impl Into<String>,
        database_type: DatabaseType,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let database_name = database_name.into();
        let connection_id = connection_id.into();
        
        cx.new(|cx| {
            let table_name_input = cx.new(|cx| InputState::new(window, cx).placeholder("table_name"));
            let fields = Arc::new(std::sync::RwLock::new(Vec::new()));
            let next_id = Arc::new(std::sync::RwLock::new(0));
            let status_msg = cx.new(|_| "New table".to_string());
            let preview_sql = cx.new(|_| "-- Enter table name and add fields to preview SQL".to_string());
            
            // 获取数据类型列表
            let data_types = Self::load_data_types(&connection_id, cx);
            
            let mut view = Self {
                database_name,
                table_name: None,
                connection_id,
                database_type,
                table_name_input,
                fields,
                next_id,
                data_types: Arc::new(data_types),
                status_msg,
                preview_sql,
                focus_handle: cx.focus_handle(),
                is_new_table: true,
            };
            
            // 添加第一个字段
            view.add_field(window, cx);
            
            view
        })
    }

    /// 编辑现有表
    pub fn edit_table(
        database_name: impl Into<String>,
        table_name: impl Into<String>,
        connection_id: impl Into<String>,
        database_type: DatabaseType,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let database_name = database_name.into();
        let table_name = table_name.into();
        let connection_id = connection_id.into();
        
        cx.new(|cx| {
            let table_name_input = cx.new(|cx| {
                let mut state = InputState::new(window, cx);
                state.set_value(table_name.clone(), window, cx);
                state
            });
            let fields = Arc::new(std::sync::RwLock::new(Vec::new()));
            let next_id = Arc::new(std::sync::RwLock::new(0));
            let status_msg = cx.new(|_| "Loading...".to_string());
            let preview_sql = cx.new(|_| String::new());
            
            // 获取数据类型列表
            let data_types = Self::load_data_types(&connection_id, cx);
            
            let view = Self {
                database_name: database_name.clone(),
                table_name: Some(table_name.clone()),
                connection_id: connection_id.clone(),
                database_type,
                table_name_input,
                fields: fields.clone(),
                next_id: next_id.clone(),
                data_types: Arc::new(data_types),
                status_msg: status_msg.clone(),
                preview_sql: preview_sql.clone(),
                focus_handle: cx.focus_handle(),
                is_new_table: false,
            };
            
            // 加载现有表结构
            view.load_table_structure(window, cx);
            
            view
        })
    }

    fn load_data_types(connection_id: &str, cx: &mut App) -> Vec<DataTypeInfo> {
        let global_state = cx.global::<GlobalDbState>();

        // Use Tokio::block_on to execute async operations from sync context
        let config = Tokio::block_on(cx, global_state.get_config(connection_id));

        if let Some(config) = config {
            if let Ok(plugin) = global_state.db_manager.get_plugin(&config.database_type) {
                return plugin.get_data_types();
            }
        }

        vec![]
    }

    fn load_table_structure(&self, _window: &mut Window, cx: &mut App) {
        let global_state = cx.global::<GlobalDbState>().clone();
        let connection_id = self.connection_id.clone();
        let table_name = self.table_name.clone().unwrap_or_default();
        let database_name = self.database_name.clone();
        let status_msg = self.status_msg.clone();
        let fields = self.fields.clone();
        let next_id = self.next_id.clone();

        cx.spawn(async move |cx| {
            let (plugin, conn_arc) = match global_state.get_plugin_and_connection(&connection_id).await {
                Ok(p) => p,
                Err(e) => {
                    cx.update(|cx| {
                        status_msg.update(cx, |s, cx| {
                            *s = format!("Failed to get plugin: {}", e);
                            cx.notify();
                        });
                    }).ok();
                    return;
                }
            };
            let conn = conn_arc.read().await;
            let result = plugin.list_columns(&**conn, &database_name, &table_name).await;

            match result {
                Ok(columns) => {
                    cx.update(|cx| {
                        if let Some(window_id) = cx.active_window() {
                            cx.update_window(window_id, |_, window, cx| {
                                let mut next_id_val = next_id.write().unwrap();
                                let mut fields_vec = fields.write().unwrap();
                                fields_vec.clear();

                                for column in columns {
                                    let field_id = *next_id_val;
                                    *next_id_val += 1;

                                    let name_input = cx.new(|cx| {
                                        let mut input = InputState::new(window, cx);
                                        input.set_value(column.name.clone(), window, cx);
                                        input
                                    });
                                    let type_input = cx.new(|cx| {
                                        let mut input = InputState::new(window, cx);
                                        input.set_value(column.data_type.clone(), window, cx);
                                        input
                                    });
                                    let nullable = cx.new(|_| column.is_nullable);
                                    let primary_key = cx.new(|_| column.is_primary_key);
                                    let default_value = cx.new(|cx| {
                                        let mut input = InputState::new(window, cx);
                                        if let Some(def) = &column.default_value {
                                            input.set_value(def.clone(), window, cx);
                                        }
                                        input
                                    });
                                    let comment = cx.new(|cx| {
                                        let mut input = InputState::new(window, cx);
                                        if let Some(cmt) = &column.comment {
                                            input.set_value(cmt.clone(), window, cx);
                                        }
                                        input
                                    });

                                    fields_vec.push(FieldRow {
                                        id: field_id,
                                        name_input,
                                        type_input,
                                        nullable,
                                        primary_key,
                                        default_value,
                                        comment,
                                        selected_type: cx.new(|_| Some(column.data_type.clone())),
                                    });
                                }

                                status_msg.update(cx, |s, cx| {
                                    *s = format!("Loaded {} columns", fields_vec.len());
                                    cx.notify();
                                });
                            }).ok();
                        }
                    }).ok();
                }
                Err(e) => {
                    cx.update(|cx| {
                        status_msg.update(cx, |s, cx| {
                            *s = format!("Failed to load columns: {}", e);
                            cx.notify();
                        });
                    }).ok();
                }
            }
        }).detach();
    }

    fn add_field(&mut self, window: &mut Window, cx: &mut App) {
        let field_id = {
            let mut next_id_val = self.next_id.write().unwrap();
            let id = *next_id_val;
            *next_id_val += 1;
            id
        };

        let name_input = cx.new(|cx| InputState::new(window, cx).placeholder("field_name"));
        let type_input = cx.new(|cx| InputState::new(window, cx).placeholder("Select type"));
        let nullable = cx.new(|_| true);
        let primary_key = cx.new(|_| false);
        let default_value = cx.new(|cx| InputState::new(window, cx).placeholder("NULL"));
        let comment = cx.new(|cx| InputState::new(window, cx).placeholder("Comment"));

        self.fields.write().unwrap().push(FieldRow {
            id: field_id,
            name_input,
            type_input,
            nullable,
            primary_key,
            default_value,
            comment,
            selected_type: cx.new(|_| None),
        });

        self.update_preview_sql(cx);
    }

    fn delete_field(&mut self, field_id: usize, _window: &mut Window, cx: &mut App) {
        {
            let mut fields_vec = self.fields.write().unwrap();
            if let Some(pos) = fields_vec.iter().position(|f| f.id == field_id) {
                fields_vec.remove(pos);
            } else {
                return; // Field not found, no need to update
            }
        }

        self.status_msg.update(cx, |s, cx| {
            *s = "Field deleted (unsaved)".to_string();
            cx.notify();
        });
        self.update_preview_sql(cx);
    }

    fn select_data_type(&mut self, field_id: usize, data_type: String, window: &mut Window, cx: &mut App) {
        {
            let fields_vec = self.fields.read().unwrap();
            if let Some(field) = fields_vec.iter().find(|f| f.id == field_id) {
                field.type_input.update(cx, |state, cx| {
                    state.replace(data_type.clone(), window, cx);
                });
                field.selected_type.update(cx, |t, cx| {
                    *t = Some(data_type);
                    cx.notify();
                });
            }
        }
        self.update_preview_sql(cx);
    }

    fn toggle_nullable(&mut self, field_id: usize, cx: &mut App) {
        {
            let fields_vec = self.fields.read().unwrap();
            if let Some(field) = fields_vec.iter().find(|f| f.id == field_id) {
                field.nullable.update(cx, |val, cx| {
                    *val = !*val;
                    cx.notify();
                });
            }
        }
        self.update_preview_sql(cx);
    }

    fn toggle_primary_key(&mut self, field_id: usize, cx: &mut App) {
        {
            let fields_vec = self.fields.read().unwrap();
            if let Some(field) = fields_vec.iter().find(|f| f.id == field_id) {
                field.primary_key.update(cx, |val, cx| {
                    *val = !*val;
                    cx.notify();
                });
            }
        }
        self.update_preview_sql(cx);
    }

    fn update_preview_sql(&mut self, cx: &mut App) {
        let table_name = self.table_name_input.read(cx).text().to_string();

        if table_name.trim().is_empty() {
            self.preview_sql.update(cx, |sql, cx| {
                *sql = "-- Enter table name to preview SQL".to_string();
                cx.notify();
            });
            return;
        }

        let columns = {
            let fields_vec = self.fields.read().unwrap();
            let mut columns = Vec::new();

            for field in fields_vec.iter() {
                let name = field.name_input.read(cx).text().to_string();
                let data_type = field.type_input.read(cx).text().to_string();

                if name.trim().is_empty() || data_type.trim().is_empty() {
                    continue;
                }

                let nullable = *field.nullable.read(cx);
                let primary_key = *field.primary_key.read(cx);
                let default_value_text = field.default_value.read(cx).text().to_string();
                let comment_text = field.comment.read(cx).text().to_string();

                columns.push(ColumnInfo {
                    name,
                    data_type,
                    is_nullable: nullable,
                    is_primary_key: primary_key,
                    default_value: if default_value_text.trim().is_empty() {
                        None
                    } else {
                        Some(default_value_text)
                    },
                    comment: if comment_text.trim().is_empty() {
                        None
                    } else {
                        Some(comment_text)
                    },
                });
            }

            columns
        };

        if columns.is_empty() {
            self.preview_sql.update(cx, |sql, cx| {
                *sql = "-- Add at least one valid column to preview SQL".to_string();
                cx.notify();
            });
            return;
        }

        let global_state = cx.global::<GlobalDbState>();
        let plugin = match global_state.db_manager.get_plugin(&self.database_type) {
            Ok(p) => p,
            Err(_) => {
                self.preview_sql.update(cx, |sql, cx| {
                    *sql = "-- Error: Cannot load database plugin".to_string();
                    cx.notify();
                });
                return;
            }
        };

        let request = db::CreateTableRequest {
            database_name: self.database_name.clone(),
            table_name,
            columns,
            if_not_exists: true,
        };

        // match plugin.generate_create_table_sql(&request) {
        //     Ok(sql) => {
        //         self.preview_sql.update(cx, |preview, cx| {
        //             *preview = sql;
        //             cx.notify();
        //         });
        //     }
        //     Err(e) => {
        //         self.preview_sql.update(cx, |sql, cx| {
        //             *sql = format!("-- Error generating SQL: {}", e);
        //             cx.notify();
        //         });
        //     }
        // }
    }

    fn handle_save(&mut self, _window: &mut Window, cx: &mut App) {
        let table_name = self.table_name_input.read(cx).text().to_string();

        if table_name.trim().is_empty() {
            self.status_msg.update(cx, |s, cx| {
                *s = "Error: Table name is required".to_string();
                cx.notify();
            });
            return;
        }

        // Collect field definitions and validate
        let fields_vec = self.fields.read().unwrap();
        let mut columns = Vec::new();

        for field in fields_vec.iter() {
            let name = field.name_input.read(cx).text().to_string();
            let data_type = field.type_input.read(cx).text().to_string();

            if name.trim().is_empty() {
                drop(fields_vec);
                self.status_msg.update(cx, |s, cx| {
                    *s = "Error: All fields must have a name".to_string();
                    cx.notify();
                });
                return;
            }

            if data_type.trim().is_empty() {
                drop(fields_vec);
                self.status_msg.update(cx, |s, cx| {
                    *s = format!("Error: Field '{}' must have a data type", name);
                    cx.notify();
                });
                return;
            }

            let nullable = *field.nullable.read(cx);
            let primary_key = *field.primary_key.read(cx);
            let default_value_text = field.default_value.read(cx).text().to_string();
            let comment_text = field.comment.read(cx).text().to_string();

            columns.push(ColumnInfo {
                name,
                data_type,
                is_nullable: nullable,
                is_primary_key: primary_key,
                default_value: if default_value_text.trim().is_empty() {
                    None
                } else {
                    Some(default_value_text)
                },
                comment: if comment_text.trim().is_empty() {
                    None
                } else {
                    Some(comment_text)
                },
            });
        }
        drop(fields_vec);

        if columns.is_empty() {
            self.status_msg.update(cx, |s, cx| {
                *s = "Error: Table must have at least one valid column".to_string();
                cx.notify();
            });
            return;
        }

        // Execute create or modify
        let global_state = cx.global::<GlobalDbState>().clone();
        let connection_id = self.connection_id.clone();
        let database_name = self.database_name.clone();
        let status_msg = self.status_msg.clone();
        let is_new = self.is_new_table;

        self.status_msg.update(cx, |s, cx| {
            *s = "Saving table...".to_string();
            cx.notify();
        });

        cx.spawn(async move |cx| {
            let (plugin, conn_arc) = match global_state.get_plugin_and_connection(&connection_id).await {
                Ok(p) => p,
                Err(e) => {
                    cx.update(|cx| {
                        status_msg.update(cx, |s, cx| {
                            *s = format!("Error: Failed to get database connection: {}", e);
                            cx.notify();
                        });
                    }).ok();
                    return;
                }
            };

            if is_new {
                // Create new table
                let request = db::CreateTableRequest {
                    database_name,
                    table_name: table_name.clone(),
                    columns,
                    if_not_exists: true,
                };

                // match plugin.generate_create_table_sql(&request) {
                //     Ok(sql) => {
                //         let conn = conn_arc.read().await;
                //         match conn.execute(&sql, db::ExecOptions::default()).await {
                //             Ok(_) => {
                //                 cx.update(|cx| {
                //                     status_msg.update(cx, |s, cx| {
                //                         *s = format!("✓ Table '{}' created successfully", table_name);
                //                         cx.notify();
                //                     });
                //                 }).ok();
                //             }
                //             Err(e) => {
                //                 cx.update(|cx| {
                //                     status_msg.update(cx, |s, cx| {
                //                         *s = format!("Error: Failed to create table: {}", e);
                //                         cx.notify();
                //                     });
                //                 }).ok();
                //             }
                //         }
                //     }
                //     Err(e) => {
                //         cx.update(|cx| {
                //             status_msg.update(cx, |s, cx| {
                //                 *s = format!("Error: Failed to generate SQL: {}", e);
                //                 cx.notify();
                //             });
                //         }).ok();
                //     }
                // }
            } else {
                // Implement ALTER TABLE logic
                cx.update(|cx| {
                    status_msg.update(cx, |s, cx| {
                        *s = "Error: Alter table not yet implemented. Please drop and recreate the table.".to_string();
                        cx.notify();
                    });
                }).ok();
            }
        }).detach();
    }

    fn render_field_row(&self, field: &FieldRow, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let field_id = field.id;
        let data_types = self.data_types.clone();
        let selected_type = field.selected_type.read(cx).clone();
        let view_entity = cx.entity();
        let view_entity_for_menu = view_entity.clone();

        h_flex()
            .gap_2()
            .items_center()
            .p_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                // 字段名
                Input::new(&field.name_input).w(px(150.0))
            )
            .child(
                // 数据类型下拉
                DropdownButton::new(SharedString::from(format!("type-{}", field_id)))
                    .w(px(180.0))
                    .button(
                        Button::new(SharedString::from(format!("type-btn-{}", field_id)))
                            .label(selected_type.unwrap_or_else(|| "Select type".to_string()))
                            .icon(IconName::ChevronDown)
                    )
                    .dropdown_menu(move |menu, window, _| {
                        let view_entity = view_entity_for_menu.clone();
                        let mut menu = menu;
                        
                        // 按类别分组
                        let mut by_category: std::collections::HashMap<DataTypeCategory, Vec<DataTypeInfo>> = 
                            std::collections::HashMap::new();
                        
                        for dt in data_types.iter() {
                            by_category.entry(dt.category).or_insert_with(Vec::new).push(dt.clone());
                        }

                        // 数值类型
                        if let Some(types) = by_category.get(&DataTypeCategory::Numeric) {
                            menu = menu.label("Numeric");
                            for dt in types {
                                let type_name = dt.name.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(format!("{} - {}", dt.name, dt.description))
                                        .on_click(window.listener_for(&view_entity, move |this, _, window, cx| {
                                            this.select_data_type(field_id, type_name.clone(), window, cx);
                                        }))
                                );
                            }
                            menu = menu.separator();
                        }

                        // 字符串类型
                        if let Some(types) = by_category.get(&DataTypeCategory::String) {
                            menu = menu.label("String");
                            for dt in types {
                                let type_name = dt.name.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(format!("{} - {}", dt.name, dt.description))
                                        .on_click(window.listener_for(&view_entity, move |this, _, window, cx| {
                                            this.select_data_type(field_id, type_name.clone(), window, cx);
                                        }))
                                );
                            }
                            menu = menu.separator();
                        }

                        // 日期时间类型
                        if let Some(types) = by_category.get(&DataTypeCategory::DateTime) {
                            menu = menu.label("Date/Time");
                            for dt in types {
                                let type_name = dt.name.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(format!("{} - {}", dt.name, dt.description))
                                        .on_click(window.listener_for(&view_entity, move |this, _, window, cx| {
                                            this.select_data_type(field_id, type_name.clone(), window, cx);
                                        }))
                                );
                            }
                            menu = menu.separator();
                        }

                        // 其他类型
                        for (cat, types) in by_category.iter() {
                            if matches!(cat, DataTypeCategory::Boolean | DataTypeCategory::Binary | DataTypeCategory::Structured | DataTypeCategory::Other) {
                                menu = menu.label(format!("{:?}", cat));
                                for dt in types {
                                    let type_name = dt.name.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new(format!("{} - {}", dt.name, dt.description))
                                            .on_click(window.listener_for(&view_entity, move |this, _, window, cx| {
                                                this.select_data_type(field_id, type_name.clone(), window, cx);
                                            }))
                                    );
                                }
                                menu = menu.separator();
                            }
                        }

                        menu
                    })
            )
            .child(
                // Nullable
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(
                        Switch::new(SharedString::from(format!("nullable-{}", field_id)))
                            .checked(*field.nullable.read(cx))
                            .on_click(window.listener_for(&view_entity, move |this, _, _, cx| {
                                this.toggle_nullable(field_id, cx);
                            }))
                    )
                    .child(div().text_xs().child("NULL"))
            )
            .child(
                // Primary Key
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(
                        Switch::new(SharedString::from(format!("pk-{}", field_id)))
                            .checked(*field.primary_key.read(cx))
                            .on_click(window.listener_for(&view_entity, move |this, _, _, cx| {
                                this.toggle_primary_key(field_id, cx);
                            }))
                    )
                    .child(div().text_xs().child("PK"))
            )
            .child(
                // Default
                Input::new(&field.default_value).w(px(120.0))
            )
            .child(
                // Comment
                Input::new(&field.comment).w(px(200.0))
            )
            .child(
                // Delete button
                Button::new(SharedString::from(format!("delete-{}", field_id)))
                    .icon(IconName::Delete)
                    .ghost()
                    .small()
                    .on_click(window.listener_for(&view_entity, move |this, _, window, cx| {
                        this.delete_field(field_id, window, cx);
                    }))
            )
    }
}

impl Focusable for TableDesignerView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TableDesignerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status_text = self.status_msg.read(cx).clone();
        let preview_sql_text = self.preview_sql.read(cx).clone();
        let fields_vec = self.fields.read().unwrap().clone();

        v_flex()
            .size_full()
            .child(
                // Toolbar
                h_flex()
                    .gap_2()
                    .p_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new("add_field")
                            .icon(IconName::Plus)
                            .child("Add Field")
                            .on_click(window.listener_for(&cx.entity(), |this, _, window, cx| {
                                this.add_field(window, cx);
                            }))
                    )
                    .child(
                        Button::new("preview")
                            .icon(IconName::Eye)
                            .child("Preview SQL")
                            .on_click(window.listener_for(&cx.entity(), |this, _, _, cx| {
                                this.update_preview_sql(cx);
                            }))
                    )
                    .child(
                        Button::new("copy_sql")
                            .icon(IconName::Copy)
                            .child("Copy SQL")
                            .on_click(window.listener_for(&cx.entity(), |this, _, _, cx| {
                                let sql = this.preview_sql.read(cx).clone();
                                if !sql.starts_with("--") {
                                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(sql));
                                    this.status_msg.update(cx, |s, cx| {
                                        *s = "SQL copied to clipboard".to_string();
                                        cx.notify();
                                    });
                                }
                            }))
                    )
                    .child(
                        Button::new("save")
                            .icon(IconName::Check)
                            .child("Execute")
                            .primary()
                            .on_click(window.listener_for(&cx.entity(), |this, _, window, cx| {
                                this.handle_save(window, cx);
                            }))
                    )
            )
            .child(
                // Table name
                h_flex()
                    .gap_2()
                    .p_2()
                    .items_center()
                    .child(div().w(px(100.0)).child("Table Name:"))
                    .child(Input::new(&self.table_name_input).w(px(300.0)))
            )
            .child(
                // Header row
                h_flex()
                    .gap_2()
                    .p_2()
                    .bg(cx.theme().muted)
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(div().w(px(150.0)).child("Field Name"))
                    .child(div().w(px(180.0)).child("Data Type"))
                    .child(div().w(px(60.0)).child("Nullable"))
                    .child(div().w(px(60.0)).child("Primary"))
                    .child(div().w(px(120.0)).child("Default"))
                    .child(div().w(px(200.0)).child("Comment"))
                    .child(div().w(px(60.0)).child("Actions"))
            )
            .child(
                // Fields list
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child({
                        let mut fields_container = v_flex().id("fields");
                        for field in fields_vec.iter() {
                            fields_container = fields_container.child(self.render_field_row(field, window, cx));
                        }
                        fields_container.scrollable(gpui::Axis::Vertical)
                    })
            )
            .child(
                // SQL Preview
                v_flex()
                    .h(px(200.0))
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        h_flex()
                            .p_2()
                            .bg(cx.theme().muted)
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child("SQL Preview")
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Click 'Preview SQL' to generate")
                            )
                    )
                    .child(
                        div()
                            .flex_1()
                            .p_2()
                            .overflow_hidden()
                            .font_family("monospace")
                            .text_xs()
                            .bg(cx.theme().background)
                            .child(preview_sql_text)
                    )
            )
            .child(
                // Status bar
                div()
                    .p_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().muted)
                    .child(status_text)
            )
    }
}

impl TabContent for TableDesignerView {
    fn title(&self) -> SharedString {
        if self.is_new_table {
            "New Table".into()
        } else {
            format!("Design: {}", self.table_name.as_ref().unwrap_or(&"Unknown".to_string())).into()
        }
    }

    fn icon(&self) -> Option<IconName> {
        Some(IconName::Table)
    }

    fn closeable(&self) -> bool {
        true
    }

    fn render_content(&self, _window: &mut Window, cx: &mut App) -> AnyElement {
        let view_clone = cx.new(|_| self.clone());
        div().size_full().child(view_clone).into_any_element()
    }

    fn content_type(&self) -> TabContentType {
        TabContentType::Custom("table-designer".to_string())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Clone for TableDesignerView {
    fn clone(&self) -> Self {
        Self {
            database_name: self.database_name.clone(),
            table_name: self.table_name.clone(),
            connection_id: self.connection_id.clone(),
            database_type: self.database_type.clone(),
            table_name_input: self.table_name_input.clone(),
            fields: self.fields.clone(),
            next_id: self.next_id.clone(),
            data_types: self.data_types.clone(),
            status_msg: self.status_msg.clone(),
            preview_sql: self.preview_sql.clone(),
            focus_handle: self.focus_handle.clone(),
            is_new_table: self.is_new_table,
        }
    }
}
