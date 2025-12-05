use std::collections::HashMap;
use crate::types::*;
use anyhow::Result;
use async_trait::async_trait;
use one_core::storage::{GlobalStorageState, StoredConnection};

// Extension trait to add query functionality to DatabasePlugin
#[async_trait]
pub trait QueryPluginExt: crate::plugin::DatabasePlugin {
    async fn build_database_tree_with_queries(
        &self,
        connection: &dyn crate::connection::DbConnection,
        node: &DbNode,
        global_storage: &GlobalStorageState,
    ) -> Result<Vec<DbNode>> {
        // First, build the regular database tree
        let mut nodes = self.build_database_tree(connection, node).await?;

        // Add the queries folder after the other folders
        let database = &node.name;
        let id = &node.id;
        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("database".to_string(), database.to_string());

        // Get query repository and list queries for this connection
        // For now, we'll use a fixed connection_id since the node's connection_id field is a string
        let query_repo = global_storage.storage.get_repo::<crate::query_model::Query>().await?;
        let connection_id = &node.connection_id; // Assuming this is the string ID of the connection
        let queries = query_repo.list_by_connection(global_storage.storage.get_pool().await?.deref(), connection_id).await?;
        
        let query_count = queries.len();
        let queries_folder = DbNode::new(
            format!("{}:queries_folder", id),
            format!("Queries ({})", query_count),
            DbNodeType::QueriesFolder,
            node.connection_id.clone()
        )
        .with_parent_context(id)
        .with_children_flag(true);

        // Add named query nodes as children
        let mut query_children = Vec::new();
        for query in queries {
            let query_node = DbNode::new(
                format!("{}:queries_folder:{}", id, query.id.unwrap_or(0)), // Using ID or 0 if not assigned yet
                query.name.clone(),
                DbNodeType::NamedQuery,
                node.connection_id.clone()
            )
            .with_parent_context(format!("{}:queries_folder", id));
            
            query_children.push(query_node);
        }

        // Update the queries folder with children
        let mut queries_folder = queries_folder;
        if !query_children.is_empty() {
            queries_folder.children = query_children;
            queries_folder.has_children = true;
            queries_folder.children_loaded = true;
        }
        
        nodes.push(queries_folder);

        Ok(nodes)
    }
}

// Blanket implementation for any type that implements DatabasePlugin
impl<T: crate::plugin::DatabasePlugin + ?Sized> QueryPluginExt for T {}