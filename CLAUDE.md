# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

One-Hub is a database management GUI application built with Rust and GPUI (a GPU-accelerated UI framework). It provides a multi-database client supporting MySQL and PostgreSQL with SQL editing, syntax highlighting, and database exploration features.

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
This is a Cargo workspace with three members:
- Root crate: `one-hub` (main application)
- `crates/assets` - Embedded SVG icons and assets using rust-embed
- `crates/core` - Currently empty, intended for core logic

## Architecture

### Database Plugin System

The application uses a plugin architecture to support multiple database types. Key components:

**DatabasePlugin Trait** (`src/db/db_plugin.rs`):
- All database operations are abstracted through the `DatabasePlugin` trait
- Plugins are stateless and accept connection parameters for each operation
- Each plugin implements database-specific SQL generation and tree-building
- Current implementations: `MySqlPlugin`, `PostgresPlugin`

**DbManager** (`src/db/db_manager.rs`):
- Factory for creating database plugin instances
- Maintains `GlobalDbState` (accessed via GPUI's global state)
- Provides `get_plugin(&DatabaseType)` to retrieve appropriate plugin

**Connection Management**:
- `DbConnection` trait defines connection interface
- Connection pooling handled per database type (MySQL/PostgreSQL specific implementations)
- Connections use SQLx with Tokio async runtime
- Health checks via `ping()` method

### Async Runtime

**Critical**: This app bridges GPUI (which uses smol) and database operations (which use Tokio):
- Database operations use a global Tokio runtime: `TOKIO_RUNTIME` in `src/db/db_runtime.rs`
- Use `TOKIO_RUNTIME.spawn()` or `TOKIO_RUNTIME.block_on()` for database tasks
- GPUI UI operations use the framework's built-in async executor

### UI Architecture

**Main Components**:
- `MainWorkspace` (`src/workspace.rs`) - Root UI container with dock system
- Uses `gpui-component` library's `DockArea` for panel management
- Layout persistence in `onehub.json` (debug) or platform-specific config

**Panel Structure**:
- Left: `DbTreeView` - Database tree navigation
- Center: `SqlEditorPanel` - SQL editor with syntax highlighting
- Bottom/Right: Configurable panels

**Tree View** (`src/db_tree_view.rs`):
- Lazy-loading hierarchical tree of database objects
- Node types defined in `DbNodeType` enum
- Uses plugin's `load_node_children()` and `build_database_tree()` methods

### SQL Execution

**SqlExecutor** (`src/db/sql_executor.rs`):
- Handles query execution and result formatting
- Returns `SqlResult` with columns, rows, affected rows, and timing
- Supports both single queries and multi-statement scripts

**Execution Flow**:
1. SQL entered in `SqlEditorPanel`
2. Submitted to plugin's `execute_script()` or `execute_query()`
3. Results rendered in result grid/table

### Storage Layer

**Connection Persistence** (`src/storage/`):
- Generic `Storage<T>` trait for CRUD operations
- `SqliteStorage` implementation stores connection configurations
- Database stored at `~/.config/one-hub/one-hub.db` (macOS/Linux) or `%APPDATA%/one-hub/one-hub.db` (Windows)
- `ConnectionStore` provides high-level API for managing saved connections

## Key Design Patterns

### SQL Generation vs Execution
Database plugins follow a two-phase approach:
1. **Generation**: `generate_*_sql()` methods create SQL strings for user review
2. **Execution**: User confirms, then SQL is executed via `execute_script()`

This allows users to review DDL statements before execution.

### Node IDs and Tree Navigation
Tree nodes use hierarchical IDs:
- Format: `<connection_id>:<database>:<folder_type>:<object_name>`
- Example: `conn_mysql:mydb:table_folder:users`
- IDs are used for lazy loading and context tracking

### Plugin Method Signatures
Most plugin methods are async and accept `&dyn DbConnection` rather than storing connection state, enabling:
- Connection pooling
- Connection switching
- Stateless plugin instances

## Database-Specific Notes

### MySQL (`src/db/mysql/`)
- Uses backticks for identifier quoting
- Connection string format: `mysql://user:pass@host:port/database`

### PostgreSQL (`src/db/postgres/`)
- Uses double quotes for identifier quoting
- Supports sequences (auto-increment)
- Connection string format: `postgres://user:pass@host:port/database`

## Adding New Database Support

To add a new database type:
1. Add variant to `DatabaseType` enum in `src/db/db_manager.rs`
2. Create module in `src/db/<dbname>/`
3. Implement `DatabasePlugin` trait
4. Implement `DbConnection` trait for connection type
5. Register plugin in `DbManager::get_plugin()`

## Important Dependencies

- `gpui` - GPU-accelerated UI framework
- `gpui-component` - UI component library (dock, buttons, etc.)
- `sqlx` - Database driver with async support
- `tokio` - Async runtime for database operations
- `rust-embed` - Compile-time asset embedding
- `serde/serde_json` - Serialization for config/state
