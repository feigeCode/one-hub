# One-Hub 代码地图

## 项目概览

### 基本信息

- **名称**: `one-hub`
- **目标**: One-Hub 是一款基于 Rust + GPUI 构建的现代化多协议数据库连接工具。它支持 MySQL、PostgreSQL 等关系型数据库的连接与管理，旨在为开发者提供统一、快速、稳定的数据库管理体验。
- **版本**: v0.1.0
- **Rust 版本**: 2024 Edition

### 技术栈

- **核心框架**: Rust + GPUI 0.2.2 (GPU 加速 UI 框架) + gpui-component 0.4.0
- **数据库驱动**: SQLx 0.8 (支持 MySQL, PostgreSQL, SQLite 异步驱动)
- **异步运行时**: Tokio 1.0 (多线程)
- **存储**: SQLite (连接配置持久化)
- **序列化**: serde, serde_json
- **数据导入导出**: CSV, JSON, SQL, Markdown, Excel (HTML/XML), Word (RTF)
---

### 核心框架源码目录
gpui：/Users/hufei/RustroverProjects/zed/crates/gpui
gpui-component（crates/ui下为框架核心源码，crates/story为各个组件的使用示例）: /Users/hufei/RustroverProjects/gpui-component

## 工作区结构

```
one-hub/
├── src/                          # 主应用程序
│   ├── main.rs                   # 程序入口
│   ├── onehup_app.rs             # 应用状态管理与 UI 布局
│   ├── app_view.rs               # 主工作区视图
│   ├── db_tree_view.rs           # 数据库树形导航
│   ├── sql_editor_view.rs        # SQL 编辑器标签页
│   ├── sql_editor.rs             # 文本编辑器组件
│   ├── tab_container.rs          # 标签页容器系统
│   ├── tab_contents.rs           # 标签页内容实现
│   ├── db_connection_form.rs     # 数据库连接表单
│   ├── connection_store.rs       # 连接配置持久化
│   ├── context_menu_tree.rs      # 树形菜单右键支持
│   ├── themes.rs                 # 主题管理
│   ├── data_export.rs            # 数据导出(多格式)
│   ├── data_import.rs            # 数据导入(多格式)
│   └── storage/                  # 存储层
│       ├── mod.rs
│       ├── traits.rs             # Storage/Queryable trait
│       ├── models.rs             # 数据模型
│       └── sqlite_backend.rs     # SQLite 实现
│
├── crates/
│   ├── db/                       # 数据库抽象层(核心)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── plugin.rs         # DatabasePlugin trait
│   │   │   ├── connection.rs     # DbConnection trait
│   │   │   ├── executor.rs       # SQL 执行与解析
│   │   │   ├── types.rs          # 数据模型与请求/响应类型
│   │   │   ├── manager.rs        # DbManager 与连接池
│   │   │   ├── runtime.rs        # Tokio 运行时桥接
│   │   │   ├── mysql/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── plugin.rs     # MySQL 插件实现
│   │   │   │   └── connection.rs # MySQL 连接实现
│   │   │   └── postgresql/
│   │   │       ├── mod.rs
│   │   │       ├── plugin.rs     # PostgreSQL 插件实现
│   │   │       └── connection.rs # PostgreSQL 连接实现
│   │   └── Cargo.toml
│   │
│   ├── assets/                   # 嵌入式资源
│   │   ├── src/lib.rs            # rust-embed 资源加载
│   │   ├── assets/               # SVG 图标等静态资源
│   │   └── Cargo.toml
│   │
│   ├── core/                     # 核心逻辑(预留)
│   │   └── src/main.rs
│   │
│   ├── mysql/                    # MySQL 专用模块(占位)
│   ├── postgresql/               # PostgreSQL 专用模块(占位)
│   └── sqlite/                   # SQLite 专用模块(占位)
│
├── Cargo.toml                    # 工作区配置
├── CLAUDE.md                     # 开发指南
└── CODEMAP.md                    # 本文档
```

---

## 核心模块详解

### 1. 入口模块 (`src/main.rs`)

**职责**: 程序入口，应用初始化和窗口创建

**核心功能**:

1. 注册本地资源加载器 `Assets` (从 `crates/assets/assets` 目录加载 SVG 图标等)
2. 初始化 GPUI 应用程序实例
3. 配置全局主题和窗口属性
4. 初始化 `GlobalDbState` 作为 GPUI 全局状态
5. 创建主窗口 (1600x1200，响应式尺寸)
6. 调用 `onehup_app::init()` 初始化框架
7. 配置窗口关闭行为

**关键代码流程**:
```rust
App::new()
    .with_assets(Assets)      // 加载嵌入式资源
    .run(|cx| {
        cx.set_global(GlobalDbState::default());
        cx.spawn(|mut cx| async move {
            cx.open_window(window_options, |cx| {
                onehup_app::init(cx);
                cx.new_view(|cx| OneHupApp::new(cx))
            })
        })
    })
```

---

### 2. 应用状态管理 (`src/onehup_app.rs`)

**职责**: 核心 UI 布局、状态管理和用户交互

#### 核心结构

**OneHupApp**:
```rust
pub struct OneHupApp {
    selected_filter: ConnectionType,      // 当前选中的连接类型过滤器
    connections: Vec<StoredConnection>,   // 所有连接配置
    tab_container: View<TabContainer>,    // 标签页容器
}
```

**ConnectionType 枚举**:
```rust
pub enum ConnectionType {
    All,            // 所有连接
    Database,       // 关系型数据库
    SshSftp,        // SSH/SFTP (预留)
    Redis,          // Redis (预留)
    MongoDB,        // MongoDB (预留)
}
```

**TabContent 类型**:
- `HomeTabContent`: 首页，显示连接卡片网格
- `ConnectionsTabContent`: 连接列表视图
- `SettingsTabContent`: 设置界面

#### UI 布局

1. **顶部工具栏**:
   - NEW HOST 按钮: 创建新连接
   - TERMINAL, SERIAL 按钮 (预留)
   - 右侧: 视图切换、设置按钮、用户头像

2. **左侧边栏**:
   - 连接类型过滤器 (All, Database, SSH/SFTP, Redis, MongoDB)
   - 筛选按钮显示当前选中数量

3. **中心内容区**:
   - 标签页容器，动态渲染首页/连接列表/设置页

4. **底部状态栏**:
   - 用户信息区域

#### 核心方法

- `new(cx)`: 初始化应用，创建标签页容器
- `render_toolbar()`: 渲染顶部工具栏
- `render_left_sidebar()`: 渲染左侧过滤器
- `render_home_content()`: 渲染首页连接卡片
- `render_connections_content()`: 渲染连接列表
- `toggle_theme()`: 切换亮色/暗色主题

---

### 3. 主工作区视图 (`src/app_view.rs`)

**职责**: 协调数据库交互、连接管理和标签页创建

#### 核心结构

**AppView**:
```rust
pub struct AppView {
    tree_view: View<DbTreeView>,                       // 数据库树视图
    connection_form: View<DbConnectionForm>,           // 连接表单
    active_connections: HashMap<String, Arc<RwLock<Box<dyn DbConnection>>>>,
    tab_container: View<TabContainer>,
    current_connection: Option<String>,
    current_database: Option<String>,
}
```

#### 核心功能

1. **连接管理**:
   - 创建并缓存数据库连接 (`active_connections`)
   - 支持多个同时打开的连接
   - 连接切换和状态追踪

2. **事件订阅**:
   - 订阅 `DbTreeView` 事件 (打开表数据、视图、创建查询等)
   - 订阅连接表单事件 (保存/测试连接)

3. **标签页创建**:
   - `open_table_data_tab()`: 打开表数据标签
   - `open_table_structure_tab()`: 打开表结构标签
   - `open_view_data_tab()`: 打开视图数据标签
   - `create_new_query_tab()`: 创建新查询标签

4. **UI 布局**: 三栏布局 (左侧树 + 中心标签页 + 顶部表单)

---

### 4. 数据库树形导航 (`src/db_tree_view.rs`)

**职责**: 分层展示数据库对象，支持懒加载

#### 核心结构

**DbTreeView**:
```rust
pub struct DbTreeView {
    connection_id: Option<String>,
    tree_state: Entity<TreeState<DbNode>>,
    nodes: HashMap<String, DbNode>,         // 节点缓存
    loaded_children: HashSet<String>,       // 已加载子节点的节点集合
    loading_nodes: HashSet<String>,         // 正在加载的节点集合
}
```

#### 懒加载机制

**节点层级**:
```
Connection
  └─ Database
      ├─ TablesFolder
      │   └─ Table
      ├─ ViewsFolder
      │   └─ View
      ├─ FunctionsFolder
      │   └─ Function
      ├─ ProceduresFolder
      │   └─ Procedure
      ├─ TriggersFolder
      │   └─ Trigger
      └─ SequencesFolder (PostgreSQL)
          └─ Sequence
```

**加载流程**:
1. 初始只加载连接节点
2. 展开连接时，调用 `plugin.build_database_tree()` 加载数据库列表
3. 展开数据库时，创建文件夹节点 (TablesFolder, ViewsFolder 等)
4. 展开文件夹时，调用 `plugin.load_node_children()` 加载具体对象

#### 事件发射

**DbTreeViewEvent 枚举**:
- `OpenTableData { database, table }`: 打开表数据
- `OpenViewData { database, view }`: 打开视图数据
- `OpenTableStructure { database, table }`: 打开表结构
- `ConnectToConnection { id, name }`: 连接到数据库
- `CreateNewQuery { database }`: 创建新查询

---

### 5. SQL 编辑器 (`src/sql_editor_view.rs`, `src/sql_editor.rs`)

#### sql_editor_view.rs - SQL 编辑器标签页

**SqlEditorTabContent**:
```rust
pub struct SqlEditorTabContent {
    connection_id: String,
    database: Option<String>,
    editor: View<SqlEditor>,                   // 文本编辑器
    results: Vec<SqlResult>,                   // 多结果集
    active_result_index: usize,
    status_message: Option<String>,
    execution_time: Option<Duration>,
    affected_rows: Option<usize>,
}
```

**功能**:
1. SQL 编辑器区域 (支持语法高亮)
2. 数据库选择下拉框
3. 执行按钮
4. 结果标签页 (支持多结果集)
5. 状态消息和执行时间显示

#### sql_editor.rs - 文本编辑器组件

**SqlEditor**:
- 基于 tree-sitter 的语法高亮
- 多行编辑支持
- 集成 gpui-component 的编辑器功能

---

### 6. 标签页系统 (`src/tab_container.rs`, `src/tab_contents.rs`)

#### tab_container.rs - 标签页容器

**TabContent Trait** (策略模式):
```rust
pub trait TabContent: 'static {
    fn render(&self, cx: &mut WindowContext) -> impl IntoElement;
    fn title(&self) -> SharedString;
    fn closeable(&self) -> bool { true }
    fn tab_type(&self) -> TabContentType;
}
```

**TabContentType 枚举**:
```rust
pub enum TabContentType {
    SqlEditor,
    TableData,
    TableForm,
    QueryResult,
    Custom(String),
}
```

**TabContainer**:
```rust
pub struct TabContainer {
    tabs: Vec<TabItem>,
    active_tab: Option<usize>,
}

pub struct TabItem {
    id: String,
    title: SharedString,
    content: Box<dyn TabContent>,
}
```

#### tab_contents.rs - 标签页内容实现

**TableDataTabContent**:
- 显示表数据的标签页
- 使用 Table 组件渲染数据网格
- 支持分页、排序、筛选

**TableStructureTabContent**:
- 显示表结构 (列定义、索引、约束)
- 多标签展示: Columns, Indexes, Constraints

**DelegateWrapper**:
- 包装 `TableDelegate` 用于 GPUI 渲染

---

### 7. 连接管理 (`src/db_connection_form.rs`, `src/connection_store.rs`)

#### db_connection_form.rs - 连接表单

**FormField**:
```rust
pub struct FormField {
    pub name: String,
    pub label: String,
    pub field_type: FieldType,        // Text, Password, Number, Select
    pub required: bool,
    pub default_value: Option<String>,
}
```

**DbFormConfig**:
- MySQL 表单配置: name, host, port, username, password, database
- PostgreSQL 表单配置: 同上

**DbConnectionForm**:
```rust
pub struct DbConnectionForm {
    connection_type: DatabaseType,
    fields: HashMap<String, Entity<InputState>>,
    status_message: Option<String>,
}
```

**方法**:
- `test_connection()`: 异步测试连接
- `save_connection()`: 保存到 ConnectionStore

#### connection_store.rs - 连接持久化

**ConnectionStore**:
```rust
pub struct ConnectionStore {
    storage: Arc<SqliteStorage<StoredConnection>>,
}
```

**方法**:
- `new()`: 初始化 SQLite 存储 (`~/.config/one-hub/one-hub.db`)
- `load_connections()`: 加载所有连接
- `save_connection(config)`: 保存连接配置
- `delete_connection(id)`: 删除连接
- `get_connection(id)`: 获取单个连接

**桥接 Tokio**:
- 使用 `crates/db/src/runtime.rs` 的 `spawn_result()` 在 GPUI 上下文中执行异步操作

---

### 8. 存储层 (`src/storage/`)

#### traits.rs - 抽象接口

**Storage Trait**:
```rust
#[async_trait]
pub trait Storage<T>: Send + Sync {
    async fn insert(&self, item: &T) -> Result<(), StorageError>;
    async fn update(&self, item: &T) -> Result<(), StorageError>;
    async fn delete(&self, id: &str) -> Result<(), StorageError>;
    async fn get(&self, id: &str) -> Result<Option<T>, StorageError>;
    async fn list(&self) -> Result<Vec<T>, StorageError>;
    async fn clear(&self) -> Result<(), StorageError>;
}
```

**Queryable Trait**:
```rust
#[async_trait]
pub trait Queryable<T>: Storage<T> {
    async fn find_by(&self, field: &str, value: &str) -> Result<Vec<T>, StorageError>;
    async fn find_one_by(&self, field: &str, value: &str) -> Result<Option<T>, StorageError>;
    async fn count(&self) -> Result<usize, StorageError>;
    async fn exists(&self, id: &str) -> Result<bool, StorageError>;
}
```

#### models.rs - 数据模型

**StoredConnection**:
```rust
pub struct StoredConnection {
    pub id: String,
    pub name: String,
    pub db_type: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub database: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
```

**转换**:
- `From<DbConnectionConfig> for StoredConnection`
- `From<StoredConnection> for DbConnectionConfig`

#### sqlite_backend.rs - SQLite 实现

**SqliteStorage**:
```rust
pub struct SqliteStorage<T> {
    pool: SqlitePool,
    _marker: PhantomData<T>,
}
```

**功能**:
- 自动创建数据库文件和 connections 表
- 实现 `Storage<StoredConnection>` trait
- 实现 `Queryable<StoredConnection>` trait
- 时间戳自动管理 (created_at, updated_at)

---

### 9. 数据导入导出 (`src/data_export.rs`, `src/data_import.rs`)

#### data_export.rs - 多格式导出

**支持格式**:

| 格式              | 描述                          | 方法                      |
|-----------------|-----------------------------|-----------------------------|
| CSV             | RFC 4180 标准 CSV，字段转义       | `export_to_csv()`           |
| JSON            | 对象数组格式                      | `export_to_json()`          |
| SQL             | INSERT 语句 (可配置表名)           | `export_to_sql()`           |
| Markdown        | Markdown 表格格式                | `export_to_markdown()`      |
| Excel (HTML)    | HTML 表格 (Excel 兼容 .xls)      | `export_to_excel_html()`    |
| Excel (XML)     | SpreadsheetML 格式 (.xml)      | `export_to_excel_xml()`     |
| Word (RTF)      | RTF 表格格式                     | `export_to_word_rtf()`      |

**配置选项**:
```rust
pub struct CsvOptions {
    pub delimiter: char,           // 默认 ','
    pub include_headers: bool,     // 默认 true
}

pub struct SqlOptions {
    pub table_name: String,
    pub null_when_empty: bool,     // 空字符串转为 NULL
}
```

**核心方法**:
- `export_to_path(path, format, data, options)`: 导出到文件 (自动创建目录)
- `export_to_bytes(format, data, options)`: 导出为字节数组
- 各格式专用函数: 正确的字段转义和 NULL 处理

**特性**:
- CSV: 字段自动引号和转义
- SQL: 参数化 INSERT 语句
- Excel HTML: `<meta charset="utf-8">` 确保中文支持
- Excel XML: 完整的 SpreadsheetML schema
- RTF: 正确的 RTF 编码和表格格式

#### data_import.rs - 多格式导入

**支持格式**:
- CSV (RFC 4180 解析，支持引号字段)
- JSON (对象数组/数组数组/NDJSON)
- SQL (原始脚本，无解析)

**配置选项**:
```rust
pub struct CsvImportOptions {
    pub delimiter: char,           // 默认 ','
    pub has_headers: bool,         // 默认 true
    pub trim_fields: bool,         // 默认 true
}

pub struct JsonImportOptions {
    pub key_extraction: KeyExtraction,
}

pub enum KeyExtraction {
    FirstObject,                   // 使用第一个对象的键
    UnionAll,                      // 合并所有对象的键
}
```

**核心方法**:
- `import_from_csv(reader, options)`: CSV 导入
- `import_from_json(reader, options)`: JSON 导入
- `import_from_sql(reader)`: SQL 脚本导入

**特性**:
- CSV: 多行引号字段支持
- JSON: NDJSON 支持 (每行一个 JSON 对象)
- 自动列名生成: `Column1`, `Column2`, ... (无表头时)
- 类型安全的 JSON 值转换

---

### 10. 数据库抽象层 (`crates/db/`)

#### plugin.rs - DatabasePlugin Trait

**核心设计**: 无状态插件，接受连接引用

```rust
#[async_trait]
pub trait DatabasePlugin: Send + Sync {
    // 数据库层级操作
    async fn list_databases(&self, conn: &dyn DbConnection) -> Result<Vec<String>>;
    async fn create_database(&self, conn: &dyn DbConnection, req: &CreateDatabaseReq) -> Result<String>;
    async fn drop_database(&self, conn: &dyn DbConnection, req: &DropDatabaseReq) -> Result<String>;

    // 表操作
    async fn list_tables(&self, conn: &dyn DbConnection, database: &str) -> Result<Vec<String>>;
    async fn get_table_columns(&self, conn: &dyn DbConnection, database: &str, table: &str) -> Result<Vec<ColumnInfo>>;
    async fn get_table_indexes(&self, conn: &dyn DbConnection, database: &str, table: &str) -> Result<Vec<IndexInfo>>;
    async fn create_table(&self, conn: &dyn DbConnection, req: &CreateTableReq) -> Result<String>;
    async fn drop_table(&self, conn: &dyn DbConnection, req: &DropTableReq) -> Result<String>;
    async fn rename_table(&self, conn: &dyn DbConnection, req: &RenameTableReq) -> Result<String>;
    async fn truncate_table(&self, conn: &dyn DbConnection, req: &TruncateTableReq) -> Result<String>;

    // 列操作
    async fn add_column(&self, conn: &dyn DbConnection, req: &AddColumnReq) -> Result<String>;
    async fn modify_column(&self, conn: &dyn DbConnection, req: &ModifyColumnReq) -> Result<String>;
    async fn drop_column(&self, conn: &dyn DbConnection, req: &DropColumnReq) -> Result<String>;

    // 视图操作
    async fn list_views(&self, conn: &dyn DbConnection, database: &str) -> Result<Vec<ViewInfo>>;
    async fn create_view(&self, conn: &dyn DbConnection, req: &CreateViewReq) -> Result<String>;
    async fn drop_view(&self, conn: &dyn DbConnection, req: &DropViewReq) -> Result<String>;

    // 函数操作
    async fn list_functions(&self, conn: &dyn DbConnection, database: &str) -> Result<Vec<FunctionInfo>>;
    async fn create_function(&self, conn: &dyn DbConnection, req: &CreateFunctionReq) -> Result<String>;
    async fn drop_function(&self, conn: &dyn DbConnection, req: &DropFunctionReq) -> Result<String>;

    // 存储过程操作
    async fn list_procedures(&self, conn: &dyn DbConnection, database: &str) -> Result<Vec<FunctionInfo>>;
    async fn create_procedure(&self, conn: &dyn DbConnection, req: &CreateProcedureReq) -> Result<String>;
    async fn drop_procedure(&self, conn: &dyn DbConnection, req: &DropProcedureReq) -> Result<String>;

    // 触发器操作
    async fn list_triggers(&self, conn: &dyn DbConnection, database: &str) -> Result<Vec<TriggerInfo>>;
    async fn create_trigger(&self, conn: &dyn DbConnection, req: &CreateTriggerReq) -> Result<String>;
    async fn drop_trigger(&self, conn: &dyn DbConnection, req: &DropTriggerReq) -> Result<String>;

    // 序列操作 (PostgreSQL)
    async fn list_sequences(&self, conn: &dyn DbConnection, database: &str) -> Result<Vec<SequenceInfo>>;
    async fn create_sequence(&self, conn: &dyn DbConnection, req: &CreateSequenceReq) -> Result<String>;
    async fn drop_sequence(&self, conn: &dyn DbConnection, req: &DropSequenceReq) -> Result<String>;
    async fn alter_sequence(&self, conn: &dyn DbConnection, req: &AlterSequenceReq) -> Result<String>;

    // 树形导航
    async fn build_database_tree(&self, conn: &dyn DbConnection, connection_id: &str) -> Result<Vec<DbNode>>;
    async fn load_node_children(&self, conn: &dyn DbConnection, node: &DbNode) -> Result<Vec<DbNode>>;

    // 查询执行
    async fn execute_query(&self, conn: &dyn DbConnection, req: &ExecuteQueryReq) -> Result<QueryResult>;
    async fn execute_script(&self, conn: &dyn DbConnection, req: &ExecuteScriptReq) -> Result<Vec<SqlResult>>;
}
```

**设计要点**:
1. **无状态**: 插件不保存连接，每次操作传入连接引用
2. **SQL 生成**: 生成数据库特定的 SQL 语句，由调用方执行
3. **标识符转义**: MySQL 使用反引号 `` ` ``，PostgreSQL 使用双引号 `"`
4. **两阶段执行**: 生成 SQL → 显示给用户 → 用户确认 → 执行

#### connection.rs - DbConnection Trait

```rust
#[async_trait]
pub trait DbConnection: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn execute(&mut self, sql: &str) -> Result<Vec<SqlResult>>;
    async fn query(&mut self, sql: &str, params: Option<Vec<Value>>) -> Result<QueryResult>;
    async fn ping(&mut self) -> Result<()> {
        self.query("SELECT 1", None)?;
        Ok(())
    }
}
```

#### executor.rs - SQL 执行模型

**SqlResult 枚举**:
```rust
pub enum SqlResult {
    Query(QueryResult),      // SELECT 结果
    Exec(ExecResult),        // INSERT/UPDATE/DELETE 结果
    Error(String),           // 错误信息
}
```

**QueryResult**:
```rust
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
    pub execution_time: Option<Duration>,
}
```

**ExecResult**:
```rust
pub struct ExecResult {
    pub rows_affected: u64,
    pub message: String,
}
```

**SqlScriptSplitter**:
- 按分号 `;` 分割 SQL 脚本
- 正确处理字符串字面量 (单引号 `'`, 双引号 `"`, 反引号 `` ` ``)
- 正确处理注释 (行注释 `--`, `#`; 块注释 `/* */`)
- 支持多行语句

**SqlStatementClassifier**:
```rust
pub enum StatementType {
    Query,         // SELECT
    DML,           // INSERT, UPDATE, DELETE
    DDL,           // CREATE, ALTER, DROP
    Transaction,   // BEGIN, COMMIT, ROLLBACK
    Command,       // USE, SET
    Exec,          // EXEC, CALL
    Unknown,
}
```

#### types.rs - 数据模型

**DatabaseType 枚举**:
```rust
pub enum DatabaseType {
    MySQL,
    PostgreSQL,
}
```

**DbNode** (树形节点):
```rust
pub struct DbNode {
    pub id: String,
    pub label: String,
    pub node_type: DbNodeType,
    pub parent_database: Option<String>,
    pub children: Vec<DbNode>,
}

pub enum DbNodeType {
    Connection,
    Database,
    TablesFolder,
    Table,
    ViewsFolder,
    View,
    FunctionsFolder,
    Function,
    ProceduresFolder,
    Procedure,
    TriggersFolder,
    Trigger,
    SequencesFolder,
    Sequence,
}
```

**DbConnectionConfig**:
```rust
pub struct DbConnectionConfig {
    pub id: String,
    pub name: String,
    pub db_type: DatabaseType,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub database: Option<String>,
}
```

**ColumnInfo**, **IndexInfo**, **ViewInfo**, **FunctionInfo**, **TriggerInfo**, **SequenceInfo**:
- 数据库对象元数据结构

**请求对象**:
- `CreateDatabaseReq`, `DropDatabaseReq`
- `CreateTableReq`, `DropTableReq`, `RenameTableReq`, `TruncateTableReq`
- `AddColumnReq`, `ModifyColumnReq`, `DropColumnReq`
- `CreateViewReq`, `DropViewReq`
- `CreateFunctionReq`, `DropFunctionReq`
- `CreateProcedureReq`, `DropProcedureReq`
- `CreateTriggerReq`, `DropTriggerReq`
- `CreateSequenceReq`, `DropSequenceReq`, `AlterSequenceReq`
- `ExecuteQueryReq`, `ExecuteScriptReq`

#### manager.rs - DbManager & ConnectionPool

**DbManager** (工厂模式):
```rust
pub struct DbManager;

impl DbManager {
    pub fn get_plugin(db_type: &DatabaseType) -> Box<dyn DatabasePlugin> {
        match db_type {
            DatabaseType::MySQL => Box::new(MySqlPlugin),
            DatabaseType::PostgreSQL => Box::new(PostgresPlugin),
        }
    }
}
```

**ConnectionPool**:
```rust
pub struct ConnectionPool {
    connections: Arc<RwLock<HashMap<String, Arc<RwLock<Box<dyn DbConnection>>>>>>,
    current_connection_id: Arc<RwLock<Option<String>>>,
    current_database: Arc<RwLock<Option<String>>>,
}
```

**方法**:
- `add_connection(id, conn)`: 添加连接到池
- `get_connection(id)`: 获取连接 (返回 Arc 克隆)
- `remove_connection(id)`: 移除连接
- `set_current(connection_id, database)`: 设置当前连接和数据库

**GlobalDbState**:
```rust
pub struct GlobalDbState {
    pub pool: ConnectionPool,
    pub connection_store: Arc<ConnectionStore>,
}
```

#### runtime.rs - Tokio 运行时桥接

**TOKIO_RUNTIME**:
```rust
pub static TOKIO_RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime")
});
```

**spawn_result 辅助函数**:
```rust
pub async fn spawn_result<F, T>(future: F) -> Result<T, PluginError>
where
    F: Future<Output = Result<T, PluginError>> + Send + 'static,
    T: Send + 'static,
{
    TOKIO_RUNTIME.spawn(future).await?
}
```

**用途**: 在 GPUI 的同步/异步上下文中执行 SQLx 异步操作 (SQLx 依赖 Tokio)

#### mysql/plugin.rs - MySQL 插件实现

**MySqlPlugin**:
- 实现 `DatabasePlugin` trait
- 使用反引号 `` ` `` 转义标识符
- 查询 `INFORMATION_SCHEMA` 获取元数据
- 生成 MySQL 特定的 DDL/DML SQL

**关键方法**:
- `list_databases()`: `SHOW DATABASES`
- `list_tables(database)`: `SHOW TABLES FROM database`
- `get_table_columns()`: 查询 `INFORMATION_SCHEMA.COLUMNS`
- `get_table_indexes()`: 查询 `INFORMATION_SCHEMA.STATISTICS`

#### mysql/connection.rs - MySQL 连接实现

**MysqlDbConnection**:
```rust
pub struct MysqlDbConnection {
    pool: Option<MySqlPool>,
    config: DbConnectionConfig,
}
```

**实现**:
- 基于 SQLx `MySqlPool`
- 参数绑定和类型提取
- 支持类型: String, Int, Float, Bool, Bytes, JSON, DateTime, BigDecimal 等
- 事务支持 (在 `execute()` 中)

#### postgresql/plugin.rs - PostgreSQL 插件实现

**PostgresPlugin**:
- 实现 `DatabasePlugin` trait
- 使用双引号 `"` 转义标识符
- 查询 `pg_database`, `pg_tables`, `information_schema` 获取元数据
- 生成 PostgreSQL 特定的 DDL/DML SQL
- **特有**: 序列 (Sequence) 支持

**关键方法**:
- `list_databases()`: 查询 `pg_database`
- `list_tables(database)`: 查询 `information_schema.tables`
- `list_sequences()`: 查询 `information_schema.sequences`

#### postgresql/connection.rs - PostgreSQL 连接实现

**PostgresDbConnection**:
```rust
pub struct PostgresDbConnection {
    pool: Option<PgPool>,
    config: DbConnectionConfig,
}
```

**实现**:
- 基于 SQLx `PgPool`
- 参数绑定和类型提取
- 支持 PostgreSQL 特定类型: UUID, JSONB, ARRAY 等
- 事务支持

---

### 11. 静态资源 (`crates/assets/`)

**内容**: SVG 图标和其他静态资源

**lib.rs**:
```rust
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "assets"]
pub struct Assets;
```

**资源目录**:
```
crates/assets/assets/
  └── icons/
      ├── mysql.svg
      ├── postgresql.svg
      ├── sqlite.svg
      ├── redis.svg
      ├── mongodb.svg
      └── ... (其他图标)
```

**使用**: 通过 `Assets::get("icons/mysql.svg")` 获取嵌入的资源

---

## 关键架构模式

### 1. 插件架构 (Plugin Architecture)

**设计**: 数据库插件无状态，接受连接引用

**优势**:
- 新增数据库类型无需修改核心代码
- 插件可独立测试
- 连接池管理灵活

**工厂模式**: `DbManager::get_plugin(db_type)` 返回对应插件

### 2. 异步/同步桥接 (Async/Sync Bridge)

**问题**: GPUI 使用 smol 执行器，SQLx 需要 Tokio 运行时

**解决方案**:
- 全局 `TOKIO_RUNTIME` 实例
- `spawn_result()` 辅助函数在 GPUI 上下文中执行 Tokio future
- `ConnectionStore` 使用桥接函数调用异步存储操作

**示例**:
```rust
cx.spawn(|this, mut cx| async move {
    let result = spawn_result(async {
        // SQLx 异步操作
        storage.load_connections().await
    }).await;
    // 更新 UI
    cx.update(|cx| { /* ... */ })
})
```

### 3. 懒加载树 (Lazy-Loading Tree)

**策略**:
1. 初始只加载顶层节点
2. 展开时动态加载子节点
3. 使用 `loaded_children` 集合避免重复加载
4. `loading_nodes` 集合防止并发加载

**节点 ID 格式**: `<connection_id>:<database>:<folder_type>:<object_name>`

### 4. 策略模式标签页 (Strategy Pattern Tabs)

**TabContent Trait**:
- 不同内容类型实现相同接口
- `TabContentType` 枚举类型标识
- `TabContainer` 统一管理

**内容类型**:
- SQL 编辑器
- 表数据查看
- 表结构查看
- 查询结果
- 自定义内容

### 5. 多连接管理 (Multi-Connection Management)

**ConnectionPool**:
- `HashMap<String, Arc<RwLock<Box<dyn DbConnection>>>>`
- 支持多个同时打开的连接
- Arc 包装实现高效克隆
- RwLock 保证线程安全

**当前连接追踪**:
- `current_connection_id`: 当前活动连接
- `current_database`: 当前选中数据库

### 6. SQL 解析与分类 (SQL Parsing & Classification)

**SqlScriptSplitter**:
- 正确处理字符串字面量和注释
- 支持多种引号风格 (`'`, `"`, `` ` ``)
- 多行语句支持

**SqlStatementClassifier**:
- 识别语句类型 (Query, DML, DDL, Transaction, Command, Exec)
- 用于选择合适的执行和结果展示策略

### 7. 持久化存储 (Persistent Storage)

**通用 Storage Trait**:
- 抽象的 CRUD 接口
- 支持不同存储后端 (当前为 SQLite)

**SQLite 后端**:
- 存储位置: `~/.config/one-hub/one-hub.db` (macOS/Linux) 或 `%APPDATA%/one-hub/one-hub.db` (Windows)
- 自动初始化数据库和表结构
- 时间戳自动管理

---

## 数据库支持状态

### 完全实现
1. **MySQL**
   - 完整的插件实现
   - 连接池 (SQLx MySqlPool)
   - INFORMATION_SCHEMA 元数据查询
   - 反引号标识符转义
   - 完整 DDL/DML/查询支持

2. **PostgreSQL**
   - 完整的插件实现
   - 连接池 (SQLx PgPool)
   - pg_catalog/information_schema 元数据查询
   - 双引号标识符转义
   - 序列 (Sequence) 支持
   - 完整 DDL/DML/查询支持

### 架构就绪但未实现
- SQLite (工作区结构已创建，crates/sqlite/)
- Redis (工作区结构已创建，crates/redis/)
- MongoDB (工作区结构已创建)
- Oracle (未开始)
- SQL Server (未开始)

---

## UI 架构

**框架**: GPUI 0.2.2 + gpui-component 0.4.0

**主要布局**:

```
┌─────────────────────────────────────────────────────┐
│  [NEW HOST] [TERMINAL] [SERIAL]     [@] [☰] [Settings] │  ← 顶部工具栏
├──────┬──────────────────────────────────────────────┤
│      │  Home / Connections / Settings Tab           │  ← 标签页
│      │                                               │
│ [≡]  │  ┌─────────────────────────────────────┐    │
│ All  │  │  Connection Cards / List / Settings │    │
│      │  │                                      │    │
│ [DB] │  └─────────────────────────────────────┘    │
│      │                                               │
│ SSH  │                                               │
│      │                                               │
│ Redis│                                               │
│      │                                               │
│ Mongo│                                               │
│      │                                               │
│ [@]  │                                               │  ← 用户信息
└──────┴──────────────────────────────────────────────┘
```

**工作区视图** (AppView):
```
┌────────────────────────────────────────────────┐
│  Connection Form                               │  ← 连接表单
├────────┬──────────────────────────────────────┤
│        │  Tab1 | Tab2 | Tab3 | ...           │  ← 标签栏
│ Tree   │                                       │
│        │  ┌──────────────────────────────┐   │
│ ├─ DB1 │  │                              │   │  ← 标签页内容
│ │ ├─ T │  │  SQL Editor / Table Data /   │   │
│ │ ├─ V │  │  Table Structure / ...       │   │
│ │ └─ F │  │                              │   │
│ ├─ DB2 │  └──────────────────────────────┘   │
│ └─ ... │                                       │
└────────┴──────────────────────────────────────┘
```

**组件库** (gpui-component):
- DockArea: 可调整大小的面板系统
- Table: 数据网格，支持列调整/排序/选择
- Tree: 树形组件，懒加载支持
- Select/Dropdown: 下拉选择框
- Input: 文本输入框
- Button: 按钮组件

**主题**:
- 亮色/暗色模式切换
- 集中式主题配置 (`themes.rs`)
- 主题持久化

---

## 独特设计决策

### 1. 无状态插件
数据库插件不维护连接状态，使得连接池管理和连接切换更加灵活。

### 2. SQL 先生成后执行
两阶段执行模式允许用户在执行前审查 SQL 语句，特别适合 DDL 操作。

### 3. 分层节点 ID
格式 `<connection_id>:<database>:<folder_type>:<object_name>` 实现高效的树导航和上下文追踪。

### 4. 全局 Tokio 运行时
单一全局运行时实例桥接 GPUI (smol) 和 SQLx (Tokio) 的异步生态系统。

### 5. Arc 包装连接
连接使用 `Arc<RwLock<Box<dyn DbConnection>>>` 包装，实现高效克隆和线程安全共享。

### 6. 通用 Storage Trait
抽象的存储接口允许轻松切换存储后端 (当前 SQLite，可扩展到 PostgreSQL/MySQL)。

### 7. 标签策略模式
`TabContent` trait 允许无缝添加新的标签类型，无需修改容器代码。

### 8. 多格式导入导出
统一的导入导出接口支持 CSV, JSON, SQL, Markdown, Excel, Word 等多种格式。

---

## 构建与开发

### 构建命令

```bash
cargo build                    # 调试构建
cargo build --release          # 发布构建
cargo run                      # 构建并运行
cargo test                     # 运行所有测试
cargo check                    # 快速语法检查
```

### 功能特性

- tree-sitter 语言支持 (SQL 语法高亮)
- Native TLS (数据库连接)
- SQLx 特性: mysql, postgres, sqlite 驱动
- Tokio 多线程运行时

### 核心依赖

| 分类        | 依赖                                        | 版本    |
|-----------|-------------------------------------------|-------|
| **UI 框架** | gpui                                      | 0.2.2 |
|           | gpui-component                            | 0.4.0 |
| **数据库驱动** | sqlx (mysql, postgres, sqlite)            | 0.8   |
| **异步运行时** | tokio                                     | 1.0   |
| **序列化**   | serde, serde_json                         | 1.0   |
| **时间处理**  | chrono                                    | 0.4   |
| **UUID**  | uuid                                      | 1.0   |
| **错误处理**  | thiserror, anyhow                         | 1.0   |
| **资源嵌入**  | rust-embed                                | 8.0   |
| **CSV**   | csv                                       | 1.0   |
| **配置路径**  | dirs                                      | 5.0   |

---

## 项目状态

### 当前阶段
**核心功能开发中**

### 已实现功能 ✅

#### 主窗口与导航
- ✅ 顶部工具栏 (新建连接、终端、串口按钮)
- ✅ 左侧过滤器 (按连接类型筛选)
- ✅ 标签页系统 (首页、连接列表、设置)
- ✅ 主题切换 (亮色/暗色)
- ✅ 用户信息显示

#### 连接管理
- ✅ 连接表单 (MySQL, PostgreSQL)
- ✅ 测试连接功能
- ✅ 保存连接到 SQLite
- ✅ 连接列表展示
- ✅ 连接卡片网格视图
- ✅ 多连接同时管理

#### 数据库树视图
- ✅ 分层树形导航 (连接 → 数据库 → 表/视图/函数/存储过程/触发器)
- ✅ 懒加载子节点
- ✅ 展开/折叠动画
- ✅ 右键菜单支持
- ✅ 双击打开表数据

#### SQL 编辑器
- ✅ 语法高亮 (tree-sitter)
- ✅ 多行编辑
- ✅ 数据库选择下拉框
- ✅ 执行查询按钮
- ✅ 多结果集标签页
- ✅ 执行时间和行数统计

#### 数据查看
- ✅ 表数据标签页 (网格展示)
- ✅ 表结构标签页 (列定义、索引、约束)
- ✅ 视图数据查看
- ✅ 查询结果展示

#### 数据导入导出
- ✅ CSV 导出 (RFC 4180 标准)
- ✅ JSON 导出 (对象数组)
- ✅ SQL 导出 (INSERT 语句)
- ✅ Markdown 导出 (表格格式)
- ✅ Excel 导出 (HTML/XML 格式)
- ✅ Word 导出 (RTF 表格)
- ✅ CSV 导入 (支持引号字段)
- ✅ JSON 导入 (对象数组/NDJSON)
- ✅ SQL 导入 (脚本)

#### 数据库驱动
- ✅ MySQL 完整实现 (连接、查询、元数据、DDL/DML)
- ✅ PostgreSQL 完整实现 (包含序列支持)
- ✅ 插件架构 (DatabasePlugin trait)
- ✅ 连接池管理
- ✅ 参数化查询 (防 SQL 注入)

#### 持久化
- ✅ SQLite 存储后端
- ✅ 连接配置持久化
- ✅ 平台特定配置路径 (~/.config/one-hub/)
- ✅ 自动时间戳管理

### 开发中 🚧

#### 数据库支持
- 🚧 SQLite 驱动实现
- 🚧 Redis 驱动实现
- 🚧 MongoDB 驱动实现

#### UI 增强
- 🚧 数据编辑功能 (单元格编辑、行增删)
- 🚧 查询历史记录
- 🚧 收藏查询
- 🚧 拖拽导入文件

### 待开发 📋

#### 高级功能
- 📋 数据库结构可视化
- 📋 性能监控和慢查询分析
- 📋 查询执行计划
- 📋 批量数据修改
- 📋 数据库对比和同步

#### 用户体验
- 📋 键盘快捷键系统
- 📋 更丰富的右键菜单
- 📋 拖拽重排标签页
- 📋 窗口分割视图
- 📋 搜索和替换

#### 扩展功能
- 📋 SSH 隧道连接
- 📋 Oracle 驱动实现
- 📋 SQL Server 驱动实现
- 📋 连接分组管理
- 📋 团队协作功能

---

## 代码统计

| 模块            | 文件数 | 代码行数 (估算) |
|---------------|-----|-----------|
| src/          | 16  | ~4500     |
| crates/db/    | 11  | ~3500     |
| crates/assets | 1   | ~10       |
| **总计**        | 28  | **~8000** |

---

## 贡献指南

### 代码规范

1. **导入顺序**:
   ```rust
   // 1. 标准库导入
   use std::collections::HashMap;

   // 2. 外部 crate 导入 (按字母顺序)
   use anyhow::Result;
   use gpui::{prelude::*, *};
   use serde::{Deserialize, Serialize};

   // 3. 当前 crate 导入 (按模块分组)
   use crate::{
       db_tree_view::DbTreeView,
       tab_container::{TabContainer, TabContent},
   };
   ```

2. **命名约定**:
   - 结构体/枚举/Trait: 大驼峰 (PascalCase)
   - 函数/变量: 蛇形 (snake_case)
   - 常量: 全大写蛇形 (UPPER_SNAKE_CASE)

3. **错误处理**: 优先使用 `Result<T, Error>`，避免 panic

4. **异步约定**:
   - 数据库操作使用 `async/await`
   - GPUI 上下文中使用 `spawn_result()` 桥接 Tokio

### 添加新数据库类型

1. 在 `crates/db/src/types.rs` 添加 `DatabaseType` 变体
2. 创建新模块 `crates/db/src/<dbname>/`
3. 实现 `DatabasePlugin` trait
4. 实现 `DbConnection` trait
5. 在 `DbManager::get_plugin()` 注册插件
6. 添加连接表单配置

### 测试

- 为每个插件编写单元测试
- 集成测试覆盖主要用户场景
- 使用 `cargo test` 运行所有测试

---

## 许可证

(项目许可证信息)

---

**最后更新**: 2025-01-19 (基于实际代码结构完整重写)
