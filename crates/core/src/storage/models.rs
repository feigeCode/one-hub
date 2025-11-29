use db::{DatabaseType, DbConnectionConfig};
use gpui_component::IconName;
use serde::{Deserialize, Serialize};
use crate::storage::traits::Entity;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ConnectionType {
    All,
    Database,
    SshSftp,
    Redis,
    MongoDB,
}

impl ConnectionType {
    pub fn label(&self) -> &'static str {
        match self {
            ConnectionType::All => "全部",
            ConnectionType::Database => "数据库",
            ConnectionType::SshSftp => "SSH/SFTP",
            ConnectionType::Redis => "Redis",
            ConnectionType::MongoDB => "MongoDB",
        }
    }

    pub fn icon(&self) -> IconName {
        match self {
            ConnectionType::All => IconName::Menu,
            ConnectionType::Database => IconName::Database,
            ConnectionType::SshSftp => IconName::Terminal,
            ConnectionType::Redis => IconName::Redis,
            ConnectionType::MongoDB => IconName::MongoDB,
        }
    }
}

/// Stored database connection with ID
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredConnection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub name: String,
    pub db_type: DatabaseType,
    pub connection_type: ConnectionType,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,
}

impl Entity for StoredConnection {
    fn id(&self) -> Option<i64> {
        self.id
    }

    fn created_at(&self) -> i64 {
        self.created_at.unwrap()
    }

    fn updated_at(&self) -> i64 {
        self.updated_at.unwrap()
    }
}

impl StoredConnection {
    pub fn new(db_type: DatabaseType, connection: DbConnectionConfig) -> Self {
        Self {
            id: None,
            name: connection.name,
            db_type,
            connection_type: ConnectionType::Database,
            host: connection.host,
            port: connection.port,
            username: connection.username,
            password: connection.password,
            database: connection.database,
            created_at: None,
            updated_at: None,
        }
    }

    pub fn to_db_connection(&self) -> DbConnectionConfig {
        DbConnectionConfig {
            id: self.id.unwrap().to_string(),
            database_type: self.db_type,
            name: self.name.clone(),
            host: self.host.clone(),
            port: self.port,
            username: self.username.clone(),
            password: self.password.clone(),
            database: self.database.clone(),
        }
    }
}

/// Generic key-value storage model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyValue {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub key: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,
}

impl KeyValue {
    pub fn new(key: String, value: String) -> Self {
        Self {
            id: None,
            key,
            value,
            created_at: None,
            updated_at: None,
        }
    }
}

pub fn parse_db_type(s: &str) -> DatabaseType {
    match s {
        "MySQL" => DatabaseType::MySQL,
        "PostgreSQL" => DatabaseType::PostgreSQL,
        _ => DatabaseType::MySQL,
    }
}
