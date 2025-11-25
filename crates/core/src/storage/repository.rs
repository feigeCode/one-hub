use anyhow::Result;
use async_trait::async_trait;
use gpui::{App, SharedString};
use sqlx::{Row, SqlitePool};
use crate::gpui_tokio::Tokio;
use crate::storage::{traits::Repository, StoredConnection};

/// Repository for StoredConnection
#[derive(Clone)]
pub struct ConnectionRepository;

impl ConnectionRepository {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Repository for ConnectionRepository {
    type Entity = StoredConnection;

    fn entity_type(&self) -> SharedString {
       SharedString::from("Connection")
    }

    async fn create_table(&self, pool: &SqlitePool) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS connections (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                db_type TEXT NOT NULL,
                connection_type TEXT NOT NULL,
                host TEXT NOT NULL,
                port INTEGER NOT NULL,
                username TEXT NOT NULL,
                password TEXT NOT NULL,
                database TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_connections_name ON connections(name)")
            .execute(pool)
            .await?;

        Ok(())
    }

    async fn insert(&self, pool: &SqlitePool, item: &mut Self::Entity) -> Result<i64> {
        let now = now();
        let result = sqlx::query(
            r#"
            INSERT INTO connections (name, db_type, connection_type, host, port, username, password, database, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&item.name)
        .bind(format!("{:?}", item.db_type))
        .bind(format!("{:?}", item.connection_type))
        .bind(&item.host)
        .bind(item.port as i64)
        .bind(&item.username)
        .bind(&item.password)
        .bind(&item.database)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;

        let id = result.last_insert_rowid();
        item.id = Some(id);
        item.created_at = Some(now);
        item.updated_at = Some(now);

        Ok(id)
    }

    async fn update(&self, pool: &SqlitePool, item: &Self::Entity) -> Result<()> {
        let id = item.id.ok_or_else(|| anyhow::anyhow!("Cannot update without ID"))?;
        let now = now();
        sqlx::query(
            r#"
            UPDATE connections 
            SET name = ?, db_type = ?, connection_type = ?, host = ?, port = ?, 
                username = ?, password = ?, database = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&item.name)
        .bind(format!("{:?}", item.db_type))
        .bind(format!("{:?}", item.connection_type))
        .bind(&item.host)
        .bind(item.port as i64)
        .bind(&item.username)
        .bind(&item.password)
        .bind(&item.database)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;

        Ok(())
    }

    async fn delete(&self, pool: &SqlitePool, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM connections WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(())
    }

    async fn get(&self, pool: &SqlitePool, id: i64) -> Result<Option<Self::Entity>> {
        let row = sqlx::query(
            r#"
            SELECT id, name, db_type, connection_type, host, port, username, password, database, created_at, updated_at
            FROM connections
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| Self::row_to_entity(&r)))
    }

    async fn list(&self, pool: &SqlitePool) -> Result<Vec<Self::Entity>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, db_type, connection_type, host, port, username, password, database, created_at, updated_at
            FROM connections
            ORDER BY updated_at DESC
            "#,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows.iter().map(|r| Self::row_to_entity(r)).collect())
    }

    async fn count(&self, pool: &SqlitePool) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM connections")
            .fetch_one(pool)
            .await?;

        Ok(row.get("count"))
    }

    async fn exists(&self, pool: &SqlitePool, id: i64) -> Result<bool> {
        let row = sqlx::query("SELECT 1 FROM connections WHERE id = ? LIMIT 1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.is_some())
    }
}

impl ConnectionRepository {
    fn row_to_entity(row: &sqlx::sqlite::SqliteRow) -> StoredConnection {
        use crate::storage::models::parse_db_type;

        let db_type_str: String = row.get("db_type");
        let conn_type_str: String = row.get("connection_type");

        StoredConnection {
            id: Some(row.get("id")),
            name: row.get("name"),
            db_type: parse_db_type(&db_type_str),
            connection_type: parse_connection_type(&conn_type_str),
            host: row.get("host"),
            port: row.get::<i64, _>("port") as u16,
            username: row.get("username"),
            password: row.get("password"),
            database: row.get("database"),
            created_at: Some(row.get("created_at")),
            updated_at: Some(row.get("updated_at")),
        }
    }
}

use crate::storage::ConnectionType;
use crate::storage::manager::{now, GlobalStorageState};

fn parse_connection_type(s: &str) -> ConnectionType {
    match s {
        "Database" => ConnectionType::Database,
        "SshSftp" => ConnectionType::SshSftp,
        "Redis" => ConnectionType::Redis,
        "MongoDB" => ConnectionType::MongoDB,
        _ => ConnectionType::Database,
    }
}

pub fn init(cx: &mut App) {
    let storage_state = cx.global::<GlobalStorageState>();
    let repo = ConnectionRepository::new();
    let result: Result<()> = Tokio::block_on(cx, async move {
        let pool = storage_state.storage.get_pool().await?;
        repo.create_table(&pool).await?;
        storage_state.storage.register(repo).await?;
        Ok(())
    });
    if let Err(e) = result {
        panic!("Failed to initialize connection repository: {}", e);
    }
}