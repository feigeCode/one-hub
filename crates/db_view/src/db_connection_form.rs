use gpui::{
    div, App, AppContext, ClickEvent, Context, Entity,
    EventEmitter, FocusHandle, Focusable, IntoElement, ParentElement, Render, Styled,
    Window,
};
use gpui_component::{
    button::{Button, ButtonVariants as _, DropdownButton},
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::PopupMenuItem,
    v_flex, ActiveTheme, Disableable, IconName, Sizable, Size, StyledExt,
};
use db::DatabaseType;
use db::DbConnectionConfig;

/// Represents a field in the connection form
#[derive(Clone, Debug)]
pub struct FormField {
    pub name: String,
    pub label: String,
    pub placeholder: String,
    pub field_type: FormFieldType,
    pub required: bool,
    pub default_value: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FormFieldType {
    Text,
    Number,
    Password,
}

impl FormField {
    pub fn new(
        name: impl Into<String>,
        label: impl Into<String>,
        field_type: FormFieldType,
    ) -> Self {
        let name = name.into();
        Self {
            placeholder: format!("Enter {}", name.to_lowercase()),
            name,
            label: label.into(),
            field_type,
            required: true,
            default_value: String::new(),
        }
    }

    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn default(mut self, value: impl Into<String>) -> Self {
        self.default_value = value.into();
        self
    }
}

/// Database connection form configuration for different database types
pub struct DbFormConfig {
    pub db_type: DatabaseType,
    pub title: String,
    pub fields: Vec<FormField>,
}

impl DbFormConfig {
    /// MySQL form configuration
    pub fn mysql() -> Self {
        Self {
            db_type: DatabaseType::MySQL,
            title: "Connect to MySQL".to_string(),
            fields: vec![
                FormField::new("name", "Connection Name", FormFieldType::Text)
                    .placeholder("My MySQL Database")
                    .default("Local MySQL"),
                FormField::new("host", "Host", FormFieldType::Text)
                    .placeholder("localhost")
                    .default("localhost"),
                FormField::new("port", "Port", FormFieldType::Number)
                    .placeholder("3306")
                    .default("3306"),
                FormField::new("username", "Username", FormFieldType::Text)
                    .placeholder("root")
                    .default("root"),
                FormField::new("password", "Password", FormFieldType::Password)
                    .placeholder("Enter password")
                    .default("hf123456"),
                FormField::new("database", "Database", FormFieldType::Text)
                    .optional()
                    .placeholder("database name (optional)")
                    .default("ai_app"),
            ],
        }
    }

    /// PostgreSQL form configuration
    pub fn postgres() -> Self {
        Self {
            db_type: DatabaseType::PostgreSQL,
            title: "Connect to PostgreSQL".to_string(),
            fields: vec![
                FormField::new("name", "Connection Name", FormFieldType::Text)
                    .placeholder("My PostgreSQL Database")
                    .default("Local PostgreSQL"),
                FormField::new("host", "Host", FormFieldType::Text)
                    .placeholder("localhost")
                    .default("localhost"),
                FormField::new("port", "Port", FormFieldType::Number)
                    .placeholder("5432")
                    .default("5432"),
                FormField::new("username", "Username", FormFieldType::Text)
                    .placeholder("postgres")
                    .default("postgres"),
                FormField::new("password", "Password", FormFieldType::Password)
                    .placeholder("Enter password"),
                FormField::new("database", "Database", FormFieldType::Text)
                    .optional()
                    .placeholder("database name (optional)"),
            ],
        }
    }

    /// Generic form configuration
    pub fn generic(db_type: DatabaseType, title: String) -> Self {
        Self {
            db_type,
            title,
            fields: vec![
                FormField::new("name", "Connection Name", FormFieldType::Text),
                FormField::new("host", "Host", FormFieldType::Text),
                FormField::new("port", "Port", FormFieldType::Number),
                FormField::new("username", "Username", FormFieldType::Text),
                FormField::new("password", "Password", FormFieldType::Password),
                FormField::new("database", "Database", FormFieldType::Text).optional(),
            ],
        }
    }
}

pub enum DbConnectionFormEvent {
    TestConnection(DatabaseType, DbConnectionConfig),
    Save(DatabaseType, DbConnectionConfig),
    Cancel,
}

/// Database connection form modal
pub struct DbConnectionForm {
    config: DbFormConfig,
    current_db_type: Entity<DatabaseType>,
    focus_handle: FocusHandle,
    // Field values stored as Entity<String> for reactivity
    field_values: Vec<(String, Entity<String>)>,
    field_inputs: Vec<Entity<InputState>>,
    is_testing: Entity<bool>,
    test_result: Entity<Option<Result<bool, String>>>,
}

impl DbConnectionForm {
    pub fn new(config: DbFormConfig, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let current_db_type = cx.new(|_| config.db_type);

        // Initialize field values and inputs
        let mut field_values = Vec::new();
        let mut field_inputs = Vec::new();

        for field in &config.fields {
            let value = cx.new(|_| field.default_value.clone());
            field_values.push((field.name.clone(), value.clone()));

            let input = cx.new(|cx| {
                let mut input_state = InputState::new(window, cx)
                    .placeholder(&field.placeholder);

                // Set password mode if needed
                if field.field_type == FormFieldType::Password {
                    // Note: InputState doesn't have a built-in password method
                    // We'll need to add this feature or handle it differently
                }

                input_state.set_value(field.default_value.clone(), window, cx);
                input_state
            });

            // Subscribe to input changes
            let value_clone = value.clone();
            cx.subscribe_in(&input, window, move |_form, _input, event, _window, cx| {
                if let InputEvent::Change = event {
                    value_clone.update(cx, |v, cx| {
                        // Get the new text from the input
                        *v = _input.read(cx).text().to_string();
                        cx.notify();
                    });
                }
            })
            .detach();

            field_inputs.push(input);
        }

        let is_testing = cx.new(|_| false);
        let test_result = cx.new(|_| None);

        Self {
            config,
            current_db_type,
            focus_handle,
            field_values,
            field_inputs,
            is_testing,
            test_result,
        }
    }

    fn get_field_value(&self, field_name: &str, cx: &App) -> String {
        self.field_values
            .iter()
            .find(|(name, _)| name == field_name)
            .map(|(_, value)| value.read(cx).clone())
            .unwrap_or_default()
    }

    fn switch_db_type(&mut self, db_type: DatabaseType, window: &mut Window, cx: &mut Context<Self>) {
        // Update current db type
        self.current_db_type.update(cx, |current, cx| {
            *current = db_type;
            cx.notify();
        });

        // Update config based on new db type
        self.config = match db_type {
            DatabaseType::MySQL => DbFormConfig::mysql(),
            DatabaseType::PostgreSQL => DbFormConfig::postgres(),
        };

        // Clear and reinitialize field values and inputs
        self.field_values.clear();
        self.field_inputs.clear();

        for field in &self.config.fields {
            let value = cx.new(|_| field.default_value.clone());
            self.field_values.push((field.name.clone(), value.clone()));

            let input = cx.new(|cx| {
                let mut input_state = InputState::new(window, cx)
                    .placeholder(&field.placeholder);

                input_state.set_value(field.default_value.clone(), window, cx);
                input_state
            });

            // Subscribe to input changes
            let value_clone = value.clone();
            cx.subscribe_in(&input, window, move |_form, _input, event, _window, cx| {
                if let InputEvent::Change = event {
                    value_clone.update(cx, |v, cx| {
                        *v = _input.read(cx).text().to_string();
                        cx.notify();
                    });
                }
            })
            .detach();

            self.field_inputs.push(input);
        }

        // Clear test result
        self.test_result.update(cx, |result, cx| {
            *result = None;
            cx.notify();
        });

        cx.notify();
    }

    fn build_connection(&self, cx: &App) -> DbConnectionConfig {
        DbConnectionConfig {
            id: String::new(),
            database_type: self.current_db_type.read(cx).clone(),
            name: self.get_field_value("name", cx),
            host: self.get_field_value("host", cx),
            port: self
                .get_field_value("port", cx)
                .parse()
                .unwrap_or(3306),
            username: self.get_field_value("username", cx),
            password: self.get_field_value("password", cx),
            database: {
                let db = self.get_field_value("database", cx);
                if db.is_empty() {
                    None
                } else {
                    Some(db)
                }
            },
        }
    }

    fn validate(&self, cx: &App) -> Result<(), String> {
        for field in &self.config.fields {
            if field.required {
                let value = self.get_field_value(&field.name, cx);
                if value.trim().is_empty() {
                    return Err(format!("{} is required", field.label));
                }
            }
        }
        Ok(())
    }

    fn handle_test_connection(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if let Err(e) = self.validate(cx) {
            self.test_result.update(cx, |result, cx| {
                *result = Some(Err(e));
                cx.notify();
            });
            return;
        }

        let connection = self.build_connection(cx);
        let db_type = *self.current_db_type.read(cx);

        self.is_testing.update(cx, |testing, cx| {
            *testing = true;
            cx.notify();
        });

        cx.emit(DbConnectionFormEvent::TestConnection(db_type, connection));
    }

    fn handle_save(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if let Err(e) = self.validate(cx) {
            self.test_result.update(cx, |result, cx| {
                *result = Some(Err(e));
                cx.notify();
            });
            return;
        }

        let connection = self.build_connection(cx);
        let db_type = *self.current_db_type.read(cx);
        cx.emit(DbConnectionFormEvent::Save(db_type, connection));
    }

    fn handle_cancel(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DbConnectionFormEvent::Cancel);
    }

    pub fn set_test_result(&mut self, result: Result<bool, String>, cx: &mut Context<Self>) {
        self.is_testing.update(cx, |testing, cx| {
            *testing = false;
            cx.notify();
        });
        self.test_result.update(cx, |test_result, cx| {
            *test_result = Some(result);
            cx.notify();
        });
    }
}

impl EventEmitter<DbConnectionFormEvent> for DbConnectionForm {}

impl Focusable for DbConnectionForm {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for DbConnectionForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_testing = *self.is_testing.read(cx);
        let test_result_msg = self.test_result.read(cx).as_ref().map(|r| match r {
            Ok(true) => "✓ Connection successful!".to_string(),
            Ok(false) => "✗ Connection failed".to_string(),
            Err(e) => format!("✗ {}", e),
        });
        let current_db_type = *self.current_db_type.read(cx);

        // Modal overlay
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x00_00_00_40))
            .child(
                // Modal content
                v_flex()
                    .gap_4()
                    .p_6()
                    .w(gpui::px(500.))
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_lg()
                    .shadow_lg()
                    .child(
                        // Header
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_xl()
                                    .font_semibold()
                                    .text_color(cx.theme().foreground)
                                    .child(self.config.title.clone()),
                            )
                            .child(
                                Button::new("close")
                                    .ghost()
                                    .with_size(Size::XSmall)
                                    .label("✕")
                                    .on_click(cx.listener(Self::handle_cancel)),
                            ),
                    )
                    .child(
                        // Database type selector
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_medium()
                                    .text_color(cx.theme().foreground)
                                    .child("Database Type"),
                            )
                            .child({
                                let view = cx.entity();
                                let db_type_label = current_db_type.as_str().to_string();
                                DropdownButton::new("db-type-selector")
                                    .w_full()
                                    .button(
                                        Button::new("db-type-button")
                                            .label(db_type_label)
                                            .icon(IconName::ChevronDown)
                                    )
                                    .dropdown_menu(move |menu, window, _cx| {
                                        menu.item(
                                            PopupMenuItem::new("MySQL")
                                                .on_click(window.listener_for(&view, move |this, _, window, cx| {
                                                    this.switch_db_type(DatabaseType::MySQL, window, cx);
                                                }))
                                        )
                                        .item(
                                            PopupMenuItem::new("PostgreSQL")
                                                .on_click(window.listener_for(&view, move |this, _, window, cx| {
                                                    this.switch_db_type(DatabaseType::PostgreSQL, window, cx);
                                                }))
                                        )
                                    })
                            }),
                    )
                    .child(
                        // Form fields
                        v_flex()
                            .gap_3()
                            .children(
                                self.config
                                    .fields
                                    .iter()
                                    .enumerate()
                                    .map(|(i, field)| {
                                        v_flex()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_medium()
                                                    .text_color(cx.theme().foreground)
                                                    .child(format!(
                                                        "{}{}",
                                                        field.label,
                                                        if field.required { " *" } else { "" }
                                                    )),
                                            )
                                            .child(Input::new(&self.field_inputs[i]).w_full())
                                    }),
                            ),
                    )
                    .children(test_result_msg.map(|msg| {
                        let is_success = msg.starts_with("✓");
                        div()
                            .p_3()
                            .rounded_md()
                            .bg(if is_success {
                                gpui::rgb(0xdcfce7)
                            } else {
                                gpui::rgb(0xfee2e2)
                            })
                            .text_color(if is_success {
                                gpui::rgb(0x166534)
                            } else {
                                gpui::rgb(0x991b1b)
                            })
                            .child(msg)
                    }))
                    .child(
                        // Action buttons
                        h_flex()
                            .gap_2()
                            .justify_end()
                            .child(
                                Button::new("cancel")
                                    .ghost()
                                    .with_size(Size::Medium)
                                    .label("Cancel")
                                    .on_click(cx.listener(Self::handle_cancel)),
                            )
                            .child(
                                Button::new("test")
                                    .outline()
                                    .with_size(Size::Medium)
                                    .label(if is_testing {
                                        "Testing..."
                                    } else {
                                        "Test Connection"
                                    })
                                    .disabled(is_testing)
                                    .on_click(cx.listener(Self::handle_test_connection)),
                            )
                            .child(
                                Button::new("save")
                                    .primary()
                                    .with_size(Size::Medium)
                                    .label("Save & Connect")
                                    .disabled(is_testing)
                                    .on_click(cx.listener(Self::handle_save)),
                            ),
                    ),
            )
    }
}
