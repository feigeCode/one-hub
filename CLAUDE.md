# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

One-Hub is a modern multi-protocol database management GUI built with Rust and GPUI (GPU-accelerated UI framework). It supports MySQL, PostgreSQL, and has architectural support for SQLite, Redis, and MongoDB. The application features:

- **Two-level tab system**: Top-level tabs for Home/Database/Settings, inner tabs for SQL editors and data views
- **DockArea workspace**: Flexible panel layout with resizable, collapsible panels
- **SQL editing with syntax highlighting**: Tree-sitter based highlighting for 20+ languages
- **Database object exploration**: Lazy-loading hierarchical tree view
- **Data import/export capabilities**: CSV, JSON, SQL, Markdown, Excel, Word formats (implemented but not yet enabled in UI)
- **Embedded UI framework**: Full gpui-component source code (~60,000 lines) for complete control

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
This is a Cargo workspace with **eight members**:
- Root crate: `one-hub` (main application)
- `crates/db` - Database abstraction layer with plugin system
- `crates/ui` - **Embedded gpui-component source code** (~60,000 lines, 64+ modules)
- `crates/macros` - Procedural macros for gpui-component
- `crates/assets` - Embedded SVG icons and assets using rust-embed
- `crates/core` - Currently empty, intended for shared core logic
- `crates/mysql`, `crates/postgresql`, `crates/sqlite` - Empty placeholders for future modularization

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

**IMPORTANT: New Architecture - gpui_tokio.rs**

**Location**: `crates/db/src/gpui_tokio.rs`

GPUI uses **smol** executor, but SQLx requires **Tokio** runtime. The new architecture bridges the two more elegantly:

**GlobalTokio State**:
- Holds a 2-worker Tokio runtime instance
- Initialized once at app startup via `db::gpui_tokio::init(cx)`

**Tokio API** (integrates with GPUI Context):
```rust
use db::gpui_tokio::Tokio;

// In GPUI components, use Tokio::spawn_result()
cx.spawn(|this, mut cx| async move {
    let result = Tokio::spawn_result(&cx, async {
        // SQLx async operations here
        connection.query("SELECT * FROM users").await
    }).await?;

    // Update UI with result
    this.update(&mut cx, |this, cx| {
        this.data = result;
        cx.notify();
    })
})
```

**Key Benefits**:
- Returns GPUI `Task<T>` instead of Tokio `JoinHandle<T>`
- Integrated with GPUI Context system
- Unified error handling with `anyhow::Result`
- No need to manually access runtime handle

**Initialization** (in `main.rs`):
```rust
db::gpui_tokio::init(&mut cx);  // Must be called before any database operations
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

**Two-Level Tab System**:

The application uses a unique two-level tab architecture for flexible workspace management:

**Level 1 - Top-Level Tabs** (managed by `OneHupApp`):
- `HomeTabContent` (`src/home.rs`): Connection cards in 3-column grid layout, non-closeable
- `DatabaseTabContent` (`src/database_tab.rs`): Database workspace with DockArea, one per connection
- `SettingsTabContent` (`src/setting_tab.rs`): Application settings, closeable

**Level 2 - Database Inner Tabs** (managed by `DatabaseTabContent`):
- `SqlEditorTabContent`: SQL editor with syntax highlighting
- `TableDataTabContent`: Table data grid view
- `TableStructureTabContent`: Table columns, indexes, constraints
- `ViewDataTabContent`: View data display
- `QueryResultTabContent`: Query execution results

**Why Two Levels?**
- Users can work with multiple database connections simultaneously
- Each database connection has its own isolated workspace
- Home tab provides quick access to all connections
- Tab bar positioned 80px from left edge to avoid macOS traffic lights

**Main Application Flow**:
1. `main.rs` - Initializes GPUI, registers Assets, initializes `db::gpui_tokio`, wraps app in `Root` for sheets/dialogs
2. `onehup_app.rs` - Root application state with connection filtering, top-level tab management
3. `home.rs` - Home tab showing connection cards
4. `database_tab.rs` - Database workspace with DockArea system
5. `setting_tab.rs` - Settings interface (placeholder)

**Key UI Components**:
- `DbTreeView` (`src/db_tree_view.rs`) - Lazy-loading hierarchical tree with PanelView integration
  - Maintains `loaded_children` and `loading_nodes` sets for optimization
  - Implements `PanelView` trait for DockArea compatibility
  - Emits events: `OpenTableData`, `OpenTableStructure`, `OpenViewData`, `ConnectToConnection`, `CreateNewQuery`
  - Node IDs format: `<connection_id>:<database>:<folder_type>:<object_name>`

- `DatabaseTabContent` (`src/database_tab.rs`) - DockArea-based database workspace
  - **Left panel**: DbTreeView (280px width, collapsible)
  - **Center panel**: TabPanel for SQL editors and data views
  - `DatabaseEventHandler`: Subscribes to tree events, creates corresponding tabs
  - Async connection with loading state and error display
  - Auto-connects on tab creation, disconnects on tab close

- `DbWorkspace` (`src/db_workspace.rs`) - Experimental/advanced workspace implementation
  - Layout versioning system (current: v5)
  - Layout persistence to JSON file
  - More flexible DockArea configuration
  - May replace `DatabaseTabContent` in future

- `TabContainer` (`src/tab_container.rs`) - Customizable tab management
  - Strategy pattern with `TabContent` trait
  - Color customization API: `with_tab_bar_colors()`, `with_tab_item_colors()`, `with_tab_content_colors()`
  - Tab type checking: `has_tab_type()`
  - Drag-and-drop tab reordering
  - Right-click context menu

- `SqlEditorTabContent` (`src/sql_editor_view.rs`) - SQL editing with tree-sitter syntax highlighting
  - Database selector dropdown, execute button, multi-result tabs
  - Displays execution time and row counts
  - Based on gpui-component's advanced Input component with LSP support

- `DbConnectionForm` (`src/db_connection_form.rs`) - Connection configuration UI
  - Supports MySQL and PostgreSQL with test connection functionality
  - Integrates with ConnectionStore for persistence

### Embedded UI Framework (crates/ui/)

**Major Architectural Decision**: The project embeds the complete gpui-component source code (~60,000 lines) instead of using it as an external dependency.

**Why Embed?**
1. **Complete control**: Can modify and extend components freely
2. **Faster iteration**: No waiting for upstream library updates
3. **Better IDE support**: Jump to component source directly
4. **Custom requirements**: Database tools need specific UI customizations

**Key Subsystems**:

**DockArea System** (`crates/ui/src/dock/`):
- Resizable, collapsible panels with 4 dock edges (left, right, top, bottom)
- `TabPanel` and `StackPanel` for different content arrangements
- Layout state serialization for persistence
- Used by `DatabaseTabContent` and `DbWorkspace`

**Advanced Input** (`crates/ui/src/input/`):
- 20+ files implementing a full-featured code editor
- Ropey-based text buffer for efficient editing
- LSP integration: completions, hover, diagnostics, code actions
- Tree-sitter syntax highlighting
- Multi-cursor support, search/replace
- Used as base for SQL editor

**Highlighter** (`crates/ui/src/highlighter/`):
- Tree-sitter integration for 20+ languages
- SQL, Rust, JavaScript, Python, Go, Java, etc.
- Theme-based syntax coloring
- Diagnostic display

**Other Components**:
- Table, List, Tree (virtual rendering for performance)
- Theme system (JSON-based, light/dark modes)
- Form inputs, dialogs, menus, popovers
- Charts (line, bar, area, pie)
- WebView support

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

**Status**: Fully implemented but **not yet enabled in UI**. Code exists in `src/data_export.rs` and `src/data_import.rs`.

**Key Functions**:
- `export_to_path()` - Write to file with directory creation
- `export_to_bytes()` - Return as UTF-8 bytes
- `import_from_csv()`, `import_from_json()`, `import_from_sql()`

## Key Design Patterns

### 1. Two-Level Tab Architecture
**Top-level tabs**: Application navigation (Home, Database connections, Settings)
**Inner tabs**: Database workspace content (SQL editors, table data, query results)

This allows users to work with multiple database connections simultaneously, each with its own isolated workspace.

### 2. Event-Driven Component Communication
**Pattern**: `DatabaseEventHandler` subscribes to `DbTreeViewEvent`, automatically creates corresponding tabs

**Event Flow**:
```
DbTreeView (user double-clicks table)
  ↓ emits DbTreeViewEvent::OpenTableData
DatabaseEventHandler (subscription handler)
  ↓ creates TableDataTabContent
DockArea center panel
  ↓ adds new tab
User sees table data
```

**Benefits**:
- Loose coupling between tree view and tab container
- Easy to add new event types
- Clean separation of concerns

### 3. PanelView Integration
`DbTreeView` implements `PanelView` trait to integrate with DockArea:
```rust
impl PanelView for DbTreeView {
    fn title(&self, cx: &WindowContext) -> AnyElement;
    fn ui_size(&self, cx: &WindowContext) -> Size<Length>;
    fn dump(&self, cx: &AppContext) -> PanelState;
}
```

This allows the tree view to be used as a collapsible dock panel with serializable state.

### 4. Stateless Plugins with Connection References
Plugins don't maintain state. They accept `&dyn DbConnection` for each operation, enabling:
- Flexible connection pooling and switching
- Thread-safe connection sharing via Arc<RwLock<>>
- Easy testing and plugin isolation

### 5. SQL Generation Before Execution
Database plugins generate SQL strings first, allowing user review before execution:
1. Call `create_table()` → returns SQL string
2. Display SQL to user
3. User confirms → execute via `execute_script()`

This is critical for DDL operations where mistakes can be destructive.

### 6. Hierarchical Node IDs for Tree Navigation
Tree nodes use structured IDs: `<connection_id>:<database>:<folder_type>:<object_name>`
- Example: `conn_mysql:mydb:table_folder:users`
- Enables efficient lazy loading and context tracking
- Folder types: `table_folder`, `view_folder`, `function_folder`, `procedure_folder`, `trigger_folder`, `sequence_folder` (PostgreSQL only)

### 7. Lazy Loading Tree with State Tracking
`DbTreeView` optimizes performance by:
- Only loading visible nodes initially
- Tracking `loaded_children` to avoid redundant queries
- Using `loading_nodes` to prevent concurrent loading of same node
- Calling `plugin.build_database_tree()` for databases, `plugin.load_node_children()` for sub-objects

### 8. Tab Strategy Pattern with Color Customization
Different tab content types implement `TabContent` trait:
- `HomeTabContent` - Connection cards grid (non-closeable)
- `DatabaseTabContent` - Database workspace (closeable)
- `SettingsTabContent` - Settings interface (closeable)
- `SqlEditor` - SQL editing interface
- `TableData` - Data grid display
- `TableForm` - Table structure (columns, indexes, constraints)
- `QueryResult` - Query execution results
- `Custom(String)` - Extensible for future types

**Color Customization**:
`TabContainer` supports full color customization via builder pattern:
```rust
TabContainer::new(cx)
    .with_tab_bar_colors(bg_color, border_color)
    .with_tab_item_colors(active_bg, hover_bg)
    .with_tab_content_colors(text_color, close_color)
```

`TabContainer` manages all tabs uniformly without knowing specific content details.

### 9. DockArea Flexible Layout System
DatabaseTabContent uses DockArea for panel management:
- Left dock: DbTreeView (280px, collapsible)
- Center: TabPanel for inner tabs
- Configurable collapsible edges
- Layout state serialization

DbWorkspace adds:
- Layout versioning (current: v5)
- Persistence to JSON file
- Version mismatch detection prompts user to reset layout

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
**Solution**: Always use `Tokio::spawn_result()` from `crates/db/src/gpui_tokio.rs` when calling SQLx operations from GPUI contexts. Initialize with `db::gpui_tokio::init(cx)` at app startup.

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

### 6. Two-Level Tab Confusion
**Problem**: Adding tabs to wrong container
**Solution**:
- Use `OneHupApp.tab_container` for top-level tabs (Home, Database, Settings)
- Use `DatabaseTabContent.dock_area` center panel for database inner tabs (SQL editors, table data)

### 7. Event Handler Subscription
**Problem**: Tree events not reaching tab container
**Solution**: Use `DatabaseEventHandler` pattern - subscribe to tree events in handler, create tabs in response

## Code Location Reference

When working on specific features, know where to look:

**Application Structure**:
- **Main entry**: `src/main.rs`
- **Root app state**: `src/onehup_app.rs`
- **Home tab**: `src/home.rs`
- **Database workspace**: `src/database_tab.rs`
- **Advanced workspace**: `src/db_workspace.rs` (experimental)
- **Settings tab**: `src/setting_tab.rs`

**Database Layer**:
- **Database plugin logic**: `crates/db/src/<dbname>/plugin.rs`
- **Connection management**: `crates/db/src/<dbname>/connection.rs`
- **Async runtime bridge**: `crates/db/src/gpui_tokio.rs` ⚠️ **NEW**
- **SQL parsing**: `crates/db/src/executor.rs`
- **Type definitions**: `crates/db/src/types.rs`
- **Manager & pool**: `crates/db/src/manager.rs`

**UI Components**:
- **UI tree navigation**: `src/db_tree_view.rs`
- **SQL editor**: `src/sql_editor_view.rs`, `src/sql_editor.rs`
- **Tab management**: `src/tab_container.rs`, `src/tab_contents.rs`
- **Connection forms**: `src/db_connection_form.rs`

**UI Framework** (embedded):
- **DockArea**: `crates/ui/src/dock/`
- **Advanced Input**: `crates/ui/src/input/`
- **Highlighter**: `crates/ui/src/highlighter/`
- **Table/List/Tree**: `crates/ui/src/{table,list,tree}/`
- **Theme system**: `crates/ui/src/theme/`

**Storage & Data**:
- **Persistent storage**: `src/storage/sqlite_backend.rs`
- **Connection store**: `src/connection_store.rs`
- **Import/Export**: `src/data_export.rs`, `src/data_import.rs` (not yet enabled)

**Assets**:
- **Icons**: `crates/assets/assets/icons/`
