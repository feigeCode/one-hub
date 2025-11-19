use crate::connection::{DbConnection, DbError};
use crate::plugin::DatabasePlugin;
use crate::types::{DatabaseType, DbConnectionConfig};
use crate::mysql::MySqlPlugin;
use crate::postgresql::PostgresPlugin;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use gpui::Global;

pub struct DbManager {}

impl DbManager {
    pub fn new() -> Self {
        Self {}
    }

    pub fn get_plugin(&self, db_type: &DatabaseType) -> Result<Box<dyn DatabasePlugin>, DbError> {
        match db_type {
            DatabaseType::MySQL => Ok(Box::new(MySqlPlugin::new())),
            DatabaseType::PostgreSQL => Ok(Box::new(PostgresPlugin::new())),
        }
    }
}

impl Default for DbManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for DbManager {
    fn clone(&self) -> Self {
        Self {}
    }
}

/// Connection pool manager
pub struct ConnectionPool {
    connections: Arc<RwLock<HashMap<String, ConnectionEntry>>>,
    current_connection_id: Arc<RwLock<Option<String>>>,
    current_database: Arc<RwLock<Option<String>>>,
}

struct ConnectionEntry {
    connection: Arc<RwLock<Box<dyn DbConnection + Send + Sync>>>,
    config: DbConnectionConfig,
}

impl ConnectionPool {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            current_connection_id: Arc::new(RwLock::new(None)),
            current_database: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn add_connection(&self, id: String, connection: Box<dyn DbConnection + Send + Sync>, config: DbConnectionConfig) {
        let mut connections = self.connections.write().await;
        connections.insert(id.clone(), ConnectionEntry {
            connection: Arc::new(RwLock::new(connection)),
            config,
        });

        let mut current = self.current_connection_id.write().await;
        if current.is_none() {
            *current = Some(id);
        }
    }

    pub async fn get_connection(&self, id: &str) -> Option<Arc<RwLock<Box<dyn DbConnection + Send + Sync>>>> {
        let connections = self.connections.read().await;
        connections.get(id).map(|entry| entry.connection.clone())
    }

    pub async fn get_connection_config(&self, id: &str) -> Option<DbConnectionConfig> {
        let connections = self.connections.read().await;
        connections.get(id).map(|entry| entry.config.clone())
    }

    pub async fn get_current_connection(&self) -> Option<Arc<RwLock<Box<dyn DbConnection + Send + Sync>>>> {
        let current_id = self.current_connection_id.read().await;
        if let Some(id) = current_id.as_ref() {
            self.get_connection(id).await
        } else {
            None
        }
    }

    pub async fn get_current_connection_config(&self) -> Option<DbConnectionConfig> {
        let current_id = self.current_connection_id.read().await;
        if let Some(id) = current_id.as_ref() {
            self.get_connection_config(id).await
        } else {
            None
        }
    }

    pub async fn set_current_connection(&self, id: String) {
        let mut current = self.current_connection_id.write().await;
        *current = Some(id);
    }

    pub async fn remove_connection(&self, id: &str) -> Option<(Arc<RwLock<Box<dyn DbConnection + Send + Sync>>>, DbConnectionConfig)> {
        let mut connections = self.connections.write().await;
        let removed = connections.remove(id);

        let mut current = self.current_connection_id.write().await;
        if current.as_ref() == Some(&id.to_string()) {
            *current = None;
        }

        removed.map(|entry| (entry.connection, entry.config))
    }

    pub async fn is_connected(&self) -> bool {
        let current_id = self.current_connection_id.read().await;
        current_id.is_some()
    }

    pub async fn current_database(&self) -> Option<String> {
        let db = self.current_database.read().await;
        db.clone()
    }

    pub async fn set_current_database(&self, database: Option<String>) {
        let mut db = self.current_database.write().await;
        *db = database;
    }

    pub async fn list_connection_ids(&self) -> Vec<String> {
        let connections = self.connections.read().await;
        connections.keys().cloned().collect()
    }

    pub async fn list_all_connections(&self) -> Vec<(String, DbConnectionConfig)> {
        let connections = self.connections.read().await;
        connections.iter()
            .map(|(id, entry)| (id.clone(), entry.config.clone()))
            .collect()
    }
}

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ConnectionPool {
    fn clone(&self) -> Self {
        Self {
            connections: Arc::clone(&self.connections),
            current_connection_id: Arc::clone(&self.current_connection_id),
            current_database: Arc::clone(&self.current_database),
        }
    }
}


/// Global database state - stores DbManager and ConnectionPool
#[derive(Clone)]
pub struct GlobalDbState {
    pub db_manager: DbManager,
    pub connection_pool: ConnectionPool,
}

impl GlobalDbState {
    pub fn new() -> Self {
        Self {
            db_manager: DbManager::new(),
            connection_pool: ConnectionPool::new(),
        }
    }
}

impl Default for GlobalDbState {
    fn default() -> Self {
        Self::new()
    }
}

impl Global for GlobalDbState {}
