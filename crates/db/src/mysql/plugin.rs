use crate::connection::{DbConnection, DbError};
use crate::executor::{ExecOptions, ExecResult, SqlResult};
use crate::mysql::connection::MysqlDbConnection;
use crate::plugin::DatabasePlugin;
use crate::types::*;
use anyhow::Result;
use std::collections::HashMap;

/// MySQL database plugin implementation (stateless)
pub struct MySqlPlugin;

impl MySqlPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl DatabasePlugin for MySqlPlugin {
    fn name(&self) -> DatabaseType {
        DatabaseType::MySQL
    }

    async fn create_connection(&self, config: DbConnectionConfig) -> Result<Box<dyn DbConnection + Send + Sync>, DbError> {
        let mut conn = MysqlDbConnection::new(config);
        conn.connect().await?;
        Ok(Box::new(conn))
    }

    // === Database/Schema Level Operations ===

    async fn list_databases(&self, connection: &dyn DbConnection) -> Result<Vec<String>> {
        let result = connection.query(
            "SELECT SCHEMA_NAME FROM INFORMATION_SCHEMA.SCHEMATA ORDER BY SCHEMA_NAME",
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
        let mut sql = format!("CREATE DATABASE `{}`", request.database_name);
        if let Some(cs) = &request.charset {
            sql.push_str(&format!(" CHARACTER SET {}", cs));
        }
        if let Some(col) = &request.collation {
            sql.push_str(&format!(" COLLATE {}", col));
        }
        Ok(sql)
    }

    fn generate_drop_database_sql(&self, request: &crate::types::DropDatabaseRequest) -> Result<String> {
        let sql = if request.if_exists {
            format!("DROP DATABASE IF EXISTS `{}`", request.database_name)
        } else {
            format!("DROP DATABASE `{}`", request.database_name)
        };
        Ok(sql)
    }

    fn generate_alter_database_sql(&self, request: &crate::types::AlterDatabaseRequest) -> Result<String> {
        let mut sql = format!("ALTER DATABASE `{}`", request.database_name);
        if let Some(cs) = &request.charset {
            sql.push_str(&format!(" CHARACTER SET {}", cs));
        }
        if let Some(col) = &request.collation {
            sql.push_str(&format!(" COLLATE {}", col));
        }
        Ok(sql)
    }

    // === Table Operations ===

    async fn list_tables(&self, connection: &dyn DbConnection, database: &str) -> Result<Vec<TableInfo>> {
        // Query to get all tables with their description/metadata
        let sql = format!(
            "SELECT \
                TABLE_NAME, \
                TABLE_COMMENT, \
                ENGINE, \
                TABLE_ROWS, \
                CREATE_TIME, \
                TABLE_COLLATION \
             FROM INFORMATION_SCHEMA.TABLES \
             WHERE TABLE_SCHEMA = '{}' AND TABLE_TYPE = 'BASE TABLE' \
             ORDER BY TABLE_NAME",
            database
        );

        let result = connection.query(&sql, None, ExecOptions::default())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list tables: {}", e))?;

        if let SqlResult::Query(query_result) = result {
            let tables: Vec<TableInfo> = query_result.rows.iter().map(|row| {
                let collation = row.get(5).and_then(|v| v.clone());
                // Extract charset from collation (e.g., "utf8mb4_general_ci" -> "utf8mb4")
                let charset = collation.as_ref().and_then(|c| {
                    c.split('_').next().map(|s| s.to_string())
                });

                // Parse row count
                let row_count = row.get(3).and_then(|v| v.clone()).and_then(|s| s.parse::<i64>().ok());

                TableInfo {
                    name: row.get(0).and_then(|v| v.clone()).unwrap_or_default(),
                    comment: row.get(1).and_then(|v| v.clone()).filter(|s| !s.is_empty()),
                    engine: row.get(2).and_then(|v| v.clone()),
                    row_count,
                    create_time: row.get(4).and_then(|v| v.clone()),
                    charset,
                    collation,
                }
            }).collect();

            Ok(tables)
        } else {
            Err(anyhow::anyhow!("Unexpected result type"))
        }
    }

    async fn list_columns(&self, connection: &dyn DbConnection, database: &str, table: &str) -> Result<Vec<ColumnInfo>> {
        let sql = format!(
            "SELECT COLUMN_NAME, COLUMN_TYPE, IS_NULLABLE, COLUMN_KEY, COLUMN_DEFAULT, COLUMN_COMMENT \
             FROM INFORMATION_SCHEMA.COLUMNS \
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' \
             ORDER BY ORDINAL_POSITION",
            database, table
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
                    is_primary_key: row.get(3).and_then(|v| v.clone()).map(|v| v == "PRI").unwrap_or(false),
                    default_value: row.get(4).and_then(|v| v.clone()),
                    comment: row.get(5).and_then(|v| v.clone()),
                }
            }).collect())
        } else {
            Err(anyhow::anyhow!("Unexpected result type"))
        }
    }

    async fn list_indexes(&self, connection: &dyn DbConnection, database: &str, table: &str) -> Result<Vec<IndexInfo>> {
        let sql = format!(
            "SELECT INDEX_NAME, COLUMN_NAME, NON_UNIQUE, INDEX_TYPE \
             FROM INFORMATION_SCHEMA.STATISTICS \
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' \
             ORDER BY INDEX_NAME, SEQ_IN_INDEX",
            database, table
        );

        let result = connection.query(&sql, None, ExecOptions::default())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list indexes: {}", e))?;

        if let SqlResult::Query(query_result) = result {
            let mut indexes: HashMap<String, IndexInfo> = HashMap::new();

            for row in query_result.rows {
                let index_name = row.get(0).and_then(|v| v.clone()).unwrap_or_default();
                let column_name = row.get(1).and_then(|v| v.clone()).unwrap_or_default();
                let is_unique = row.get(2).and_then(|v| v.clone()).map(|v| v == "0").unwrap_or(false);
                let index_type = row.get(3).and_then(|v| v.clone());

                indexes.entry(index_name.clone())
                    .or_insert_with(|| IndexInfo {
                        name: index_name,
                        columns: Vec::new(),
                        is_unique,
                        index_type: index_type.clone(),
                    })
                    .columns.push(column_name);
            }

            Ok(indexes.into_values().collect())
        } else {
            Err(anyhow::anyhow!("Unexpected result type"))
        }
    }

    fn generate_create_table_sql(&self, request: &crate::types::CreateTableRequest) -> Result<String> {
        let column_defs: Vec<String> = request.columns.iter().map(|col| {
            self.build_column_definition(col, true)
        }).collect();

        let if_not_exists = if request.if_not_exists { "IF NOT EXISTS " } else { "" };
        let sql = format!("CREATE TABLE {}`{}`.`{}` ({})",
            if_not_exists,
            request.database_name,
            request.table_name,
            column_defs.join(", ")
        );
        Ok(sql)
    }

    fn generate_drop_table_sql(&self, request: &crate::types::DropTableRequest) -> Result<String> {
        let sql = if request.if_exists {
            format!("DROP TABLE IF EXISTS `{}`.`{}`", request.database_name, request.table_name)
        } else {
            format!("DROP TABLE `{}`.`{}`", request.database_name, request.table_name)
        };
        Ok(sql)
    }

    fn generate_rename_table_sql(&self, request: &crate::types::RenameTableRequest) -> Result<String> {
        let sql = format!("RENAME TABLE `{}`.`{}` TO `{}`.`{}`",
            request.database_name,
            request.old_table_name,
            request.database_name,
            request.new_table_name
        );
        Ok(sql)
    }

    fn generate_truncate_table_sql(&self, request: &crate::types::TruncateTableRequest) -> Result<String> {
        let sql = format!("TRUNCATE TABLE `{}`.`{}`", request.database_name, request.table_name);
        Ok(sql)
    }

    fn generate_add_column_sql(&self, request: &crate::types::AddColumnRequest) -> Result<String> {
        let col_def = self.build_column_definition(&request.column, false);
        let sql = format!("ALTER TABLE `{}`.`{}` ADD COLUMN `{}` {}",
            request.database_name,
            request.table_name,
            request.column.name,
            col_def
        );
        Ok(sql)
    }

    fn generate_drop_column_sql(&self, request: &crate::types::DropColumnRequest) -> Result<String> {
        let sql = format!("ALTER TABLE `{}`.`{}` DROP COLUMN `{}`",
            request.database_name,
            request.table_name,
            request.column_name
        );
        Ok(sql)
    }

    fn generate_modify_column_sql(&self, request: &crate::types::ModifyColumnRequest) -> Result<String> {
        let col_def = self.build_column_definition(&request.column, false);
        let sql = format!("ALTER TABLE `{}`.`{}` MODIFY COLUMN `{}` {}",
            request.database_name,
            request.table_name,
            request.column.name,
            col_def
        );
        Ok(sql)
    }

    // === Index Operations ===

    fn generate_create_index_sql(&self, request: &crate::types::CreateIndexRequest) -> Result<String> {
        let index_type = if request.index.is_unique { "UNIQUE" } else { "INDEX" };
        let columns = request.index.columns.iter().map(|c| format!("`{}`", c)).collect::<Vec<_>>().join(", ");
        let sql = format!("CREATE {} INDEX `{}` ON `{}`.`{}` ({})",
            index_type,
            request.index.name,
            request.database_name,
            request.table_name,
            columns
        );
        Ok(sql)
    }

    fn generate_drop_index_sql(&self, request: &crate::types::DropIndexRequest) -> Result<String> {
        let sql = format!("DROP INDEX `{}` ON `{}`.`{}`",
            request.index_name,
            request.database_name,
            request.table_name
        );
        Ok(sql)
    }

    // === View Operations ===

    async fn list_views(&self, connection: &dyn DbConnection, database: &str) -> Result<Vec<ViewInfo>> {
        let sql = format!(
            "SELECT TABLE_NAME, VIEW_DEFINITION \
             FROM INFORMATION_SCHEMA.VIEWS \
             WHERE TABLE_SCHEMA = '{}' \
             ORDER BY TABLE_NAME",
            database
        );

        let result = connection.query(&sql, None, ExecOptions::default())
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
            format!("CREATE OR REPLACE VIEW `{}`.`{}` AS {}",
                request.database_name,
                request.view_name,
                request.definition
            )
        } else {
            format!("CREATE VIEW `{}`.`{}` AS {}",
                request.database_name,
                request.view_name,
                request.definition
            )
        };
        Ok(sql)
    }

    fn generate_drop_view_sql(&self, request: &crate::types::DropViewRequest) -> Result<String> {
        let sql = if request.if_exists {
            format!("DROP VIEW IF EXISTS `{}`.`{}`", request.database_name, request.view_name)
        } else {
            format!("DROP VIEW `{}`.`{}`", request.database_name, request.view_name)
        };
        Ok(sql)
    }

    // === Function Operations ===

    async fn list_functions(&self, connection: &dyn DbConnection, database: &str) -> Result<Vec<FunctionInfo>> {
        let sql = format!(
            "SELECT ROUTINE_NAME, DTD_IDENTIFIER \
             FROM INFORMATION_SCHEMA.ROUTINES \
             WHERE ROUTINE_SCHEMA = '{}' AND ROUTINE_TYPE = 'FUNCTION' \
             ORDER BY ROUTINE_NAME",
            database
        );

        let result = connection.query(&sql, None, ExecOptions::default())
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
            format!("DROP FUNCTION IF EXISTS `{}`.`{}`", request.database_name, request.function_name)
        } else {
            format!("DROP FUNCTION `{}`.`{}`", request.database_name, request.function_name)
        };
        Ok(sql)
    }

    // === Procedure Operations ===

    async fn list_procedures(&self, connection: &dyn DbConnection, database: &str) -> Result<Vec<FunctionInfo>> {
        let sql = format!(
            "SELECT ROUTINE_NAME \
             FROM INFORMATION_SCHEMA.ROUTINES \
             WHERE ROUTINE_SCHEMA = '{}' AND ROUTINE_TYPE = 'PROCEDURE' \
             ORDER BY ROUTINE_NAME",
            database
        );

        let result = connection.query(&sql, None, ExecOptions::default())
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
            format!("DROP PROCEDURE IF EXISTS `{}`.`{}`", request.database_name, request.procedure_name)
        } else {
            format!("DROP PROCEDURE `{}`.`{}`", request.database_name, request.procedure_name)
        };
        Ok(sql)
    }

    // === Trigger Operations ===

    async fn list_triggers(&self, connection: &dyn DbConnection, database: &str) -> Result<Vec<TriggerInfo>> {
        let sql = format!(
            "SELECT TRIGGER_NAME, EVENT_OBJECT_TABLE, EVENT_MANIPULATION, ACTION_TIMING \
             FROM INFORMATION_SCHEMA.TRIGGERS \
             WHERE TRIGGER_SCHEMA = '{}' \
             ORDER BY TRIGGER_NAME",
            database
        );

        let result = connection.query(&sql, None, ExecOptions::default())
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
        let sql = if request.if_exists {
            format!("DROP TRIGGER IF EXISTS `{}`.`{}`", request.database_name, request.trigger_name)
        } else {
            format!("DROP TRIGGER `{}`.`{}`", request.database_name, request.trigger_name)
        };
        Ok(sql)
    }

    // === Sequence Operations ===
    // MySQL doesn't support sequences natively (until MySQL 8.0 which has AUTO_INCREMENT only)
    // Return empty results

    async fn list_sequences(&self, _connection: &dyn DbConnection, _database: &str) -> Result<Vec<SequenceInfo>> {
        Ok(Vec::new())
    }

    fn generate_create_sequence_sql(&self, _request: &crate::types::CreateSequenceRequest) -> Result<String> {
        Err(anyhow::anyhow!("MySQL does not support sequences"))
    }

    fn generate_drop_sequence_sql(&self, _request: &crate::types::DropSequenceRequest) -> Result<String> {
        Err(anyhow::anyhow!("MySQL does not support sequences"))
    }

    fn generate_alter_sequence_sql(&self, _request: &crate::types::AlterSequenceRequest) -> Result<String> {
        Err(anyhow::anyhow!("MySQL does not support sequences"))
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
        // MySQL supports switching database using USE statement.
        // Delegate to connection.execute so the underlying implementation can adjust its pool/context.
        let sql = format!("USE `{}`", database);
        let results = connection
            .execute(&sql, ExecOptions::default())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to switch database: {}", e))?;

        // For a single USE statement we expect exactly one Exec result.
        if let Some(result) = results.into_iter().next() {
            Ok(result)
        } else {
            Ok(SqlResult::Exec(ExecResult {
                sql,
                rows_affected: 0,
                elapsed_ms: 0,
                message: Some("Database changed".to_string()),
            }))
        }
    }

    fn get_data_types(&self) -> Vec<DataTypeInfo> {
        vec![
            // 数值类型
            DataTypeInfo::new("TINYINT", "Very small integer (-128 to 127)").with_category(DataTypeCategory::Numeric),
            DataTypeInfo::new("SMALLINT", "Small integer (-32768 to 32767)").with_category(DataTypeCategory::Numeric),
            DataTypeInfo::new("MEDIUMINT", "Medium integer (-8388608 to 8388607)").with_category(DataTypeCategory::Numeric),
            DataTypeInfo::new("INT", "Standard integer (-2147483648 to 2147483647)").with_category(DataTypeCategory::Numeric),
            DataTypeInfo::new("BIGINT", "Large integer").with_category(DataTypeCategory::Numeric),
            DataTypeInfo::new("DECIMAL(10,2)", "Fixed-point number").with_category(DataTypeCategory::Numeric),
            DataTypeInfo::new("FLOAT", "Single-precision floating-point").with_category(DataTypeCategory::Numeric),
            DataTypeInfo::new("DOUBLE", "Double-precision floating-point").with_category(DataTypeCategory::Numeric),
            
            // 字符串类型
            DataTypeInfo::new("CHAR(255)", "Fixed-length string").with_category(DataTypeCategory::String),
            DataTypeInfo::new("VARCHAR(255)", "Variable-length string").with_category(DataTypeCategory::String),
            DataTypeInfo::new("TINYTEXT", "Very small text (255 bytes)").with_category(DataTypeCategory::String),
            DataTypeInfo::new("TEXT", "Text (65,535 bytes)").with_category(DataTypeCategory::String),
            DataTypeInfo::new("MEDIUMTEXT", "Medium text (16MB)").with_category(DataTypeCategory::String),
            DataTypeInfo::new("LONGTEXT", "Large text (4GB)").with_category(DataTypeCategory::String),
            
            // 日期时间类型
            DataTypeInfo::new("DATE", "Date (YYYY-MM-DD)").with_category(DataTypeCategory::DateTime),
            DataTypeInfo::new("TIME", "Time (HH:MM:SS)").with_category(DataTypeCategory::DateTime),
            DataTypeInfo::new("DATETIME", "Date and time").with_category(DataTypeCategory::DateTime),
            DataTypeInfo::new("TIMESTAMP", "Timestamp with timezone").with_category(DataTypeCategory::DateTime),
            DataTypeInfo::new("YEAR", "Year (1901-2155)").with_category(DataTypeCategory::DateTime),
            
            // 二进制类型
            DataTypeInfo::new("BINARY(255)", "Fixed-length binary").with_category(DataTypeCategory::Binary),
            DataTypeInfo::new("VARBINARY(255)", "Variable-length binary").with_category(DataTypeCategory::Binary),
            DataTypeInfo::new("TINYBLOB", "Very small BLOB (255 bytes)").with_category(DataTypeCategory::Binary),
            DataTypeInfo::new("BLOB", "BLOB (65KB)").with_category(DataTypeCategory::Binary),
            DataTypeInfo::new("MEDIUMBLOB", "Medium BLOB (16MB)").with_category(DataTypeCategory::Binary),
            DataTypeInfo::new("LONGBLOB", "Large BLOB (4GB)").with_category(DataTypeCategory::Binary),
            
            // 其他类型
            DataTypeInfo::new("BOOLEAN", "Boolean (TINYINT(1))").with_category(DataTypeCategory::Boolean),
            DataTypeInfo::new("JSON", "JSON document").with_category(DataTypeCategory::Structured),
            DataTypeInfo::new("ENUM('value1','value2')", "Enumeration").with_category(DataTypeCategory::Other),
            DataTypeInfo::new("SET('value1','value2')", "Set of values").with_category(DataTypeCategory::Other),
        ]
    }
}

impl Default for MySqlPlugin {
    fn default() -> Self {
        Self::new()
    }
}
