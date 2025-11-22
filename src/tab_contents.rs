use gpui::{
    div, AnyElement, App, AppContext as _, ClickEvent, Entity, IntoElement,
    ParentElement, SharedString, Styled, Window, Focusable, FocusHandle, EventEmitter,
    Render, Context, WeakEntity,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputState},
    table::{Column, Table, TableDelegate},
    v_flex, ActiveTheme as _, IconName, Sizable as _, Size, StyledExt as _,
    dock::{Panel, PanelControl, PanelEvent, PanelState, TabPanel, TitleStyle},
    menu::PopupMenu,
};
use std::any::Any;
use std::sync::Arc;
use gpui_component::table::TableState;
use db::{GlobalDbState, ColumnInfo};
use crate::tab_container::{TabContent, TabContentType};

// ============================================================================
// SQL Editor Tab Content
// ============================================================================





// ============================================================================
// Table Data Tab Content - Display table rows
// ============================================================================

pub struct TableDataTabContent {
    table_name: String,
    delegate: Arc<std::sync::RwLock<ResultsDelegate>>,
    table: Entity<TableState<DelegateWrapper>>,
    status_msg: Entity<String>,
    focus_handle: FocusHandle,
}

impl TableDataTabContent {
    pub fn new(
        table_name: impl Into<String>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let table_name = table_name.into();
        let delegate = Arc::new(std::sync::RwLock::new(ResultsDelegate {
            columns: vec![],
            rows: vec![],
        }));

        let delegate_wrapper = DelegateWrapper {
            inner: delegate.clone(),
        };
        let table = cx.new(|cx| TableState::new(delegate_wrapper, window, cx));
        let status_msg = cx.new(|_| "Loading...".to_string());
        let focus_handle = cx.focus_handle();

        let result = Self {
            table_name: table_name.clone(),
            delegate: delegate.clone(),
            table: table.clone(),
            status_msg: status_msg.clone(),
            focus_handle,
        };

        // Load data initially
        // TODO: Need connection config to load data
        // result.load_data(cx);
        status_msg.update(cx, |s, _cx| {
            *s = "Table data loading not yet implemented with new connection pool".to_string();
        });

        result
    }

    fn load_data(&self, _cx: &mut App) {
        // TODO: Implement data loading with connection config
        // This requires the connection config to be passed in
    }

    fn handle_refresh(&self, _: &ClickEvent, _: &mut Window, cx: &mut App) {
        self.load_data(cx);
    }
}

impl TabContent for TableDataTabContent {
    fn title(&self) -> SharedString {
        format!("{} - Data", self.table_name).into()
    }

    fn icon(&self) -> Option<IconName> {
        Some(IconName::Folder)
    }

    fn closeable(&self) -> bool {
        true
    }

    fn render_content(&self, _window: &mut Window, cx: &mut App) -> AnyElement {
        let status_msg_render = self.status_msg.clone();

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
                        Button::new("refresh-data")
                            .with_size(Size::Small)
                            .primary()
                            .label("Refresh")
                            .icon(IconName::ArrowDown)
                            .on_click({
                                let this = self.clone();
                                move |e, w, cx| this.handle_refresh(e, w, cx)
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .justify_end()
                            .items_center()
                            .px_2()
                            .text_color(cx.theme().muted_foreground)
                            .text_sm()
                            .child(status_msg_render.read(cx).clone()),
                    ),
            )
            .child(
                // Table
                v_flex()
                    .flex_1()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .overflow_hidden()
                    .child(Table::new(&self.table)),
            )
            .into_any_element()
    }

    fn content_type(&self) -> TabContentType {
        TabContentType::TableData(self.table_name.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Clone for TableDataTabContent {
    fn clone(&self) -> Self {
        Self {
            table_name: self.table_name.clone(),
            delegate: self.delegate.clone(),
            table: self.table.clone(),
            status_msg: self.status_msg.clone(),
            focus_handle: self.focus_handle.clone(),
        }
    }
}

impl EventEmitter<PanelEvent> for TableDataTabContent {}

impl Render for TableDataTabContent {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(self.render_content(window, cx))
    }
}

impl Focusable for TableDataTabContent {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for TableDataTabContent {
    fn panel_name(&self) -> &'static str {
        "TableData"
    }

    fn tab_name(&self, _cx: &App) -> Option<SharedString> {
        Some(TabContent::title(self))
    }

    fn title(&self, _window: &Window, _cx: &App) -> AnyElement {
        h_flex()
            .items_center()
            .gap_2()
            .child(IconName::Folder)
            .child(TabContent::title(self))
            .into_any_element()
    }

    fn title_style(&self, _cx: &App) -> Option<TitleStyle> {
        None
    }

    fn title_suffix(&self, _window: &mut Window, _cx: &mut App) -> Option<AnyElement> {
        None
    }

    fn closable(&self, _cx: &App) -> bool {
        true
    }

    fn zoomable(&self, _cx: &App) -> Option<PanelControl> {
        None
    }

    fn visible(&self, _cx: &App) -> bool {
        true
    }

    fn set_active(&mut self, _active: bool, _window: &mut Window, _cx: &mut App) {
        // No special handling needed
    }

    fn set_zoomed(&mut self, _zoomed: bool, _window: &mut Window, _cx: &mut App) {
        // No special handling needed
    }

    fn on_added_to(&mut self, _tab_panel: WeakEntity<TabPanel>, _window: &mut Window, _cx: &mut App) {
        // No special handling needed
    }

    fn on_removed(&mut self, _window: &mut Window, _cx: &mut App) {
        // No special handling needed
    }

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

// ============================================================================
// Table Structure Tab Content - Edit table structure
// ============================================================================

#[derive(Clone, Debug)]
struct FieldRow {
    id: usize,
    name: Entity<String>,
    data_type: Entity<String>,
    nullable: Entity<bool>,
    name_input: Entity<InputState>,
    type_input: Entity<InputState>,
}

pub struct TableStructureTabContent {
    database_name: String,
    table_name: String,
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
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let database_name = database_name.into();
        let table_name = table_name.into();
        let status_msg = cx.new(|_| "Click 'Load Columns' to load existing table structure".to_string());
        let fields = Arc::new(std::sync::RwLock::new(Vec::new()));
        let next_id = Arc::new(std::sync::RwLock::new(0));
        let columns_loaded = Arc::new(std::sync::RwLock::new(false));
        let loaded_columns = Arc::new(std::sync::RwLock::new(Vec::new()));
        let focus_handle = cx.focus_handle();

        let result = Self {
            database_name: database_name.clone(),
            table_name: table_name.clone(),
            fields: fields.clone(),
            next_id: next_id.clone(),
            status_msg: status_msg.clone(),
            columns_loaded: columns_loaded.clone(),
            loaded_columns: loaded_columns.clone(),
            focus_handle,
        };

        // Start loading structure in background
        // TODO: Need connection config to load structure
        // result.load_structure(window, cx);
        status_msg.update(cx, |s, _cx| {
            *s = "Table structure loading not yet implemented with new connection pool".to_string();
        });
        result
    }

    fn load_structure(&self, _window: &mut Window, _cx: &mut App) {
        // TODO: Implement structure loading with connection config
        // This requires the connection config to be passed in
    }

    fn populate_from_loaded_columns(&self, window: &mut Window, cx: &mut App) {
        let columns = self.loaded_columns.read().unwrap().clone();

        if columns.is_empty() {
            self.status_msg.update(cx, |s, cx| {
                *s = "No columns to load".to_string();
                cx.notify();
            });
            return;
        }

        // Clear existing fields
        self.fields.write().unwrap().clear();

        let mut next_id_val = self.next_id.write().unwrap();
        let mut fields_vec = self.fields.write().unwrap();

        for column in columns {
            let field_id = *next_id_val;
            *next_id_val += 1;

            let name = cx.new(|_| column.name.clone());
            let data_type = cx.new(|_| column.data_type.clone());
            let nullable = cx.new(|_| column.is_nullable);

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

            fields_vec.push(FieldRow {
                id: field_id,
                name,
                data_type,
                nullable,
                name_input,
                type_input,
            });
        }

        drop(fields_vec);
        drop(next_id_val);

        self.status_msg.update(cx, |s, cx| {
            *s = format!("Loaded {} existing columns", self.fields.read().unwrap().len());
            cx.notify();
        });
    }

    fn add_field(&self, window: &mut Window, cx: &mut App) {
        let mut next_id_val = self.next_id.write().unwrap();
        let field_id = *next_id_val;
        *next_id_val += 1;
        drop(next_id_val);

        let name = cx.new(|_| String::new());
        let data_type = cx.new(|_| "VARCHAR(255)".to_string());
        let nullable = cx.new(|_| true);

        let name_input = cx.new(|cx| InputState::new(window, cx).placeholder("field_name"));
        let type_input = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder("VARCHAR(255)");
            input.set_value("VARCHAR(255)".to_string(), window, cx);
            input
        });

        let mut fields_vec = self.fields.write().unwrap();
        fields_vec.push(FieldRow {
            id: field_id,
            name,
            data_type,
            nullable,
            name_input,
            type_input,
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

        let status_msg = self.status_msg.clone();
        let _status_msg = self.status_msg.clone();
        let _global_state = cx.global::<GlobalDbState>().clone();
        let _database_name = self.database_name.clone();
        let _table_name = self.table_name.clone();

        // TODO: Implement save with connection config
        self.status_msg.update(cx, |s, cx| {
            *s = "Save not yet implemented with new connection pool".to_string();
            cx.notify();
        });
    }
}

impl TabContent for TableStructureTabContent {
    fn title(&self) -> SharedString {
        format!("{} - Structure", self.table_name).into()
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
        let columns_loaded = *self.columns_loaded.read().unwrap();
        let has_loaded_columns = !self.loaded_columns.read().unwrap().is_empty();
        let should_show_load_button = columns_loaded && has_loaded_columns && fields_vec.is_empty();

        let mut toolbar = h_flex()
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
                    .child(format!("Table Structure: {}", self.table_name)),
            )
            .child(div().flex_1());

        // Conditionally add Load Columns button
        if should_show_load_button {
            toolbar = toolbar.child(
                Button::new("load-columns")
                    .with_size(Size::Small)
                    .outline()
                    .label("Load Columns")
                    .icon(IconName::ArrowDown)
                    .on_click({
                        let this = self.clone();
                        move |_e, w, cx| {
                            this.populate_from_loaded_columns(w, cx);
                        }
                    }),
            );
        }

        toolbar = toolbar
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
            );

        v_flex()
            .size_full()
            .gap_2()
            .child(toolbar)
            .child(
                // Fields list
                v_flex()
                    .flex_1()
                    .gap_2()
                    .p_4()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
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
        TabContentType::TableForm(self.table_name.clone())
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
            fields: self.fields.clone(),
            next_id: self.next_id.clone(),
            status_msg: self.status_msg.clone(),
            columns_loaded: self.columns_loaded.clone(),
            loaded_columns: self.loaded_columns.clone(),
            focus_handle: self.focus_handle.clone(),
        }
    }
}

impl EventEmitter<PanelEvent> for TableStructureTabContent {}

impl Render for TableStructureTabContent {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(self.render_content(window, cx))
    }
}

impl Focusable for TableStructureTabContent {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for TableStructureTabContent {
    fn panel_name(&self) -> &'static str {
        "TableStructure"
    }

    fn tab_name(&self, _cx: &App) -> Option<SharedString> {
        Some(TabContent::title(self))
    }

    fn title(&self, _window: &Window, _cx: &App) -> AnyElement {
        h_flex()
            .items_center()
            .gap_2()
            .child(IconName::Settings)
            .child(TabContent::title(self))
            .into_any_element()
    }

    fn title_style(&self, _cx: &App) -> Option<TitleStyle> {
        None
    }

    fn title_suffix(&self, _window: &mut Window, _cx: &mut App) -> Option<AnyElement> {
        None
    }

    fn closable(&self, _cx: &App) -> bool {
        true
    }

    fn zoomable(&self, _cx: &App) -> Option<PanelControl> {
        None
    }

    fn visible(&self, _cx: &App) -> bool {
        true
    }

    fn set_active(&mut self, _active: bool, _window: &mut Window, _cx: &mut App) {
        // No special handling needed
    }

    fn set_zoomed(&mut self, _zoomed: bool, _window: &mut Window, _cx: &mut App) {
        // No special handling needed
    }

    fn on_added_to(&mut self, _tab_panel: WeakEntity<TabPanel>, _window: &mut Window, _cx: &mut App) {
        // No special handling needed
    }

    fn on_removed(&mut self, _window: &mut Window, _cx: &mut App) {
        // No special handling needed
    }

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

// ============================================================================
// Helper Types
// ============================================================================

pub struct ResultsDelegate {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<String>>,
}

impl TableDelegate for ResultsDelegate {
    fn columns_count(&self, _cx: &App) -> usize {
        self.columns.len()
    }
    fn rows_count(&self, _cx: &App) -> usize {
        self.rows.len()
    }
    fn column(&self, col_ix: usize, _cx: &App) -> &Column {
        &self.columns[col_ix]
    }
    fn render_td(
        &self,
        row: usize,
        col: usize,
        _window: &mut Window,
        _cx: &mut App,
    ) -> impl IntoElement {
        self.rows
            .get(row)
            .and_then(|r| r.get(col))
            .cloned()
            .unwrap_or_default()
    }
}

#[derive(Clone)]
pub struct DelegateWrapper {
    pub inner: Arc<std::sync::RwLock<ResultsDelegate>>,
}

impl TableDelegate for DelegateWrapper {
    fn columns_count(&self, _cx: &App) -> usize {
        self.inner.read().unwrap().columns.len()
    }
    fn rows_count(&self, _cx: &App) -> usize {
        self.inner.read().unwrap().rows.len()
    }
    fn column(&self, col_ix: usize, _cx: &App) -> &Column {
        unsafe { &*(&self.inner.read().unwrap().columns[col_ix] as *const Column) }
    }
    fn render_td(
        &self,
        row: usize,
        col: usize,
        _window: &mut Window,
        _cx: &mut App,
    ) -> impl IntoElement {
        self.inner
            .read()
            .unwrap()
            .rows
            .get(row)
            .and_then(|r| r.get(col))
            .cloned()
            .unwrap_or_default()
    }
}
