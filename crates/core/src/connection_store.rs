use anyhow::Result;
use std::path::PathBuf;
use db::TOKIO_RUNTIME;
use crate::storage::{SqliteStorage, StoredConnection};

/// Connection persistence manager using SQLite
pub struct ConnectionStore {
    storage: SqliteStorage,
}

impl ConnectionStore {
    /// Create a new connection store
    pub fn new() -> Result<Self> {
        let db_path = Self::get_db_path()?;

        // Use Tokio runtime to create storage
        let storage = TOKIO_RUNTIME.block_on(async {
            SqliteStorage::new(db_path).await
        })?;

        Ok(Self { storage })
    }

    /// Get the database file path
    fn get_db_path() -> Result<PathBuf> {
        let config_dir = Self::get_config_dir()?;
        Ok(config_dir.join("one-hub.db"))
    }

    /// Get the configuration directory
    fn get_config_dir() -> Result<PathBuf> {
        // Use platform-specific config directory
        let config_dir = if cfg!(target_os = "macos") {
            dirs::home_dir()
                .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?
                .join(".config")
                .join("one-hub")
        } else if cfg!(target_os = "windows") {
            dirs::config_dir()
                .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?
                .join("one-hub")
        } else {
            dirs::home_dir()
                .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?
                .join(".config")
                .join("one-hub")
        };

        Ok(config_dir)
    }

    /// Load all saved connections
    pub fn load_connections(&self) -> Result<Vec<StoredConnection>> {
        TOKIO_RUNTIME.block_on(async {
            self.storage.load_connections().await
        })
    }

    /// Save a new connection
    pub fn save_connection(&self, stored_conn: StoredConnection) -> Result<()> {
        TOKIO_RUNTIME.block_on(async {
            self.storage.save_connection(&stored_conn).await.map(|_| ())
        })
    }

    /// Delete a connection by name
    pub fn delete_connection(&self, name: &str) -> Result<()> {
        TOKIO_RUNTIME.block_on(async {
            self.storage.delete_connection(name).await
        })
    }

    /// Get connection by name
    pub fn get_connection(&self, id: &str) -> Result<Option<StoredConnection>> {
        TOKIO_RUNTIME.block_on(async {
            self.storage.get_connection(id).await
        })
    }
}

impl Default for ConnectionStore {
    fn default() -> Self {
        Self::new().expect("Failed to create connection store")
    }
}
