# One-Hub - Qwen Code Context

## Project Overview

One-Hub is a modern multi-protocol connection tool built with Rust and GPUI. It supports database connections (MySQL, PostgreSQL, SQLite, etc.), SSH connections, Redis, and MongoDB, aiming to provide developers with a unified, fast, and stable connection and management experience.

### Architecture
The project is organized as a multi-crate workspace with the following main components:

- `one-hub`: Main application crate that serves as the entry point
- `crates/core`: Core functionality including tab container, themes, and storage management
- `crates/db`: Database connectivity layer supporting multiple database types
- `crates/db_view`: Database-specific UI components and views
- `crates/ui`: UI components using GPUI framework
- `crates/assets`: Asset management
- `crates/macros`: Custom macros

### Key Technologies
- **GPUI**: Graphics and UI framework for building the application
- **Tokio**: Asynchronous runtime for concurrent operations
- **SQLx**: Database client with support for multiple database backends
- **Tracing**: Logging and diagnostics
- **Serde**: Serialization/deserialization

### Features
- Database connection management for multiple database types
- Connection workspaces and organization
- SQL editor and result viewer
- Tabbed interface for managing multiple connections
- Dark/light theme support
- Connection testing functionality

## Building and Running

### Prerequisites
- Rust (latest stable version)
- Cargo

### Build Commands
```bash
# Build the project
cargo build

# Build in release mode
cargo build --release

# Run the project
cargo run

# Run tests
cargo test
```

### Development
To run the application in development mode:
```bash
cargo run
```

## Development Conventions

### Coding Style
- Follow Rust standard conventions and idioms
- Use `rustfmt` for code formatting (configured via rustfmt.toml)
- Use `clippy` for linting
- Async functions should use `tokio::spawn` for background tasks
- Error handling should use `anyhow` for application-level errors

### Project Structure
- UI components are built using GPUI framework
- Database operations are encapsulated in the `db` crate
- State management follows GPUI patterns with proper event emitters
- Tab-based interface managed by `TabContainer` in the `core` crate
- Data persistence handled through storage repositories

### Configuration
- Workspace dependencies are managed in the root Cargo.toml
- Different database backends are feature-gated
- Theme and UI customization via GPUI components

### Key Components
1. **TabContainer**: Core component for managing tabbed interface
2. **HomePage**: Main application screen with connection management
3. **DatabaseTabContent**: Database-specific tab content with tree view and editors
4. **Connection Management**: Supports MySQL, PostgreSQL, SQLite, SQL Server, Oracle
5. **Storage Layer**: Persists connections, workspaces, and other application data

### UI Framework
- Uses GPUI for cross-platform UI rendering
- Components built with reactive patterns
- Theme support with light/dark mode switching
- Responsive design for various screen sizes

### Data Flow
1. User interactions trigger events in GPUI components
2. Asynchronous operations are handled via Tokio tasks
3. Database operations are dispatched through the DB manager
4. State changes are propagated through GPUI context
5. UI updates occur reactively based on state changes