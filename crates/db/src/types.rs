use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;

/// SQL value type for parameter binding
#[derive(Debug, Clone)]
pub enum SqlValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    Json(serde_json::Value),
}

/// Database tree node types for hierarchical display
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum DbNodeType {
    Connection,
    Database,
    TablesFolder,
    Table,
    ColumnsFolder,
    Column,
    IndexesFolder,
    Index,
    ViewsFolder,
    View,
    FunctionsFolder,
    Function,
    ProceduresFolder,
    Procedure,
    TriggersFolder,
    Trigger,
    SequencesFolder,
    Sequence,
}

impl fmt::Display for DbNodeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DbNodeType::Connection => write!(f, "Connection"),
            DbNodeType::Database => write!(f, "Database"),
            DbNodeType::TablesFolder => write!(f, "Tables"),
            DbNodeType::Table => write!(f, "Table"),
            DbNodeType::ColumnsFolder => write!(f, "Columns"),
            DbNodeType::Column => write!(f, "Column"),
            DbNodeType::IndexesFolder => write!(f, "Indexes"),
            DbNodeType::Index => write!(f, "Index"),
            DbNodeType::ViewsFolder => write!(f, "Views"),
            DbNodeType::View => write!(f, "View"),
            DbNodeType::FunctionsFolder => write!(f, "Functions"),
            DbNodeType::Function => write!(f, "Function"),
            DbNodeType::ProceduresFolder => write!(f, "Procedures"),
            DbNodeType::Procedure => write!(f, "Procedure"),
            DbNodeType::TriggersFolder => write!(f, "Triggers"),
            DbNodeType::Trigger => write!(f, "Trigger"),
            DbNodeType::SequencesFolder => write!(f, "Sequences"),
            DbNodeType::Sequence => write!(f, "Sequence"),
        }
    }
}

/// Database tree node for lazy-loading hierarchical display
#[derive(Debug, Clone)]
pub struct DbNode {
    pub id: String,
    pub name: String,
    pub node_type: DbNodeType,
    pub has_children: bool,
    pub children_loaded: bool,
    pub children: Vec<DbNode>,
    pub metadata: Option<HashMap<String, String>>,
    pub connection_id: String ,
    pub parent_context: Option<String>,
}

impl PartialEq for DbNode {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for DbNode {}

impl PartialOrd for DbNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DbNode {
    fn cmp(&self, other: &Self) -> Ordering {
        let type_ordering = self.node_type.cmp(&other.node_type);
        if type_ordering != Ordering::Equal {
            return type_ordering;
        }
        let name_ordering = self.name.to_lowercase().cmp(&other.name.to_lowercase());
        if name_ordering != Ordering::Equal {
            return name_ordering;
        }
        self.id.cmp(&other.id)
    }
}

impl DbNode {
    pub fn new(id: impl Into<String>, name: impl Into<String>, node_type: DbNodeType, connection_id: String) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            node_type,
            has_children: false,
            children_loaded: false,
            children: Vec::new(),
            metadata: None,
            connection_id,
            parent_context: None,
        }
    }

    pub fn with_children_flag(mut self, has_children: bool) -> Self {
        self.has_children = has_children;
        self
    }

    pub fn with_metadata(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata = Some(metadata.into());
        self
    }

    pub fn with_parent_context(mut self, context: impl Into<String>) -> Self {
        self.parent_context = Some(context.into());
        self
    }

    pub fn sort_children(&mut self) {
        self.children.sort();
    }

    pub fn sort_children_recursive(&mut self) {
        self.children.sort();
        for child in &mut self.children {
            child.sort_children_recursive();
        }
    }
}

/// Column information
#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
    pub is_primary_key: bool,
    pub default_value: Option<String>,
    pub comment: Option<String>,
}

/// Index information
#[derive(Debug, Clone)]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub is_unique: bool,
    pub index_type: Option<String>,
}

/// Table information with description/metadata
#[derive(Debug, Clone)]
pub struct TableInfo {
    pub name: String,
    pub comment: Option<String>,
    pub engine: Option<String>,
    pub row_count: Option<i64>,
    pub create_time: Option<String>,
    pub charset: Option<String>,
    pub collation: Option<String>,
}

/// View information
#[derive(Debug, Clone)]
pub struct ViewInfo {
    pub name: String,
    pub definition: Option<String>,
    pub comment: Option<String>,
}

/// Function information
#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub name: String,
    pub return_type: Option<String>,
    pub parameters: Vec<String>,
    pub definition: Option<String>,
    pub comment: Option<String>,
}

/// Trigger information
#[derive(Debug, Clone)]
pub struct TriggerInfo {
    pub name: String,
    pub table_name: String,
    pub event: String,
    pub timing: String,
    pub definition: Option<String>,
}

/// Sequence information
#[derive(Debug, Clone)]
pub struct SequenceInfo {
    pub name: String,
    pub start_value: Option<i64>,
    pub increment: Option<i64>,
    pub min_value: Option<i64>,
    pub max_value: Option<i64>,
}

/// Data type information for table designer
#[derive(Debug, Clone)]
pub struct DataTypeInfo {
    pub name: String,
    pub description: String,
    pub category: DataTypeCategory,
}

impl DataTypeInfo {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        let name_str = name.into();
        let category = Self::infer_category(&name_str);
        Self {
            name: name_str,
            description: description.into(),
            category,
        }
    }

    pub fn with_category(mut self, category: DataTypeCategory) -> Self {
        self.category = category;
        self
    }

    fn infer_category(name: &str) -> DataTypeCategory {
        let upper = name.to_uppercase();
        if upper.contains("INT") || upper.contains("SERIAL") || upper.contains("BIGINT") || upper.contains("SMALLINT") {
            DataTypeCategory::Numeric
        } else if upper.contains("CHAR") || upper.contains("TEXT") || upper.contains("CLOB") {
            DataTypeCategory::String
        } else if upper.contains("DATE") || upper.contains("TIME") || upper.contains("TIMESTAMP") {
            DataTypeCategory::DateTime
        } else if upper.contains("BOOL") {
            DataTypeCategory::Boolean
        } else if upper.contains("BLOB") || upper.contains("BINARY") || upper.contains("BYTEA") {
            DataTypeCategory::Binary
        } else if upper.contains("JSON") || upper.contains("XML") {
            DataTypeCategory::Structured
        } else if upper.contains("DECIMAL") || upper.contains("NUMERIC") || upper.contains("FLOAT") || upper.contains("DOUBLE") || upper.contains("REAL") {
            DataTypeCategory::Numeric
        } else {
            DataTypeCategory::Other
        }
    }
}

/// Data type category for grouping
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataTypeCategory {
    Numeric,
    String,
    DateTime,
    Boolean,
    Binary,
    Structured,
    Other,
}

/// Database type enumeration
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum DatabaseType {
    MySQL,
    PostgreSQL,
}

impl DatabaseType {
    pub fn as_str(&self) -> &str {
        match self {
            DatabaseType::MySQL => "MySQL",
            DatabaseType::PostgreSQL => "PostgreSQL",
        }
    }
}

/// Connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbConnectionConfig {
    pub id: String,
    pub database_type: DatabaseType,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<i64>,
}

// === SQL Operation Request Objects ===

#[derive(Debug, Clone)]
pub struct CreateDatabaseRequest {
    pub database_name: String,
    pub charset: Option<String>,
    pub collation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DropDatabaseRequest {
    pub database_name: String,
    pub if_exists: bool,
}

#[derive(Debug, Clone)]
pub struct AlterDatabaseRequest {
    pub database_name: String,
    pub charset: Option<String>,
    pub collation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateTableRequest {
    pub database_name: String,
    pub table_name: String,
    pub columns: Vec<ColumnInfo>,
    pub if_not_exists: bool,
}

#[derive(Debug, Clone)]
pub struct DropTableRequest {
    pub database_name: String,
    pub table_name: String,
    pub if_exists: bool,
}

#[derive(Debug, Clone)]
pub struct RenameTableRequest {
    pub database_name: String,
    pub old_table_name: String,
    pub new_table_name: String,
}

#[derive(Debug, Clone)]
pub struct TruncateTableRequest {
    pub database_name: String,
    pub table_name: String,
}

#[derive(Debug, Clone)]
pub struct AddColumnRequest {
    pub database_name: String,
    pub table_name: String,
    pub column: ColumnInfo,
}

#[derive(Debug, Clone)]
pub struct DropColumnRequest {
    pub database_name: String,
    pub table_name: String,
    pub column_name: String,
}

#[derive(Debug, Clone)]
pub struct ModifyColumnRequest {
    pub database_name: String,
    pub table_name: String,
    pub column: ColumnInfo,
}

#[derive(Debug, Clone)]
pub struct CreateIndexRequest {
    pub database_name: String,
    pub table_name: String,
    pub index: IndexInfo,
}

#[derive(Debug, Clone)]
pub struct DropIndexRequest {
    pub database_name: String,
    pub table_name: String,
    pub index_name: String,
}

#[derive(Debug, Clone)]
pub struct CreateViewRequest {
    pub database_name: String,
    pub view_name: String,
    pub definition: String,
    pub or_replace: bool,
}

#[derive(Debug, Clone)]
pub struct DropViewRequest {
    pub database_name: String,
    pub view_name: String,
    pub if_exists: bool,
}

#[derive(Debug, Clone)]
pub struct CreateFunctionRequest {
    pub database_name: String,
    pub definition: String,
}

#[derive(Debug, Clone)]
pub struct DropFunctionRequest {
    pub database_name: String,
    pub function_name: String,
    pub if_exists: bool,
}

#[derive(Debug, Clone)]
pub struct CreateProcedureRequest {
    pub database_name: String,
    pub definition: String,
}

#[derive(Debug, Clone)]
pub struct DropProcedureRequest {
    pub database_name: String,
    pub procedure_name: String,
    pub if_exists: bool,
}

#[derive(Debug, Clone)]
pub struct CreateTriggerRequest {
    pub database_name: String,
    pub definition: String,
}

#[derive(Debug, Clone)]
pub struct DropTriggerRequest {
    pub database_name: String,
    pub trigger_name: String,
    pub if_exists: bool,
}

#[derive(Debug, Clone)]
pub struct CreateSequenceRequest {
    pub database_name: String,
    pub sequence: SequenceInfo,
}

#[derive(Debug, Clone)]
pub struct DropSequenceRequest {
    pub database_name: String,
    pub sequence_name: String,
    pub if_exists: bool,
}

#[derive(Debug, Clone)]
pub struct AlterSequenceRequest {
    pub database_name: String,
    pub sequence: SequenceInfo,
}
