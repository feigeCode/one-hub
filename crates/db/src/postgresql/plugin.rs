use anyhow::Result;
use std::collections::HashMap;
use crate::connection::{DbConnection, DbError};
use crate::types::*;
use crate::plugin::DatabasePlugin;
use crate::postgresql::connection::PostgresDbConnection;
use crate::executor::{ExecOptions, SqlResult, ExecResult};

/// PostgreSQL database plugin implementation (stateless)
pub struct PostgresPlugin;

impl PostgresPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl DatabasePlugin for PostgresPlugin {
    fn name(&self) -> DatabaseType {
        DatabaseType::PostgreSQL
    }
    async fn create_connection(&self, config: DbConnectionConfig) -> Result<Box<dyn DbConnection + Send + Sync>, DbError> {
        let mut conn = PostgresDbConnection::new(config);
        conn.connect().await?;
        Ok(Box::new(conn))
    }

    // === Database/Schema Level Operations ===

    async fn list_databases(&self, connection: &dyn DbConnection) -> Result<Vec<String>> {
        let result = connection.query(
            "SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY datname",
            None,
            ExecOptions::default()
        ).await.map_err(|e| anyhow::anyhow!("Failed to list databases: {}", e))?;

        if let SqlResult::Query(query_result) = result {
            Ok(query_result.rows.iter()
                .filter_map(|row| row.first().and_then(|v| v.clone()))
                .collect())
        } else {
            Err(anyhow::anyhow!("Unexpected result type"))
        }
    }

    fn generate_create_database_sql(&self, request: &crate::types::CreateDatabaseRequest) -> Result<String> {
        let mut sql = format!("CREATE DATABASE \"{}\"", request.database_name);
        if let Some(cs) = &request.charset {
            sql.push_str(&format!(" ENCODING '{}'", cs));
        }
        if let Some(col) = &request.collation {
            sql.push_str(&format!(" LC_COLLATE '{}'", col));
        }
        Ok(sql)
    }

    fn generate_drop_database_sql(&self, request: &crate::types::DropDatabaseRequest) -> Result<String> {
        let sql = if request.if_exists {
            format!("DROP DATABASE IF EXISTS \"{}\"", request.database_name)
        } else {
            format!("DROP DATABASE \"{}\"", request.database_name)
        };
        Ok(sql)
    }

    fn generate_alter_database_sql(&self, request: &crate::types::AlterDatabaseRequest) -> Result<String> {
        // PostgreSQL doesn't support altering database encoding/collation after creation
        Err(anyhow::anyhow!("PostgreSQL does not support altering database encoding/collation"))
    }

    // === Table Operations ===

    async fn list_tables(&self, connection: &dyn DbConnection, database: &str) -> Result<Vec<String>> {
        let sql = "SELECT tablename FROM pg_tables WHERE schemaname = 'public' ORDER BY tablename";
        let result = connection.query(sql, None, ExecOptions::default())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list tables: {}", e))?;

        if let SqlResult::Query(query_result) = result {
            Ok(query_result.rows.iter()
                .filter_map(|row| row.first().and_then(|v| v.clone()))
                .collect())
        } else {
            Err(anyhow::anyhow!("Unexpected result type"))
        }
    }

    async fn list_columns(&self, connection: &dyn DbConnection, database: &str, table: &str) -> Result<Vec<ColumnInfo>> {
        let sql = format!(
            "SELECT column_name, data_type, is_nullable, column_default, \
             (SELECT COUNT(*) FROM information_schema.key_column_usage kcu \
              WHERE kcu.table_name = c.table_name AND kcu.column_name = c.column_name \
              AND kcu.table_schema = 'public' AND EXISTS \
              (SELECT 1 FROM information_schema.table_constraints tc \
               WHERE tc.constraint_name = kcu.constraint_name AND tc.constraint_type = 'PRIMARY KEY')) > 0 AS is_primary \
             FROM information_schema.columns c \
             WHERE table_schema = 'public' AND table_name = '{}' \
             ORDER BY ordinal_position",
            table
        );

        let result = connection.query(&sql, None, ExecOptions::default())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list columns: {}", e))?;

        if let SqlResult::Query(query_result) = result {
            Ok(query_result.rows.iter().map(|row| {
                ColumnInfo {
                    name: row.get(0).and_then(|v| v.clone()).unwrap_or_default(),
                    data_type: row.get(1).and_then(|v| v.clone()).unwrap_or_default(),
                    is_nullable: row.get(2).and_then(|v| v.clone()).map(|v| v == "YES").unwrap_or(true),
                    is_primary_key: row.get(4).and_then(|v| v.clone()).map(|v| v == "t" || v == "true" || v == "1").unwrap_or(false),
                    default_value: row.get(3).and_then(|v| v.clone()),
                    comment: None,
                }
            }).collect())
        } else {
            Err(anyhow::anyhow!("Unexpected result type"))
        }
    }

    async fn list_indexes(&self, connection: &dyn DbConnection, database: &str, table: &str) -> Result<Vec<IndexInfo>> {
        let sql = format!(
            "SELECT i.relname AS index_name, \
             a.attname AS column_name, \
             ix.indisunique AS is_unique \
             FROM pg_class t \
             JOIN pg_index ix ON t.oid = ix.indrelid \
             JOIN pg_class i ON i.oid = ix.indexrelid \
             JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = ANY(ix.indkey) \
             WHERE t.relname = '{}' AND t.relkind = 'r' \
             ORDER BY i.relname, a.attnum",
            table
        );

        let result = connection.query(&sql, None, ExecOptions::default())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list indexes: {}", e))?;

        if let SqlResult::Query(query_result) = result {
            let mut indexes: HashMap<String, IndexInfo> = HashMap::new();

            for row in query_result.rows {
                let index_name = row.get(0).and_then(|v| v.clone()).unwrap_or_default();
                let column_name = row.get(1).and_then(|v| v.clone()).unwrap_or_default();
                let is_unique = row.get(2).and_then(|v| v.clone()).map(|v| v == "t" || v == "true").unwrap_or(false);

                indexes.entry(index_name.clone())
                    .or_insert_with(|| IndexInfo {
                        name: index_name,
                        columns: Vec::new(),
                        is_unique,
                        index_type: Some("btree".to_string()),
                    })
                    .columns.push(column_name);
            }

            Ok(indexes.into_values().collect())
        } else {
            Err(anyhow::anyhow!("Unexpected result type"))
        }
    }

    fn generate_create_table_sql(&self, request: &crate::types::CreateTableRequest) -> Result<String> {
        use crate::plugin::DatabasePlugin;

        let column_defs: Vec<String> = request.columns.iter().map(|col| {
            self.build_column_definition(col, true)
        }).collect();

        let if_not_exists = if request.if_not_exists { "IF NOT EXISTS " } else { "" };
        let sql = format!("CREATE TABLE {}\"{}\" ({})",
            if_not_exists,
            request.table_name,
            column_defs.join(", ")
        );
        Ok(sql)
    }

    fn generate_drop_table_sql(&self, request: &crate::types::DropTableRequest) -> Result<String> {
        let sql = if request.if_exists {
            format!("DROP TABLE IF EXISTS \"{}\"", request.table_name)
        } else {
            format!("DROP TABLE \"{}\"", request.table_name)
        };
        Ok(sql)
    }

    fn generate_rename_table_sql(&self, request: &crate::types::RenameTableRequest) -> Result<String> {
        let sql = format!("ALTER TABLE \"{}\" RENAME TO \"{}\"",
            request.old_table_name,
            request.new_table_name
        );
        Ok(sql)
    }

    fn generate_truncate_table_sql(&self, request: &crate::types::TruncateTableRequest) -> Result<String> {
        let sql = format!("TRUNCATE TABLE \"{}\"", request.table_name);
        Ok(sql)
    }

    fn generate_add_column_sql(&self, request: &crate::types::AddColumnRequest) -> Result<String> {
        use crate::plugin::DatabasePlugin;

        let col_def = self.build_column_definition(&request.column, false);
        let sql = format!("ALTER TABLE \"{}\" ADD COLUMN \"{}\" {}",
            request.table_name,
            request.column.name,
            col_def
        );
        Ok(sql)
    }

    fn generate_drop_column_sql(&self, request: &crate::types::DropColumnRequest) -> Result<String> {
        let sql = format!("ALTER TABLE \"{}\" DROP COLUMN \"{}\"",
            request.table_name,
            request.column_name
        );
        Ok(sql)
    }

    fn generate_modify_column_sql(&self, request: &crate::types::ModifyColumnRequest) -> Result<String> {
        // PostgreSQL requires separate ALTER statements for type and nullability
        let mut sqls = Vec::new();

        sqls.push(format!("ALTER TABLE \"{}\" ALTER COLUMN \"{}\" TYPE {}",
            request.table_name,
            request.column.name,
            request.column.data_type
        ));

        if request.column.is_nullable {
            sqls.push(format!("ALTER TABLE \"{}\" ALTER COLUMN \"{}\" DROP NOT NULL",
                request.table_name,
                request.column.name
            ));
        } else {
            sqls.push(format!("ALTER TABLE \"{}\" ALTER COLUMN \"{}\" SET NOT NULL",
                request.table_name,
                request.column.name
            ));
        }

        if let Some(default) = &request.column.default_value {
            sqls.push(format!("ALTER TABLE \"{}\" ALTER COLUMN \"{}\" SET DEFAULT {}",
                request.table_name,
                request.column.name,
                default
            ));
        }

        Ok(sqls.join(";\n"))
    }

    // === Index Operations ===

    fn generate_create_index_sql(&self, request: &crate::types::CreateIndexRequest) -> Result<String> {
        let index_type = if request.index.is_unique { "UNIQUE " } else { "" };
        let columns = request.index.columns.iter().map(|c| format!("\"{}\"", c)).collect::<Vec<_>>().join(", ");
        let sql = format!("CREATE {}INDEX \"{}\" ON \"{}\" ({})",
            index_type,
            request.index.name,
            request.table_name,
            columns
        );
        Ok(sql)
    }

    fn generate_drop_index_sql(&self, request: &crate::types::DropIndexRequest) -> Result<String> {
        let sql = format!("DROP INDEX \"{}\"", request.index_name);
        Ok(sql)
    }

    // === View Operations ===

    async fn list_views(&self, connection: &dyn DbConnection, database: &str) -> Result<Vec<ViewInfo>> {
        let sql = "SELECT table_name, view_definition FROM information_schema.views WHERE table_schema = 'public' ORDER BY table_name";

        let result = connection.query(sql, None, ExecOptions::default())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list views: {}", e))?;

        if let SqlResult::Query(query_result) = result {
            Ok(query_result.rows.iter().map(|row| {
                ViewInfo {
                    name: row.get(0).and_then(|v| v.clone()).unwrap_or_default(),
                    definition: row.get(1).and_then(|v| v.clone()),
                    comment: None,
                }
            }).collect())
        } else {
            Err(anyhow::anyhow!("Unexpected result type"))
        }
    }

    fn generate_create_view_sql(&self, request: &crate::types::CreateViewRequest) -> Result<String> {
        let sql = if request.or_replace {
            format!("CREATE OR REPLACE VIEW \"{}\" AS {}",
                request.view_name,
                request.definition
            )
        } else {
            format!("CREATE VIEW \"{}\" AS {}",
                request.view_name,
                request.definition
            )
        };
        Ok(sql)
    }

    fn generate_drop_view_sql(&self, request: &crate::types::DropViewRequest) -> Result<String> {
        let sql = if request.if_exists {
            format!("DROP VIEW IF EXISTS \"{}\"", request.view_name)
        } else {
            format!("DROP VIEW \"{}\"", request.view_name)
        };
        Ok(sql)
    }

    // === Function Operations ===

    async fn list_functions(&self, connection: &dyn DbConnection, database: &str) -> Result<Vec<FunctionInfo>> {
        let sql = "SELECT routine_name, data_type FROM information_schema.routines WHERE routine_schema = 'public' AND routine_type = 'FUNCTION' ORDER BY routine_name";

        let result = connection.query(sql, None, ExecOptions::default())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list functions: {}", e))?;

        if let SqlResult::Query(query_result) = result {
            Ok(query_result.rows.iter().map(|row| {
                FunctionInfo {
                    name: row.get(0).and_then(|v| v.clone()).unwrap_or_default(),
                    return_type: row.get(1).and_then(|v| v.clone()),
                    parameters: Vec::new(),
                    definition: None,
                    comment: None,
                }
            }).collect())
        } else {
            Err(anyhow::anyhow!("Unexpected result type"))
        }
    }

    fn generate_create_function_sql(&self, request: &crate::types::CreateFunctionRequest) -> Result<String> {
        // For functions, the definition should contain the complete CREATE FUNCTION statement
        Ok(request.definition.clone())
    }

    fn generate_drop_function_sql(&self, request: &crate::types::DropFunctionRequest) -> Result<String> {
        let sql = if request.if_exists {
            format!("DROP FUNCTION IF EXISTS \"{}\"", request.function_name)
        } else {
            format!("DROP FUNCTION \"{}\"", request.function_name)
        };
        Ok(sql)
    }

    // === Procedure Operations ===

    async fn list_procedures(&self, connection: &dyn DbConnection, database: &str) -> Result<Vec<FunctionInfo>> {
        let sql = "SELECT routine_name FROM information_schema.routines WHERE routine_schema = 'public' AND routine_type = 'PROCEDURE' ORDER BY routine_name";

        let result = connection.query(sql, None, ExecOptions::default())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list procedures: {}", e))?;

        if let SqlResult::Query(query_result) = result {
            Ok(query_result.rows.iter().map(|row| {
                FunctionInfo {
                    name: row.get(0).and_then(|v| v.clone()).unwrap_or_default(),
                    return_type: None,
                    parameters: Vec::new(),
                    definition: None,
                    comment: None,
                }
            }).collect())
        } else {
            Err(anyhow::anyhow!("Unexpected result type"))
        }
    }

    fn generate_create_procedure_sql(&self, request: &crate::types::CreateProcedureRequest) -> Result<String> {
        // For procedures, the definition should contain the complete CREATE PROCEDURE statement
        Ok(request.definition.clone())
    }

    fn generate_drop_procedure_sql(&self, request: &crate::types::DropProcedureRequest) -> Result<String> {
        let sql = if request.if_exists {
            format!("DROP PROCEDURE IF EXISTS \"{}\"", request.procedure_name)
        } else {
            format!("DROP PROCEDURE \"{}\"", request.procedure_name)
        };
        Ok(sql)
    }

    // === Trigger Operations ===

    async fn list_triggers(&self, connection: &dyn DbConnection, database: &str) -> Result<Vec<TriggerInfo>> {
        let sql = "SELECT trigger_name, event_object_table, event_manipulation, action_timing \
                   FROM information_schema.triggers \
                   WHERE trigger_schema = 'public' \
                   ORDER BY trigger_name";

        let result = connection.query(sql, None, ExecOptions::default())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list triggers: {}", e))?;

        if let SqlResult::Query(query_result) = result {
            Ok(query_result.rows.iter().map(|row| {
                TriggerInfo {
                    name: row.get(0).and_then(|v| v.clone()).unwrap_or_default(),
                    table_name: row.get(1).and_then(|v| v.clone()).unwrap_or_default(),
                    event: row.get(2).and_then(|v| v.clone()).unwrap_or_default(),
                    timing: row.get(3).and_then(|v| v.clone()).unwrap_or_default(),
                    definition: None,
                }
            }).collect())
        } else {
            Err(anyhow::anyhow!("Unexpected result type"))
        }
    }

    fn generate_create_trigger_sql(&self, request: &crate::types::CreateTriggerRequest) -> Result<String> {
        // For triggers, the definition should contain the complete CREATE TRIGGER statement
        Ok(request.definition.clone())
    }

    fn generate_drop_trigger_sql(&self, request: &crate::types::DropTriggerRequest) -> Result<String> {
        // PostgreSQL requires table name for DROP TRIGGER
        // Since we don't have it in the request, we'll return an error
        Err(anyhow::anyhow!("PostgreSQL requires table name for DROP TRIGGER. Please use raw SQL with format: DROP TRIGGER trigger_name ON table_name"))
    }

    // === Sequence Operations ===

    async fn list_sequences(&self, connection: &dyn DbConnection, database: &str) -> Result<Vec<SequenceInfo>> {
        let sql = "SELECT sequence_name, start_value::bigint, increment::bigint, min_value::bigint, max_value::bigint \
                   FROM information_schema.sequences \
                   WHERE sequence_schema = 'public' \
                   ORDER BY sequence_name";

        let result = connection.query(sql, None, ExecOptions::default())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list sequences: {}", e))?;

        if let SqlResult::Query(query_result) = result {
            Ok(query_result.rows.iter().map(|row| {
                SequenceInfo {
                    name: row.get(0).and_then(|v| v.clone()).unwrap_or_default(),
                    start_value: row.get(1).and_then(|v| v.clone()).and_then(|s| s.parse().ok()),
                    increment: row.get(2).and_then(|v| v.clone()).and_then(|s| s.parse().ok()),
                    min_value: row.get(3).and_then(|v| v.clone()).and_then(|s| s.parse().ok()),
                    max_value: row.get(4).and_then(|v| v.clone()).and_then(|s| s.parse().ok()),
                }
            }).collect())
        } else {
            Err(anyhow::anyhow!("Unexpected result type"))
        }
    }

    fn generate_create_sequence_sql(&self, request: &crate::types::CreateSequenceRequest) -> Result<String> {
        let mut sql = format!("CREATE SEQUENCE \"{}\"", request.sequence.name);
        if let Some(start) = request.sequence.start_value {
            sql.push_str(&format!(" START {}", start));
        }
        if let Some(inc) = request.sequence.increment {
            sql.push_str(&format!(" INCREMENT {}", inc));
        }
        if let Some(min) = request.sequence.min_value {
            sql.push_str(&format!(" MINVALUE {}", min));
        }
        if let Some(max) = request.sequence.max_value {
            sql.push_str(&format!(" MAXVALUE {}", max));
        }
        Ok(sql)
    }

    fn generate_drop_sequence_sql(&self, request: &crate::types::DropSequenceRequest) -> Result<String> {
        let sql = if request.if_exists {
            format!("DROP SEQUENCE IF EXISTS \"{}\"", request.sequence_name)
        } else {
            format!("DROP SEQUENCE \"{}\"", request.sequence_name)
        };
        Ok(sql)
    }

    fn generate_alter_sequence_sql(&self, request: &crate::types::AlterSequenceRequest) -> Result<String> {
        let mut sqls = Vec::new();

        if let Some(inc) = request.sequence.increment {
            sqls.push(format!("ALTER SEQUENCE \"{}\" INCREMENT {}", request.sequence.name, inc));
        }
        if let Some(min) = request.sequence.min_value {
            sqls.push(format!("ALTER SEQUENCE \"{}\" MINVALUE {}", request.sequence.name, min));
        }
        if let Some(max) = request.sequence.max_value {
            sqls.push(format!("ALTER SEQUENCE \"{}\" MAXVALUE {}", request.sequence.name, max));
        }

        if sqls.is_empty() {
            return Err(anyhow::anyhow!("No sequence modifications specified"));
        }

        Ok(sqls.join(";\n"))
    }

    // === Query Execution ===

    async fn execute_query(
        &self,
        connection: &dyn DbConnection,
        _database: &str,
        query: &str,
        params: Option<Vec<SqlValue>>,
    ) -> Result<SqlResult> {
        connection.query(query, params, ExecOptions::default())
            .await
            .map_err(|e| anyhow::anyhow!("Query execution failed: {}", e))
    }

    async fn execute_script(
        &self,
        connection: &dyn DbConnection,
        _database: &str,
        script: &str,
        options: ExecOptions,
    ) -> Result<Vec<SqlResult>> {
        connection.execute(script, options)
            .await
            .map_err(|e| anyhow::anyhow!("Script execution failed: {}", e))
    }

    // === Database Switching ===

    async fn switch_db(&self, connection: &dyn DbConnection, database: &str) -> Result<SqlResult> {
        // PostgreSQL does not support switching database on an existing connection.
        // Return a clear Exec message instructing the caller to create a new connection.
        let message = format!(
            "PostgreSQL cannot switch database on an existing connection. Please reconnect to database '{}'.",
            database
        );
        Ok(SqlResult::Exec(ExecResult {
            sql: format!("-- switch to {}", database),
            rows_affected: 0,
            elapsed_ms: 0,
            message: Some(message),
        }))
    }
}

impl Default for PostgresPlugin {
    fn default() -> Self {
        Self::new()
    }
}
