use gpui::{
    div, App, AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window,
};
use gpui_component::{v_flex, ActiveTheme};
use db::types::DbNodeType;
use super::{
    ColumnListView, FunctionListView, TableListView, ViewListView,
};

/// Represents the currently selected node and what should be displayed
#[derive(Debug, Clone)]
pub enum SelectedNode {
    None,
    Database { name: String },
    TablesFolder { database: String },
    Table { database: String, name: String },
    ViewsFolder { database: String },
    View { database: String, name: String },
    FunctionsFolder { database: String },
    Function { database: String, name: String },
    ProceduresFolder { database: String },
    Procedure { database: String, name: String },
    TriggersFolder { database: String },
    SequencesFolder { database: String },
}

impl SelectedNode {
    /// Parse a node ID into a SelectedNode variant
    /// Node ID format: <connection_id>:<database>:<folder_type>:<object_name>
    pub fn from_node_id(node_id: &str, node_type: DbNodeType) -> Self {
        let parts: Vec<&str> = node_id.split(':').collect();

        match node_type {
            DbNodeType::Database => {
                if parts.len() >= 2 {
                    SelectedNode::Database {
                        name: parts[1].to_string(),
                    }
                } else {
                    SelectedNode::Database {
                        name: node_id.to_string(),
                    }
                }
            }
            DbNodeType::TablesFolder => {
                if parts.len() >= 2 {
                    SelectedNode::TablesFolder {
                        database: parts[1].to_string(),
                    }
                } else {
                    SelectedNode::None
                }
            }
            DbNodeType::Table => {
                if parts.len() >= 4 {
                    SelectedNode::Table {
                        database: parts[1].to_string(),
                        name: parts[3].to_string(),
                    }
                } else {
                    SelectedNode::None
                }
            }
            DbNodeType::ViewsFolder => {
                if parts.len() >= 2 {
                    SelectedNode::ViewsFolder {
                        database: parts[1].to_string(),
                    }
                } else {
                    SelectedNode::None
                }
            }
            DbNodeType::View => {
                if parts.len() >= 4 {
                    SelectedNode::View {
                        database: parts[1].to_string(),
                        name: parts[3].to_string(),
                    }
                } else {
                    SelectedNode::None
                }
            }
            DbNodeType::FunctionsFolder => {
                if parts.len() >= 2 {
                    SelectedNode::FunctionsFolder {
                        database: parts[1].to_string(),
                    }
                } else {
                    SelectedNode::None
                }
            }
            DbNodeType::Function => {
                if parts.len() >= 4 {
                    SelectedNode::Function {
                        database: parts[1].to_string(),
                        name: parts[3].to_string(),
                    }
                } else {
                    SelectedNode::None
                }
            }
            DbNodeType::ProceduresFolder => {
                if parts.len() >= 2 {
                    SelectedNode::ProceduresFolder {
                        database: parts[1].to_string(),
                    }
                } else {
                    SelectedNode::None
                }
            }
            DbNodeType::Procedure => {
                if parts.len() >= 4 {
                    SelectedNode::Procedure {
                        database: parts[1].to_string(),
                        name: parts[3].to_string(),
                    }
                } else {
                    SelectedNode::None
                }
            }
            DbNodeType::TriggersFolder => {
                if parts.len() >= 2 {
                    SelectedNode::TriggersFolder {
                        database: parts[1].to_string(),
                    }
                } else {
                    SelectedNode::None
                }
            }
            DbNodeType::SequencesFolder => {
                if parts.len() >= 2 {
                    SelectedNode::SequencesFolder {
                        database: parts[1].to_string(),
                    }
                } else {
                    SelectedNode::None
                }
            }
            _ => SelectedNode::None,
        }
    }
}

/// Data loaded for display
#[derive(Clone)]
enum LoadedData {
    Tables(Vec<db::types::TableInfo>),
    Columns(String, Vec<db::types::ColumnInfo>),
    Views(Vec<db::types::ViewInfo>),
    Functions(String, Vec<db::types::FunctionInfo>),
    None,
}

/// Main view component that displays object details based on the selected tree node
pub struct ObjectDetailView {
    selected_node: Entity<SelectedNode>,
    loaded_data: Entity<LoadedData>,
    config: Entity<Option<db::DbConnectionConfig>>,
}

impl ObjectDetailView {
    pub fn new(cx: &mut App) -> Self {
        let selected_node = cx.new(|_| SelectedNode::None);
        let loaded_data = cx.new(|_| LoadedData::None);
        let config = cx.new(|_| None);

        Self {
            selected_node,
            loaded_data,
            config,
        }
    }

    /// Update the selected node and load corresponding data
    pub fn set_selected_node(
        &self,
        node: SelectedNode,
        config: db::DbConnectionConfig,
        cx: &mut App,
    ) {
        self.selected_node.update(cx, |n, cx| {
            *n = node.clone();
            cx.notify();
        });

        self.config.update(cx, |c, cx| {
            *c = Some(config.clone());
            cx.notify();
        });

        self.load_data_for_node(node, config, cx);
    }

    fn load_data_for_node(
        &self,
        node: SelectedNode,
        config: db::DbConnectionConfig,
        cx: &mut App,
    ) {
        let loaded_data = self.loaded_data.clone();

        cx.spawn(async move |cx| {
            let global_state = cx.update(|cx| cx.global::<db::GlobalDbState>().clone()).ok()?;

            // Get plugin
            let plugin = global_state.db_manager.get_plugin(&config.database_type).ok()?;

            // Get connection
            let conn_arc = global_state
                .connection_pool
                .get_connection(config, &global_state.db_manager)
                .await
                .ok()?;

            let conn = conn_arc.read().await;

            match node {
                SelectedNode::Database { ref name } | SelectedNode::TablesFolder { database: ref name } => {
                    if let Ok(tables) = plugin.list_tables(&**conn, name).await {
                        cx.update(|cx| {
                            loaded_data.update(cx, |data, cx| {
                                *data = LoadedData::Tables(tables);
                                cx.notify();
                            });
                        }).ok();
                    }
                }
                SelectedNode::Table { ref database, ref name } => {
                    if let Ok(columns) = plugin.list_columns(&**conn, database, name).await {
                        let table_name = name.clone();
                        cx.update(|cx| {
                            loaded_data.update(cx, |data, cx| {
                                *data = LoadedData::Columns(table_name, columns);
                                cx.notify();
                            });
                        }).ok();
                    }
                }
                SelectedNode::ViewsFolder { ref database } => {
                    if let Ok(views) = plugin.list_views(&**conn, database).await {
                        cx.update(|cx| {
                            loaded_data.update(cx, |data, cx| {
                                *data = LoadedData::Views(views);
                                cx.notify();
                            });
                        }).ok();
                    }
                }
                SelectedNode::FunctionsFolder { ref database } => {
                    if let Ok(functions) = plugin.list_functions(&**conn, database).await {
                        cx.update(|cx| {
                            loaded_data.update(cx, |data, cx| {
                                *data = LoadedData::Functions("Functions".to_string(), functions);
                                cx.notify();
                            });
                        }).ok();
                    }
                }
                SelectedNode::ProceduresFolder { ref database } => {
                    if let Ok(procedures) = plugin.list_procedures(&**conn, database).await {
                        cx.update(|cx| {
                            loaded_data.update(cx, |data, cx| {
                                *data = LoadedData::Functions("Procedures".to_string(), procedures);
                                cx.notify();
                            });
                        }).ok();
                    }
                }
                _ => {
                    cx.update(|cx| {
                        loaded_data.update(cx, |data, cx| {
                            *data = LoadedData::None;
                            cx.notify();
                        });
                    }).ok();
                }
            }

            Some(())
        })
        .detach();
    }
}

impl Render for ObjectDetailView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let loaded_data = self.loaded_data.read(cx).clone();
        let selected_node = self.selected_node.read(cx).clone();

        div().size_full().child(match loaded_data {
            LoadedData::Tables(tables) => {
                TableListView::new(tables, window, cx).into_any_element()
            }
            LoadedData::Columns(table_name, columns) => {
                ColumnListView::new(table_name, columns, window, cx).into_any_element()
            }
            LoadedData::Views(views) => {
                ViewListView::new(views, window, cx).into_any_element()
            }
            LoadedData::Functions(title, functions) => {
                FunctionListView::new(title, functions, window, cx).into_any_element()
            }
            LoadedData::None => {
                let message = match selected_node {
                    SelectedNode::None => "Select a database object to view details",
                    _ => "Loading...",
                };

                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child(message),
                    )
                    .into_any_element()
            }
        })
    }
}
