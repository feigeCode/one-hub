use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Generic storage trait for CRUD operations
#[async_trait]
pub trait Storage<T>: Send + Sync
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync,
{
    /// Insert a new record
    async fn insert(&self, item: &T) -> Result<i64>;

    /// Update an existing record by ID
    async fn update(&self, id: i64, item: &T) -> Result<()>;

    /// Delete a record by ID
    async fn delete(&self, id: i64) -> Result<()>;

    /// Get a record by ID
    async fn get(&self, id: i64) -> Result<Option<T>>;

    /// Get all records
    async fn list(&self) -> Result<Vec<T>>;

    /// Clear all records
    async fn clear(&self) -> Result<()>;
}

/// Trait for custom queries
#[async_trait]
pub trait Queryable<T>: Send + Sync
where
    T: Send + Sync,
{
    /// Find records by a specific field value
    async fn find_by(&self, field: &str, value: &str) -> Result<Vec<T>>;

    /// Find one record by a specific field value
    async fn find_one_by(&self, field: &str, value: &str) -> Result<Option<T>>;

    /// Count records
    async fn count(&self) -> Result<i64>;

    /// Check if a record exists
    async fn exists(&self, id: i64) -> Result<bool>;
}
