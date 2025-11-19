use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::storage::models::StoredConnection;
use db::DatabaseType;

/// SQLite storage backend
pub struct SqliteStorage {
    pool: Arc<RwLock<Option<SqlitePool>>>,
    db_path: PathBuf,
}

impl SqliteStorage {
    /// Create a new SQLite storage instance
    pub async fn new(db_path: PathBuf) -> Result<Self> {
        let storage = Self {
            pool: Arc::new(RwLock::new(None)),
            db_path,
        };

        storage.init().await?;
        Ok(storage)
    }

    /// Initialize database and run migrations
    async fn init(&self) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.db_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", self.db_path.display()))?
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        // Run migrations
        self.run_migrations(&pool).await?;

        *self.pool.write().await = Some(pool);

        Ok(())
    }

    /// Run database migrations
    async fn run_migrations(&self, pool: &SqlitePool) -> Result<()> {
        // Create connections table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS connections (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                db_type TEXT NOT NULL,
                host TEXT NOT NULL,
                port INTEGER NOT NULL,
                username TEXT NOT NULL,
                password TEXT NOT NULL,
                database TEXT,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Create key-value table for generic storage
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS key_values (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                key TEXT NOT NULL UNIQUE,
                value TEXT NOT NULL,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Create index on connection name
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_connections_name ON connections(name)")
            .execute(pool)
            .await?;

        // Create index on key
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_key_values_key ON key_values(key)")
            .execute(pool)
            .await?;

        Ok(())
    }

    /// Get the database pool
    async fn get_pool(&self) -> Result<SqlitePool> {
        let pool = self.pool.read().await;
        pool.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not initialized"))
            .cloned()
    }

    /// Save a connection
    pub async fn save_connection(&self, conn: &StoredConnection) -> Result<i64> {
        let pool = self.get_pool().await?;

        let result = sqlx::query(
            r#"
            INSERT INTO connections (name, db_type, host, port, username, password, database)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(name) DO UPDATE SET
                db_type = excluded.db_type,
                host = excluded.host,
                port = excluded.port,
                username = excluded.username,
                password = excluded.password,
                database = excluded.database,
                updated_at = strftime('%s', 'now')
            "#,
        )
        .bind(&conn.name)
        .bind(format!("{:?}", conn.db_type))
        .bind(&conn.host)
        .bind(conn.port as i64)
        .bind(&conn.username)
        .bind(&conn.password)
        .bind(&conn.database)
        .execute(&pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Load all connections
    pub async fn load_connections(&self) -> Result<Vec<StoredConnection>> {
        let pool = self.get_pool().await?;

        let rows = sqlx::query(
            r#"
            SELECT id, name, db_type, host, port, username, password, database, created_at, updated_at
            FROM connections
            ORDER BY updated_at DESC
            "#,
        )
        .fetch_all(&pool)
        .await?;

        let mut connections = Vec::new();
        for row in rows {
            let db_type_str: String = row.get("db_type");
            let db_type = match db_type_str.as_str() {
                "MySQL" => DatabaseType::MySQL,
                "PostgreSQL" => DatabaseType::PostgreSQL,
                _ => DatabaseType::MySQL, // Default fallback
            };

            connections.push(StoredConnection {
                id: Some(row.get("id")),
                name: row.get("name"),
                db_type,
                host: row.get("host"),
                port: row.get::<i64, _>("port") as u16,
                username: row.get("username"),
                password: row.get("password"),
                database: row.get("database"),
                created_at: Some(row.get("created_at")),
                updated_at: Some(row.get("updated_at")),
            });
        }

        Ok(connections)
    }

    /// Delete a connection by name
    pub async fn delete_connection(&self, name: &str) -> Result<()> {
        let pool = self.get_pool().await?;

        sqlx::query("DELETE FROM connections WHERE name = ?")
            .bind(name)
            .execute(&pool)
            .await?;

        Ok(())
    }

    /// Get a connection by name
    pub async fn get_connection(&self, id: &str) -> Result<Option<StoredConnection>> {
        let pool = self.get_pool().await?;

        let row = sqlx::query(
            r#"
            SELECT id, name, db_type, host, port, username, password, database, created_at, updated_at
            FROM connections
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&pool)
        .await?;

        if let Some(row) = row {
            let db_type_str: String = row.get("db_type");
            let db_type = match db_type_str.as_str() {
                "MySQL" => DatabaseType::MySQL,
                "PostgreSQL" => DatabaseType::PostgreSQL,
                _ => DatabaseType::MySQL,
            };

            Ok(Some(StoredConnection {
                id: Some(row.get("id")),
                name: row.get("name"),
                db_type,
                host: row.get("host"),
                port: row.get::<i64, _>("port") as u16,
                username: row.get("username"),
                password: row.get("password"),
                database: row.get("database"),
                created_at: Some(row.get("created_at")),
                updated_at: Some(row.get("updated_at")),
            }))
        } else {
            Ok(None)
        }
    }

    /// Set a key-value pair
    pub async fn set_kv(&self, key: &str, value: &str) -> Result<()> {
        let pool = self.get_pool().await?;

        sqlx::query(
            r#"
            INSERT INTO key_values (key, value)
            VALUES (?, ?)
            ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at = strftime('%s', 'now')
            "#,
        )
        .bind(key)
        .bind(value)
        .execute(&pool)
        .await?;

        Ok(())
    }

    /// Get a value by key
    pub async fn get_kv(&self, key: &str) -> Result<Option<String>> {
        let pool = self.get_pool().await?;

        let row = sqlx::query("SELECT value FROM key_values WHERE key = ?")
            .bind(key)
            .fetch_optional(&pool)
            .await?;

        Ok(row.map(|r| r.get("value")))
    }

    /// Delete a key-value pair
    pub async fn delete_kv(&self, key: &str) -> Result<()> {
        let pool = self.get_pool().await?;

        sqlx::query("DELETE FROM key_values WHERE key = ?")
            .bind(key)
            .execute(&pool)
            .await?;

        Ok(())
    }

    /// List all keys
    pub async fn list_keys(&self) -> Result<Vec<String>> {
        let pool = self.get_pool().await?;

        let rows = sqlx::query("SELECT key FROM key_values ORDER BY key")
            .fetch_all(&pool)
            .await?;

        Ok(rows.into_iter().map(|r| r.get("key")).collect())
    }
}

impl Clone for SqliteStorage {
    fn clone(&self) -> Self {
        Self {
            pool: Arc::clone(&self.pool),
            db_path: self.db_path.clone(),
        }
    }
}
