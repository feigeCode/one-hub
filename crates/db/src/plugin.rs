use crate::connection::{DbConnection, DbError};
use crate::executor::{ExecOptions, SqlResult};
use crate::types::*;
use anyhow::Result;
use async_trait::async_trait;

/// Database plugin trait for supporting multiple database types
#[async_trait]
pub trait DatabasePlugin: Send + Sync {
    fn name(&self) -> DatabaseType;

    fn identifier_quote(&self) -> &str {
        match self.name() {
            DatabaseType::MySQL => "`",
            DatabaseType::PostgreSQL => "\"",
        }
    }

    fn quote_identifier(&self, identifier: &str) -> String {
        let quote = self.identifier_quote();
        format!("{}{}{}", quote, identifier, quote)
    }

    async fn create_connection(&self, config: DbConnectionConfig) -> Result<Box<dyn DbConnection + Send + Sync>, DbError>;

    // === Database/Schema Level Operations ===
    async fn list_databases(&self, connection: &dyn DbConnection) -> Result<Vec<String>>;
    fn generate_create_database_sql(&self, request: &CreateDatabaseRequest) -> Result<String>;
    fn generate_drop_database_sql(&self, request: &DropDatabaseRequest) -> Result<String>;
    fn generate_alter_database_sql(&self, request: &AlterDatabaseRequest) -> Result<String>;

    // === Table Operations ===
    async fn list_tables(&self, connection: &dyn DbConnection, database: &str) -> Result<Vec<String>>;
    async fn list_columns(&self, connection: &dyn DbConnection, database: &str, table: &str) -> Result<Vec<ColumnInfo>>;
    async fn list_indexes(&self, connection: &dyn DbConnection, database: &str, table: &str) -> Result<Vec<IndexInfo>>;
    
    fn generate_create_table_sql(&self, request: &CreateTableRequest) -> Result<String>;
    fn generate_drop_table_sql(&self, request: &DropTableRequest) -> Result<String>;
    fn generate_rename_table_sql(&self, request: &RenameTableRequest) -> Result<String>;
    fn generate_truncate_table_sql(&self, request: &TruncateTableRequest) -> Result<String>;
    fn generate_add_column_sql(&self, request: &AddColumnRequest) -> Result<String>;
    fn generate_drop_column_sql(&self, request: &DropColumnRequest) -> Result<String>;
    fn generate_modify_column_sql(&self, request: &ModifyColumnRequest) -> Result<String>;

    // === Index Operations ===
    fn generate_create_index_sql(&self, request: &CreateIndexRequest) -> Result<String>;
    fn generate_drop_index_sql(&self, request: &DropIndexRequest) -> Result<String>;

    // === View Operations ===
    async fn list_views(&self, connection: &dyn DbConnection, database: &str) -> Result<Vec<ViewInfo>>;
    fn generate_create_view_sql(&self, request: &CreateViewRequest) -> Result<String>;
    fn generate_drop_view_sql(&self, request: &DropViewRequest) -> Result<String>;

    // === Function Operations ===
    async fn list_functions(&self, connection: &dyn DbConnection, database: &str) -> Result<Vec<FunctionInfo>>;
    fn generate_create_function_sql(&self, request: &CreateFunctionRequest) -> Result<String>;
    fn generate_drop_function_sql(&self, request: &DropFunctionRequest) -> Result<String>;

    // === Procedure Operations ===
    async fn list_procedures(&self, connection: &dyn DbConnection, database: &str) -> Result<Vec<FunctionInfo>>;
    fn generate_create_procedure_sql(&self, request: &CreateProcedureRequest) -> Result<String>;
    fn generate_drop_procedure_sql(&self, request: &DropProcedureRequest) -> Result<String>;

    // === Trigger Operations ===
    async fn list_triggers(&self, connection: &dyn DbConnection, database: &str) -> Result<Vec<TriggerInfo>>;
    fn generate_create_trigger_sql(&self, request: &CreateTriggerRequest) -> Result<String>;
    fn generate_drop_trigger_sql(&self, request: &DropTriggerRequest) -> Result<String>;

    // === Sequence Operations ===
    async fn list_sequences(&self, connection: &dyn DbConnection, database: &str) -> Result<Vec<SequenceInfo>>;
    fn generate_create_sequence_sql(&self, request: &CreateSequenceRequest) -> Result<String>;
    fn generate_drop_sequence_sql(&self, request: &DropSequenceRequest) -> Result<String>;
    fn generate_alter_sequence_sql(&self, request: &AlterSequenceRequest) -> Result<String>;

    // === Helper Methods ===
    fn build_column_definition(&self, column: &ColumnInfo, include_name: bool) -> String {
        let mut def = String::new();

        if include_name {
            def.push_str(&self.quote_identifier(&column.name));
            def.push(' ');
        }

        def.push_str(&column.data_type);

        if !column.is_nullable {
            def.push_str(" NOT NULL");
        }

        if let Some(default) = &column.default_value {
            def.push_str(&format!(" DEFAULT {}", default));
        }

        if column.is_primary_key {
            def.push_str(" PRIMARY KEY");
        }

        if let Some(comment) = &column.comment {
            if self.name() == DatabaseType::MySQL {
                def.push_str(&format!(" COMMENT '{}'", comment.replace("'", "''")));
            }
        }

        def
    }

    // === Tree Building ===
    async fn build_database_tree(&self, connection: &dyn DbConnection, node: &DbNode) -> Result<Vec<DbNode>> {
        let mut nodes = Vec::new();
        let database = &node.name;
        let id = &node.id;

        // Tables folder
        let tables = self.list_tables(connection, database).await?;
        let table_count = tables.len();
        let mut table_folder = DbNode::new(
            format!("{}:table_folder", id),
            format!("Tables ({})", table_count),
            DbNodeType::TablesFolder,
        ).with_parent_context(id);

        if table_count > 0 {
            let children: Vec<DbNode> = tables
                .into_iter()
                .map(|table| {
                    DbNode::new(
                        format!("{}:table_folder:{}", id, table),
                        table.clone(),
                        DbNodeType::Table,
                    )
                    .with_children_flag(true)
                    .with_parent_context(format!("{}:table_folder", id))
                    .with_metadata(database)
                })
                .collect();
            table_folder.children = children;
            table_folder.has_children = true;
            table_folder.children_loaded = true;
        }
        nodes.push(table_folder);

        // Views folder
        let views = self.list_views(connection, database).await?;
        let view_count = views.len();
        if view_count > 0 {
            let mut views_folder = DbNode::new(
                format!("{}:views_folder", id),
                format!("Views ({})", view_count),
                DbNodeType::ViewsFolder,
            ).with_parent_context(id);

            let children: Vec<DbNode> = views
                .into_iter()
                .map(|view| {
                    let mut node = DbNode::new(
                        format!("{}:views_folder:{}", id, view.name),
                        view.name.clone(),
                        DbNodeType::View,
                    ).with_parent_context(format!("{}:views_folder", id));

                    if let Some(comment) = view.comment {
                        node = node.with_metadata(comment);
                    }
                    node
                })
                .collect();

            views_folder.children = children;
            views_folder.has_children = true;
            views_folder.children_loaded = true;
            nodes.push(views_folder);
        }

        Ok(nodes)
    }

    async fn load_node_children(&self, connection: &dyn DbConnection, node: &DbNode) -> Result<Vec<DbNode>> {
        let id = &node.id;
        match node.node_type {
            DbNodeType::Connection => {
                let databases = self.list_databases(connection).await?;
                Ok(databases
                    .into_iter()
                    .map(|db| {
                        DbNode::new(format!("{}:{}", &node.id, db), db.clone(), DbNodeType::Database)
                            .with_children_flag(true)
                            .with_parent_context(id)
                    })
                    .collect())
            }
            DbNodeType::Database => {
                self.build_database_tree(connection, node).await
            }
            DbNodeType::TablesFolder | DbNodeType::ViewsFolder |
            DbNodeType::FunctionsFolder | DbNodeType::ProceduresFolder |
            DbNodeType::TriggersFolder | DbNodeType::SequencesFolder => {
                if node.children_loaded {
                    Ok(node.children.clone())
                } else {
                    Ok(Vec::new())
                }
            }
            DbNodeType::Table => {
                let db = node.metadata.as_ref().unwrap();
                let table = &node.name;
                let mut children = Vec::new();

                // Columns folder
                let columns = self.list_columns(connection, db, table).await?;
                let column_count = columns.len();
                let mut columns_folder = DbNode::new(
                    format!("{}:columns_folder", id),
                    format!("Columns ({})", column_count),
                    DbNodeType::ColumnsFolder,
                ).with_parent_context(id);

                if column_count > 0 {
                    let column_nodes: Vec<DbNode> = columns
                        .into_iter()
                        .map(|col| {
                            let mut metadata = col.data_type.clone();
                            if !col.is_nullable {
                                metadata.push_str(" NOT NULL");
                            }
                            if col.is_primary_key {
                                metadata.push_str(" PRIMARY KEY");
                            }

                            DbNode::new(
                                format!("{}:columns_folder:{}", id, col.name),
                                col.name,
                                DbNodeType::Column,
                            )
                            .with_metadata(metadata)
                            .with_parent_context(format!("{}:columns_folder", id))
                        })
                        .collect();

                    columns_folder.children = column_nodes;
                    columns_folder.has_children = true;
                    columns_folder.children_loaded = true;
                }
                children.push(columns_folder);

                // Indexes folder
                let indexes = self.list_indexes(connection, db, table).await?;
                let index_count = indexes.len();
                let mut indexes_folder = DbNode::new(
                    format!("{}:indexes_folder", id),
                    format!("Indexes ({})", index_count),
                    DbNodeType::IndexesFolder,
                ).with_parent_context(id);

                if index_count > 0 {
                    let index_nodes: Vec<DbNode> = indexes
                        .into_iter()
                        .map(|idx| {
                            let metadata = format!(
                                "{} ({})",
                                if idx.is_unique { "UNIQUE" } else { "INDEX" },
                                idx.columns.join(", ")
                            );

                            DbNode::new(
                                format!("{}:indexes_folder:{}", id, idx.name),
                                idx.name,
                                DbNodeType::Index,
                            )
                            .with_metadata(metadata)
                            .with_parent_context(format!("{}:indexes_folder", id))
                        })
                        .collect();

                    indexes_folder.children = index_nodes;
                    indexes_folder.has_children = true;
                    indexes_folder.children_loaded = true;
                }
                children.push(indexes_folder);

                Ok(children)
            }
            DbNodeType::ColumnsFolder | DbNodeType::IndexesFolder => {
                if node.children_loaded {
                    Ok(node.children.clone())
                } else {
                    Ok(Vec::new())
                }
            }
            _ => Ok(Vec::new()),
        }
    }

    // === Query Execution ===
    async fn execute_query(
        &self,
        connection: &dyn DbConnection,
        database: &str,
        query: &str,
        params: Option<Vec<SqlValue>>,
    ) -> Result<SqlResult>;

    async fn execute_script(
        &self,
        connection: &dyn DbConnection,
        database: &str,
        script: &str,
        options: ExecOptions,
    ) -> Result<Vec<SqlResult>>;

    async fn switch_db(&self, connection: &dyn DbConnection, database: &str) -> Result<SqlResult>;
}
