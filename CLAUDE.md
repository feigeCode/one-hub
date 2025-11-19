# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

One-Hub is a modern multi-protocol database management GUI built with Rust and GPUI (GPU-accelerated UI framework). It supports MySQL, PostgreSQL, and has architectural support for SQLite, Redis, and MongoDB. The application features SQL editing with syntax highlighting, database object exploration, and data import/export capabilities.

## Build and Development Commands

### Building
- `cargo build` - Build the project in debug mode
- `cargo build --release` - Build optimized release version
- `cargo run` - Build and run the application

### Testing
- `cargo test` - Run all tests
- `cargo test <test_name>` - Run specific test
- `cargo check` - Quick syntax/type checking without building

### Workspace Structure
This is a Cargo workspace with four members:
- Root crate: `one-hub` (main application)
- `crates/db` - Database abstraction layer with plugin system
- `crates/assets` - Embedded SVG icons and assets using rust-embed
- `crates/core` - Currently empty, intended for shared core logic

## Architecture

### Database Plugin System (crates/db/)

The core architectural pattern is a **stateless plugin system** where database operations are abstracted through traits:

**DatabasePlugin Trait** (`crates/db/src/plugin.rs`):
- All database operations abstracted through this trait
- Plugins are **stateless** and accept `&dyn DbConnection` references for each operation
- Each plugin implements database-specific SQL generation and metadata queries
- Current implementations: `MySqlPlugin`, `PostgresPlugin`
- Key responsibilities:
  - List/create/drop databases, tables, views, functions, procedures, triggers
  - Generate DDL/DML SQL statements specific to each database
  - Build hierarchical tree structures for UI navigation (`build_database_tree`, `load_node_children`)
  - Execute queries and scripts with proper result formatting

**DbConnection Trait** (`crates/db/src/connection.rs`):
- Defines async connection interface: `connect`, `disconnect`, `execute`, `query`, `ping`
- Implementations use SQLx with connection pooling (MySqlPool, PgPool)
- All connections must be Send + Sync for thread-safe usage

**DbManager & ConnectionPool** (`crates/db/src/manager.rs`):
- Factory pattern for creating plugin instances via `get_plugin(&DatabaseType)`
- `ConnectionPool` manages multiple active connections by ID
- `GlobalDbState` stores pool and connection store, accessed via GPUI's global state
- Connections wrapped in `Arc<RwLock<Box<dyn DbConnection>>>` for efficient cloning and thread safety

### Critical: Async Runtime Bridge

**TOKIO_RUNTIME** (`crates/db/src/runtime.rs`):
- GPUI uses **smol** executor, but SQLx requires **Tokio** runtime
- Global Tokio runtime bridges the two: `TOKIO_RUNTIME.spawn()` or `spawn_result()`
- When writing async database code in GPUI contexts, always use `spawn_result()` helper
- Pattern:
  ```rust
  cx.spawn(|this, mut cx| async move {
      let result = spawn_result(async {
          // SQLx async operations here
      }).await;
      // Update UI with result
  })
  ```

### SQL Execution & Parsing

**SqlScriptSplitter** (`crates/db/src/executor.rs`):
- Robust SQL parsing that handles string literals (single/double/backtick quotes)
- Correctly handles comments (line: `--`, `#`; block: `/* */`)
- Splits multi-statement scripts by semicolons while respecting context

**SqlStatementClassifier**:
- Classifies statements as Query, DML, DDL, Transaction, Command, or Exec
- Used to determine execution strategy and result formatting

**SqlResult Enum**:
- `Query(QueryResult)` - SELECT results with columns/rows
- `Exec(ExecResult)` - INSERT/UPDATE/DELETE with affected row count
- `Error(String)` - Execution errors

### UI Architecture (src/)

**Main Application Flow**:
1. `main.rs` - Initializes GPUI, registers Assets, creates main window (1600x1200)
2. `onehup_app.rs` - Root application state with connection filtering and tab management
3. `app_view.rs` - Main workspace orchestrating tree view, connection form, and tab container

**Key UI Components**:
- `DbTreeView` (`src/db_tree_view.rs`) - Lazy-loading hierarchical tree
  - Maintains `loaded_children` and `loading_nodes` sets for optimization
  - Emits events: `OpenTableData`, `OpenTableStructure`, `OpenViewData`, `ConnectToConnection`, `CreateNewQuery`
  - Node IDs format: `<connection_id>:<database>:<folder_type>:<object_name>`

- `TabContainer` (`src/tab_container.rs`) - Strategy pattern for tab management
  - `TabContent` trait allows different content types (SqlEditor, TableData, TableForm, QueryResult)
  - Supports multiple tabs with active tab tracking

- `SqlEditorTabContent` (`src/sql_editor_view.rs`) - SQL editing with tree-sitter syntax highlighting
  - Database selector dropdown, execute button, multi-result tabs
  - Displays execution time and row counts

- `DbConnectionForm` (`src/db_connection_form.rs`) - Connection configuration UI
  - Supports MySQL and PostgreSQL with test connection functionality
  - Integrates with ConnectionStore for persistence

### Storage Layer (src/storage/)

**Generic Storage Abstraction**:
- `Storage<T>` trait - Async CRUD operations (insert, update, delete, get, list, clear)
- `Queryable<T>` trait - Extended queries (find_by, find_one_by, count, exists)
- `SqliteStorage` - Concrete implementation using SQLx SQLite driver

**ConnectionStore** (`src/connection_store.rs`):
- High-level API wrapping SqliteStorage
- Database location: `~/.config/one-hub/one-hub.db` (macOS/Linux) or `%APPDATA%/one-hub/one-hub.db` (Windows)
- Automatic database initialization and schema creation
- Timestamp management (created_at, updated_at)

### Data Import/Export (src/data_export.rs, src/data_import.rs)

**Export Formats**:
- CSV (RFC 4180 compliant with field escaping)
- JSON (array of objects)
- SQL (INSERT statements with configurable table name)
- Markdown (table format)
- Excel HTML/XML (SpreadsheetML)
- Word RTF (table format)

**Import Formats**:
- CSV (RFC 4180 parsing with quoted field handling)
- JSON (array of objects/arrays, NDJSON support)
- SQL (raw scripts)

**Key Functions**:
- `export_to_path()` - Write to file with directory creation
- `export_to_bytes()` - Return as UTF-8 bytes
- `import_from_csv()`, `import_from_json()`, `import_from_sql()`

## Key Design Patterns

### 1. Stateless Plugins with Connection References
Plugins don't maintain state. They accept `&dyn DbConnection` for each operation, enabling:
- Flexible connection pooling and switching
- Thread-safe connection sharing via Arc<RwLock<>>
- Easy testing and plugin isolation

### 2. Two-Phase SQL Execution
Database plugins generate SQL strings first, allowing user review before execution:
1. Call `create_table()` → returns SQL string
2. Display SQL to user
3. User confirms → execute via `execute_script()`

This is critical for DDL operations where mistakes can be destructive.

### 3. Hierarchical Node IDs for Tree Navigation
Tree nodes use structured IDs: `<connection_id>:<database>:<folder_type>:<object_name>`
- Example: `conn_mysql:mydb:table_folder:users`
- Enables efficient lazy loading and context tracking
- Folder types: `table_folder`, `view_folder`, `function_folder`, `procedure_folder`, `trigger_folder`, `sequence_folder` (PostgreSQL only)

### 4. Lazy Loading Tree with State Tracking
`DbTreeView` optimizes performance by:
- Only loading visible nodes initially
- Tracking `loaded_children` to avoid redundant queries
- Using `loading_nodes` to prevent concurrent loading of same node
- Calling `plugin.build_database_tree()` for databases, `plugin.load_node_children()` for sub-objects

### 5. Tab Strategy Pattern
Different tab content types implement `TabContent` trait:
- `SqlEditor` - SQL editing interface
- `TableData` - Data grid display
- `TableForm` - Table structure (columns, indexes, constraints)
- `QueryResult` - Query execution results
- `Custom(String)` - Extensible for future types

`TabContainer` manages all tabs uniformly without knowing specific content details.

## Database-Specific Notes

### MySQL (crates/db/src/mysql/)
- Identifier quoting: backticks (`` `table_name` ``)
- Connection: SQLx MySqlPool
- Metadata queries: `INFORMATION_SCHEMA.TABLES`, `INFORMATION_SCHEMA.COLUMNS`, `INFORMATION_SCHEMA.STATISTICS`
- Notable methods:
  - `list_tables()` - `SHOW TABLES FROM database`
  - `get_table_columns()` - Query INFORMATION_SCHEMA.COLUMNS
  - `get_table_indexes()` - Query INFORMATION_SCHEMA.STATISTICS

### PostgreSQL (crates/db/src/postgresql/)
- Identifier quoting: double quotes (`"table_name"`)
- Connection: SQLx PgPool
- Metadata queries: `pg_database`, `pg_tables`, `information_schema`
- Unique feature: Sequence support (auto-increment)
  - `list_sequences()`, `create_sequence()`, `drop_sequence()`, `alter_sequence()`
- Notable methods:
  - `list_tables()` - Query information_schema.tables
  - `list_sequences()` - Query information_schema.sequences

## Adding New Database Support

To add a new database type (e.g., SQLite, Redis, MongoDB):

1. **Add DatabaseType variant** in `crates/db/src/types.rs`:
   ```rust
   pub enum DatabaseType {
       MySQL,
       PostgreSQL,
       SQLite,  // Add new variant
   }
   ```

2. **Create plugin module** `crates/db/src/sqlite/`:
   - `mod.rs` - Module exports
   - `plugin.rs` - Implement `DatabasePlugin` trait
   - `connection.rs` - Implement `DbConnection` trait

3. **Implement DatabasePlugin** for all required methods:
   - Database operations: `list_databases()`, `create_database()`, `drop_database()`
   - Table operations: `list_tables()`, `get_table_columns()`, `get_table_indexes()`, etc.
   - View/Function/Procedure/Trigger operations as applicable
   - Tree building: `build_database_tree()`, `load_node_children()`
   - Execution: `execute_query()`, `execute_script()`

4. **Implement DbConnection** with connection pooling:
   - Use SQLx driver if available, or custom connection type
   - Implement async methods: `connect()`, `disconnect()`, `execute()`, `query()`, `ping()`

5. **Register plugin** in `DbManager::get_plugin()` (`crates/db/src/manager.rs`):
   ```rust
   match db_type {
       DatabaseType::MySQL => Box::new(MySqlPlugin),
       DatabaseType::PostgreSQL => Box::new(PostgresPlugin),
       DatabaseType::SQLite => Box::new(SqlitePlugin),  // Add case
   }
   ```

6. **Add connection form** in `src/db_connection_form.rs`:
   - Create `DbFormConfig` for the new database type
   - Define form fields (host, port, username, password, etc.)

7. **Update UI** in `src/onehup_app.rs`:
   - Add icon to Assets if needed
   - Update connection type filtering if applicable

## Important Dependencies

### UI Framework
- `gpui` 0.2.2 - GPU-accelerated UI framework (uses smol executor)
- `gpui-component` 0.4.0 - UI widgets (DockArea, Table, Tree, Select, Input, Button)
  - Enable `tree-sitter-languages` feature for SQL syntax highlighting

### Database & Async
- `sqlx` 0.8 - Async database driver (features: mysql, postgres, sqlite, chrono, bigdecimal, json)
- `tokio` 1.0 - Async runtime (features: rt-multi-thread, macros, sync, time)
- `async-trait` 0.1 - Async trait support

### Serialization & Storage
- `serde` / `serde_json` - Configuration and state serialization
- `rust-embed` 8.7.2 - Compile-time asset embedding

### Utilities
- `anyhow` - Error handling
- `dirs` 6.0 - Platform-specific directory paths
- `once_cell` / `lazy_static` - Global state initialization
- `tracing` / `tracing-subscriber` - Logging

## Common Pitfalls & Solutions

### 1. Async Runtime Mismatch
**Problem**: GPUI uses smol, SQLx uses Tokio
**Solution**: Always use `spawn_result()` from `crates/db/src/runtime.rs` when calling SQLx operations from GPUI contexts

### 2. Connection Lifetime in UI
**Problem**: Cannot hold database connection across async boundaries
**Solution**: Use `ConnectionPool` with Arc<RwLock<>> wrapping. Get connection, clone Arc, move into async block

### 3. Tree Node Loading Race Conditions
**Problem**: Multiple simultaneous expansions of same node
**Solution**: Check `loading_nodes` set before starting load, add node ID immediately

### 4. SQL Statement Splitting
**Problem**: Naive semicolon splitting breaks on strings like `INSERT INTO t VALUES ('a;b')`
**Solution**: Use `SqlScriptSplitter` which correctly handles quotes and comments

### 5. Database-Specific SQL Generation
**Problem**: Hardcoded SQL won't work across databases
**Solution**: Each plugin generates its own SQL with proper identifier quoting (backticks for MySQL, double quotes for PostgreSQL)

## Code Location Reference

When working on specific features, know where to look:

- **Database plugin logic**: `crates/db/src/<dbname>/plugin.rs`
- **Connection management**: `crates/db/src/<dbname>/connection.rs`
- **UI tree navigation**: `src/db_tree_view.rs`
- **SQL editor**: `src/sql_editor_view.rs`, `src/sql_editor.rs`
- **Tab management**: `src/tab_container.rs`, `src/tab_contents.rs`
- **Connection forms**: `src/db_connection_form.rs`
- **Persistent storage**: `src/storage/sqlite_backend.rs`
- **Import/Export**: `src/data_export.rs`, `src/data_import.rs`
- **Async runtime bridge**: `crates/db/src/runtime.rs`
- **SQL parsing**: `crates/db/src/executor.rs`
- **Type definitions**: `crates/db/src/types.rs`
