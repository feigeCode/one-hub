use std::collections::{HashMap, HashSet};
use gpui::{App, AppContext, Context, Entity, IntoElement, InteractiveElement, ParentElement, Render, Styled, Window, div, AnyElement, StatefulInteractiveElement, EventEmitter, SharedString, Focusable, FocusHandle, WeakEntity};
use gpui_component::{
    ActiveTheme, IconName, StyledExt,
    h_flex,
    list::ListItem,
    menu::{ContextMenuExt, PopupMenuItem},
    tree::TreeItem,
    v_flex,
    dock::{Panel, PanelEvent, PanelState},
};
use crate::context_menu_tree::{context_menu_tree, ContextMenuTreeState};
use db::{GlobalDbState, DbNode, DbNodeType, spawn_result};
use gpui_component::button::Button;
use gpui_component::dock::{PanelControl, TabPanel, TitleStyle};
use gpui_component::menu::PopupMenu;
use crate::storage::StoredConnection;
// ============================================================================
// DbTreeView Events
// ============================================================================

/// 数据库树视图事件
#[derive(Debug, Clone)]
pub enum DbTreeViewEvent {
    /// 打开表数据标签页
    OpenTableData { database: String, table: String },
    /// 打开视图数据标签页
    OpenViewData { database: String, view: String },
    /// 打开表结构标签页
    OpenTableStructure { database: String, table: String },
    /// 连接到指定的已保存连接（由名称标识）
    ConnectToConnection {id: String, name: String },
    /// 为指定数据库创建新查询
    CreateNewQuery { database: String },
}

// ============================================================================
// DbTreeView - 数据库连接树视图（支持懒加载）
// ============================================================================

pub struct DbTreeView {
    focus_handle: FocusHandle,
    tree_state: Entity<ContextMenuTreeState>,
    selected_item: Option<TreeItem>,
    // 存储 DbNode 映射 (ID -> DbNode)，用于懒加载
    db_nodes: HashMap<String, DbNode>,
    // 已经懒加载过子节点的集合
    loaded_children: HashSet<String>,
    // 正在加载的节点集合（用于显示加载状态）
    loading_nodes: HashSet<String>,
    // 已展开的节点（用于在重建树时保持展开状态）
    expanded_nodes: HashSet<String>,
    // 当前树的根节点集合，便于我们更新子节点
    items: Vec<TreeItem>,
    // 当前连接名称
    connection_name: Option<String>,
}

impl DbTreeView {
    pub fn new(connections: &Vec<StoredConnection>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let mut db_nodes = HashMap::new();
        let mut init_nodes = vec![];
        if connections.is_empty() {
            let node =  DbNode::new("root", "No Database Connected", DbNodeType::Connection);
            db_nodes.insert(
                "root".to_string(),
               node.clone()
            );
            init_nodes.push( node)
        }else {
            for conn in connections {
                let id = conn.id.unwrap().to_string();
                let node = DbNode::new(id.clone(), conn.name.to_string(), DbNodeType::Connection);
                db_nodes.insert(id, node.clone());
                init_nodes.push(node);
            }
        }
        init_nodes.sort();
        let items = Self::create_initial_tree(init_nodes);
        let clone_items = items.clone();
        let tree_state = cx.new(|cx| {
            ContextMenuTreeState::new(cx).items(items)
        });
        Self {
            focus_handle,
            tree_state,
            selected_item: None,
            db_nodes,
            loaded_children: HashSet::new(),
            loading_nodes: HashSet::new(),
            expanded_nodes: HashSet::new(),
            items: clone_items,
            connection_name: None,
        }
    }

    /// 公开方法：重新加载指定节点的子节点
    pub fn reload_children(&mut self, node_id: String, cx: &mut Context<Self>) {
        self.loaded_children.remove(&node_id);
        if let Some(n) = self.db_nodes.get_mut(&node_id) {
            n.children_loaded = false;
            n.children.clear();
        }
        self.lazy_load_children(node_id, cx);
    }

    /// 公开方法：断开连接并刷新树
    pub fn disconnect(&mut self, cx: &mut Context<Self>) {
        let global_state = cx.global::<GlobalDbState>().clone();
        cx.spawn(async move |this, cx| {
            // Clear current database
            global_state.connection_pool.set_current_database(None).await;

            this.update(cx, |this: &mut Self, cx| {
                this.refresh_tree(cx);
            }).ok();
        }).detach();
    }

    /// 公开方法：设置当前数据库并刷新树
    pub fn set_current_database_and_refresh(&mut self, database: String, cx: &mut Context<Self>) {
        let global_state = cx.global::<GlobalDbState>().clone();
        cx.spawn(async move |this, cx| {
            global_state.connection_pool.set_current_database(Some(database)).await;
            this.update(cx, |this: &mut Self, cx| {
                this.refresh_tree(cx);
            }).ok();
        }).detach();
    }

    /// 设置连接名称
    pub fn set_connection_name(&mut self, name: String) {
        self.connection_name = Some(name);
    }

    /// 更新连接节点为已连接状态，显示数据库列表
    pub fn update_connection_node(&mut self, connection_id: &str, cx: &mut Context<Self>) {
        let node = self.db_nodes.get_mut(connection_id);
        if let Some(node) = node {
            node.has_children = true;
            node.children_loaded = false;
            // 触发节点展开以加载数据库
            self.expanded_nodes.insert(node.id.clone());
            self.lazy_load_children(connection_id.to_string(), cx)
        }
    }

    /// 展开指定的数据库节点
    pub fn expand_database(&mut self, connection_id: &str, database: &str, cx: &mut Context<Self>) {
        // 构建数据库节点 ID
        let db_node_id = format!("{}_db_{}", connection_id, database);
        
        // 标记为已展开
        self.expanded_nodes.insert(db_node_id.clone());
        
        // 如果子节点未加载，触发加载
        if !self.loaded_children.contains(&db_node_id) {
            self.lazy_load_children(db_node_id, cx);
        }
    }

    /// 创建初始树结构（未连接状态）
    fn create_initial_tree(init_nodes: Vec<DbNode>) -> Vec<TreeItem> {
        if init_nodes.is_empty() {
            return vec![
                TreeItem::new("root".to_string(), "No Database Connected".to_string())
            ]
        }
        let mut items: Vec<TreeItem> = Vec::new();
        for node in init_nodes.iter() {
            items.push(TreeItem::new(SharedString::new(node.id.to_string()), SharedString::new(node.name.to_string())))
        }
        items
    }

    /// 将 DbNode 转换为 TreeItem
    fn db_node_to_tree_item(node: &DbNode) -> TreeItem {
        let mut item = TreeItem::new(node.id.clone(), node.name.clone());

        // 如果节点有子节点能力但未加载，设置一个占位子节点让树组件显示展开按钮
        if node.has_children && !node.children_loaded {
            // 使用一个特殊的占位节点
            let placeholder = TreeItem::new(
                format!("{}_placeholder", node.id),
                "Loading...".to_string()
            );
            item = item.children(vec![placeholder]);
        } else if !node.children.is_empty() {
            // 如果已经加载了子节点，递归转换
            let children: Vec<TreeItem> = node
                .children
                .iter()
                .map(Self::db_node_to_tree_item)
                .collect();
            item = item.children(children);
        }

        item
    }

    /// 刷新树结构（从数据库加载数据库列表）
    pub fn refresh_tree(&mut self, cx: &mut Context<Self>) {
        let global_state = cx.global::<GlobalDbState>().clone();
        let tree_state = self.tree_state.clone();

        cx.spawn(async move |this, cx| {
            // 检查是否已连接
            let is_connected = global_state.connection_pool.is_connected().await;

            if !is_connected {
                // 未连接，保留当前的连接列表而不是清空
                return;
            }

            // Get current connection and config
            let conn_arc = match global_state.connection_pool.get_current_connection().await {
                Some(c) => c,
                None => {
                    eprintln!("No current connection");
                    return;
                }
            };

            let config = match global_state.connection_pool.get_current_connection_config().await {
                Some(c) => c,
                None => {
                    eprintln!("No connection config");
                    return;
                }
            };

            // Get plugin
            let plugin = match global_state.db_manager.get_plugin(&config.database_type) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Failed to get plugin: {}", e);
                    return;
                }
            };

            // 获取数据库列表
            let conn = conn_arc.read().await;
            let databases = plugin.list_databases(&**conn).await.unwrap_or_else(|e| {
                eprintln!("Failed to list databases: {}", e);
                vec![]
            });

            // 构建树结构
            this.update(cx, |this: &mut Self, cx| {
                // 只清除数据库相关节点，保留连接节点
                let conn_nodes: Vec<(String, DbNode)> = this.db_nodes
                    .iter()
                    .filter(|(_, n)| n.node_type == DbNodeType::Connection)
                    .map(|(id, n)| (id.clone(), n.clone()))
                    .collect();

                this.db_nodes.clear();
                this.loaded_children.clear();
                this.loading_nodes.clear();
                // 保留对应连接的展开状态
                this.expanded_nodes.retain(|id| conn_nodes.iter().any(|(cid, _)| cid == id));

                // 恢复连接节点
                for (id, node) in conn_nodes {
                    this.db_nodes.insert(id, node);
                }

                let mut db_nodes_vec = Vec::new();
                for db_name in databases.iter() {
                    let db_id = format!("db:{}", db_name);
                    eprintln!("Creating database node: {} with id: {}", db_name, db_id);
                    let db_node = DbNode::new(db_id.clone(), db_name.clone(), DbNodeType::Database)
                        .with_children_flag(true);

                    this.db_nodes.insert(db_id.clone(), db_node.clone());
                    db_nodes_vec.push(db_node);
                }

                eprintln!("Total databases loaded: {}", db_nodes_vec.len());

                // 使用存储的连接名称，如果没有则使用默认值
                let conn_name = this.connection_name.as_deref().unwrap_or("Current Connection");
                // 生成唯一的连接ID
                let conn_id = format!("conn_active:{}", conn_name);

                let mut conn_node = DbNode::new(conn_id.clone(), conn_name, DbNodeType::Connection)
                    .with_children_flag(true);
                conn_node.children = db_nodes_vec;
                conn_node.children_loaded = true;

                this.db_nodes.insert(conn_id.clone(), conn_node.clone());
                this.loaded_children.insert(conn_id.clone());

                let items = vec![Self::db_node_to_tree_item(&conn_node)];
                this.items = items.clone();

                tree_state.update(cx, |state, cx| {
                    state.set_items(items, cx);
                });
            }).ok();
        }).detach();
    }

    /// 懒加载节点的子节点
    fn lazy_load_children(&mut self, node_id: String, cx: &mut Context<Self>) {
        // 如果已经加载过或正在加载，跳过
        if self.loaded_children.contains(&node_id) || self.loading_nodes.contains(&node_id) {
            eprintln!("Skipping {}: already loaded or loading", node_id);
            return;
        }

        // 获取节点信息
        let node = match self.db_nodes.get(&node_id) {
            Some(n) => n.clone(),
            None => {
                eprintln!("Node not found in db_nodes: {}", node_id);
                return;
            }
        };

        eprintln!("Attempting to load children for: {} (type: {:?}, has_children: {})",
                  node_id, node.node_type, node.has_children);

        // 如果节点没有子节点能力，跳过
        if !node.has_children {
            eprintln!("Node {} has no children capability", node_id);
            return;
        }

        // 标记为正在加载
        self.loading_nodes.insert(node_id.clone());
        cx.notify();

        let global_state = cx.global::<GlobalDbState>().clone();
        let clone_node_id = node_id.clone();
        cx.spawn(async move |this, cx| {
            // 使用 DatabasePlugin 的方法加载子节点
            let children_result = spawn_result(async move {
                // 检查是否已连接
                if !global_state.connection_pool.is_connected().await {
                    return Err(anyhow::anyhow!("Not connected to any database"));
                }

                // 获取当前连接和配置
                let conn_arc = match global_state.connection_pool.get_current_connection().await {
                    Some(c) => c,
                    None => return Err(anyhow::anyhow!("No current connection")),
                };

                let config = match global_state.connection_pool.get_current_connection_config().await {
                    Some(c) => c,
                    None => return Err(anyhow::anyhow!("No connection config")),
                };

                // 获取插件
                let plugin = global_state.db_manager.get_plugin(&config.database_type)?;

                // 锁定连接
                let conn = conn_arc.read().await;

                // 加载子节点并返回结果
                plugin.load_node_children(&**conn, &node).await
            }).await;

            this.update(cx, |this: &mut Self, cx| {
                // 移除加载状态
                this.loading_nodes.remove(&clone_node_id);

                match children_result {
                    Ok(children) => {
                        eprintln!("Loaded {} children for node: {}", children.len(), clone_node_id);
                        // 标记为已加载
                        this.loaded_children.insert(clone_node_id.clone());

                        // 更新节点的子节点
                        if let Some(parent_node) = this.db_nodes.get_mut(&clone_node_id) {
                            parent_node.children = children.clone();
                            parent_node.children_loaded = true;
                        }

                        // 递归地将所有子节点及其后代添加到 db_nodes
                        fn insert_nodes_recursive(
                            db_nodes: &mut HashMap<String, DbNode>,
                            node: &DbNode,
                        ) {
                            db_nodes.insert(node.id.clone(), node.clone());
                            for child in &node.children {
                                insert_nodes_recursive(db_nodes, child);
                            }
                        }

                        for child in &children {
                            eprintln!("  - Adding child: {} (type: {:?})", child.id, child.node_type);
                            insert_nodes_recursive(&mut this.db_nodes, child);
                        }

                        // 重建树结构
                        this.rebuild_tree(cx);
                    }
                    Err(e) => {
                        eprintln!("Failed to load children for {}: {}", clone_node_id, e);
                    }
                }
            }).ok();
        }).detach();
    }

    /// 重建整个树结构（保留连接列表）
    pub fn rebuild_tree(&mut self, cx: &mut Context<Self>) {
        // 从真正的根节点重建（不依赖 self.items，因为它可能过时）
        // 找到所有顶层节点（在 db_nodes 中但不是任何节点的子节点）
        let mut root_nodes: Vec<DbNode> = Vec::new();

        for node in self.db_nodes.values() {
            if node.parent_context == None {
                root_nodes.push(node.clone());
            }
        }

        // 如果没有根节点，保留当前的树
        if root_nodes.is_empty() {
            return;
        }
        // 排序
        root_nodes.sort();

        // 使用找到的根节点ID构建树
        let root_items: Vec<TreeItem> = root_nodes
            .iter()
            .map(|node| {
                Self::db_node_to_tree_item_recursive(node, &self.db_nodes, &self.expanded_nodes)
            })
            .collect();
        // 只有当有新的items时才更新
        if !root_items.is_empty() {
            self.items = root_items.clone();
            self.tree_state.update(cx, |state, cx| {
                state.set_items(root_items, cx);
            });
        }
    }

    /// 递归构建 TreeItem，使用 db_nodes 映射
    fn db_node_to_tree_item_recursive(
        node: &DbNode,
        db_nodes: &HashMap<String, DbNode>,
        expanded_nodes: &HashSet<String>,
    ) -> TreeItem {
        let mut item = TreeItem::new(node.id.clone(), node.name.clone());

        // 保持展开状态
        if expanded_nodes.contains(&node.id) {
            item = item.expanded(true);
        }

        if node.children_loaded {
            if !node.children.is_empty() {
                let children: Vec<TreeItem> = node
                    .children
                    .iter()
                    .map(|child_node| {
                        // 优先使用 db_nodes 中的最新版本，避免使用过期的克隆
                        if let Some(updated) = db_nodes.get::<str>(child_node.id.as_ref()) {
                            Self::db_node_to_tree_item_recursive(updated, db_nodes, expanded_nodes)
                        } else {
                            Self::db_node_to_tree_item_recursive(child_node, db_nodes, expanded_nodes)
                        }
                    })
                    .collect();
                item = item.children(children);
            } else {
                // 已加载且为空：不要添加占位节点，保持为叶子
            }
        } else if node.has_children {
            // 有子节点但未加载，设置占位节点以显示展开箭头
            let placeholder = TreeItem::new(
                format!("{}_placeholder", node.id),
                "Loading...".to_string()
            );
            item = item.children(vec![placeholder]);
        }

        item
    }

    /// 根据节点类型获取图标
    fn get_icon_for_node(&self, node_id: &str, is_expanded: bool) -> IconName {
        let node = self.db_nodes.get(node_id);
        match node.map(|n| &n.node_type) {
            Some(DbNodeType::Connection) => IconName::Building2,
            Some(DbNodeType::Database) => if is_expanded { IconName::FolderOpen } else { IconName::Folder },
            Some(DbNodeType::TablesFolder) | Some(DbNodeType::ViewsFolder) |
            Some(DbNodeType::FunctionsFolder) | Some(DbNodeType::ProceduresFolder) |
            Some(DbNodeType::TriggersFolder) | Some(DbNodeType::SequencesFolder) => {
                if is_expanded { IconName::FolderOpen } else { IconName::Folder }
            }
            Some(DbNodeType::Table) => IconName::LayoutDashboard,
            Some(DbNodeType::View) => IconName::Eye,
            Some(DbNodeType::Function) | Some(DbNodeType::Procedure) => IconName::Settings,
            Some(DbNodeType::Column) => IconName::Dash,
            Some(DbNodeType::ColumnsFolder) | Some(DbNodeType::IndexesFolder) => {
                if is_expanded { IconName::FolderOpen } else { IconName::Folder }
            }
            Some(DbNodeType::Index) => IconName::Settings,
            Some(DbNodeType::Trigger) => IconName::Settings,
            Some(DbNodeType::Sequence) => IconName::ArrowRight,
            _ => IconName::File,
        }
    }

    fn handle_item_double_click(&mut self, item: TreeItem, cx: &mut Context<Self>) {
        // 根据节点类型执行不同的操作
        if let Some(node) = self.db_nodes.get(item.id.as_ref()).cloned() {
            match node.node_type {
                DbNodeType::Connection => {
                    // 连接项：由 AppView 处理实际连接逻辑
                    cx.emit(DbTreeViewEvent::ConnectToConnection {id: node.id.clone(), name: node.name.clone() });
                }
                DbNodeType::Table => {
                    // 查找所属数据库
                    if let Some(database) = self.find_parent_database(&node.id) {
                        eprintln!("Opening table data tab: {}.{}", database, node.name);
                        cx.emit(DbTreeViewEvent::OpenTableData {
                            database,
                            table: node.name.clone(),
                        });
                    }
                }
                DbNodeType::View => {
                    // 查找所属数据库
                    if let Some(database) = self.find_parent_database(&node.id) {
                        eprintln!("Opening view data tab: {}.{}", database, node.name);
                        cx.emit(DbTreeViewEvent::OpenViewData {
                            database,
                            view: node.name.clone(),
                        });
                    }
                }
                _ => {
                    // 其他类型的节点暂不处理双击
                }
            }
        }
        cx.notify();
    }

    /// 查找节点所属的数据库名称
    fn find_parent_database(&self, node_id: &str) -> Option<String> {
        // 向上遍历查找数据库节点
        let mut current_id = node_id.to_string();

        while let Some(node) = self.db_nodes.get(&current_id) {
            if node.node_type == DbNodeType::Database {
                return Some(node.name.clone());
            }

            // 查找父节点
            let parent_found = self.db_nodes.values().find(|parent| {
                parent.children.iter().any(|child| child.id == current_id)
            });

            if let Some(parent) = parent_found {
                current_id = parent.id.clone();
            } else {
                break;
            }
        }

        None
    }
}

impl Render for DbTreeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();

        v_flex()
            .id("db-tree-view")
            .size_full()
            .bg(cx.theme().background)
            .child(
                // 树形视图
                v_flex()
                    .flex_1()
                    .w_full()
                    .bg(cx.theme().muted.opacity(0.3))
                    .child(
                        div()
                            .id("tree-scroll")
                            .flex_1()
                            .overflow_scroll()
                            .p_2()
                            .child({
                                let view_for_click = view.clone();
                                let view_for_double_click = view.clone();

                                context_menu_tree(
                                    &self.tree_state,
                                    move |ix, item, depth, _selected, window, cx| {
                                        let node_id = item.id.to_string();
                                        let (icon, label_text, _item_clone) = view.update(cx, |this, _cx| {
                                            let icon = this.get_icon_for_node(&node_id, item.is_expanded());

                                            // 同步节点展开状态
                                            if item.is_expanded() {
                                                this.expanded_nodes.insert(item.id.to_string());
                                            } else {
                                                this.expanded_nodes.remove(item.id.as_ref());
                                            }

                                            // 显示加载状态
                                            let is_loading = this.loading_nodes.contains(&node_id);
                                            let label_text = if is_loading {
                                                format!("{} (Loading...)", item.label)
                                            } else {
                                                item.label.to_string()
                                            };

                                            (icon, label_text, item.clone())
                                        });

                                        // 在 update 之后触发懒加载
                                        if item.is_expanded() {
                                            let id = node_id.clone();
                                            view.update(cx, |this, cx| {
                                                this.lazy_load_children(id, cx);
                                            });
                                        }

                                        // 创建 ListItem (不再添加 on_click，缩进由 context_menu_tree 处理)
                                        let view_clone = view.clone();
                                        let node_id_clone = node_id.clone();
                                        println!("node_id: {}, item: {}", &node_id, &item.label);
                                        let list_item = ListItem::new(ix)
                                            .w_full()
                                            .rounded(cx.theme().radius)
                                            .px_2()
                                            .py_1()
                                            .child(
                                                h_flex()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(icon)
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .child(label_text)
                                                    )
                                            );

                                        // 使用 context_menu 方法为 ListItem 添加上下文菜单
                                        list_item
                                            .context_menu(move |menu, window, cx| {
                                                        // 从 db_nodes 获取节点信息
                                                        if let Some(node) = view_clone.read(cx).db_nodes.get(&node_id_clone).cloned() {
                                                            let node_type = format!("{:?}", node.node_type);
                                                            let has_children = node.has_children;

                                                            let mut menu = menu
                                                                .label(format!("Type: {}", node_type))
                                                                .separator();

                                                            // 根据节点类型添加不同的菜单项
                                                            match node.node_type {
                                                                DbNodeType::Database => {
                                                                    let view_ref2 = view_clone.clone();
                                                                    let db_name = node.name.clone();
                                                                    let db_name_for_query = db_name.clone();

                                                                    menu = menu
                                                                        .item(
                                                                            PopupMenuItem::new("New Query")
                                                                                .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                    eprintln!("Creating new query for database: {}", db_name_for_query);
                                                                                    cx.emit(DbTreeViewEvent::CreateNewQuery {
                                                                                        database: db_name_for_query.clone(),
                                                                                    });
                                                                                }))
                                                                        )
                                                                        .separator()
                                                                        .item(
                                                                            PopupMenuItem::new("Set as Current Database")
                                                                                .on_click(window.listener_for(&view_ref2, move |this, _, _, cx| {
                                                                                    this.set_current_database_and_refresh(db_name.clone(), cx);
                                                                                }))
                                                                        )
                                                                        .separator();
                                                                }
                                                                DbNodeType::Table => {
                                                                    let table_name = node.name.clone();
                                                                    let database_name = node.parent_context.clone().unwrap_or_else(|| "unknown".to_string());

                                                                    menu = menu
                                                                        .item(PopupMenuItem::new("View Table Data"))
                                                                        .item(PopupMenuItem::new("Export Table"))
                                                                        .item(
                                                                            PopupMenuItem::new("Edit Table")
                                                                            .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                eprintln!("Opening table structure tab: {}.{}", database_name, table_name);
                                                                                cx.emit(DbTreeViewEvent::OpenTableStructure {
                                                                                    database: database_name.clone(),
                                                                                    table: table_name.clone(),
                                                                                });
                                                                            }))
                                                                        )
                                                                        .separator();
                                                                }
                                                                DbNodeType::Connection => {
                                                                    let view_ref2 = view_clone.clone();
                                                                    menu = menu
                                                                        .item(
                                                                            PopupMenuItem::new("Disconnect")
                                                                                .on_click(window.listener_for(&view_ref2, |this, _, _, cx| {
                                                                                    this.disconnect(cx);
                                                                                }))
                                                                        )
                                                                        .separator();
                                                                }
                                                                _ => {}
                                                            }

                                                            // 添加通用的刷新选项
                                                            if has_children {
                                                                let view_ref2 = view_clone.clone();
                                                                let id_clone = node_id_clone.clone();
                                                                menu = menu.item(
                                                                    PopupMenuItem::new("Load Children")
                                                                        .on_click(window.listener_for(&view_ref2, move |this, _, _, cx| {
                                                                            this.reload_children(id_clone.clone(), cx);
                                                                        }))
                                                                );
                                                            }

                                                            let view_ref2 = view_clone.clone();
                                                            let id_clone = node_id_clone.clone();
                                                            menu.item(
                                                                PopupMenuItem::new("Refresh Node")
                                                                    .on_click(window.listener_for(&view_ref2, move |this, _, _, cx| {
                                                                        this.reload_children(id_clone.clone(), cx);
                                                                    }))
                                                            )
                                                        } else {
                                                            menu
                                                        }
                                            })
                                            .into_any_element()
                                    },
                                )
                                .on_click({
                                    move |_ix, item, cx| {
                                        view_for_click.update(cx, |this, cx| {
                                            this.selected_item = Some(item.clone());
                                            cx.notify();
                                        });
                                    }
                                })
                                .on_double_click({
                                    move |_ix, item, cx| {
                                        view_for_double_click.update(cx, |this, cx| {
                                            this.handle_item_double_click(item.clone(), cx);
                                        });
                                    }
                                })
                            })
                    )
            )
            // .child(
            //     // 状态栏
            //     div()
            //         .w_full()
            //         .p_2()
            //         .bg(cx.theme().muted.opacity(0.5))
            //         .border_t_1()
            //         .border_color(cx.theme().border)
            //         .child(
            //             h_flex()
            //                 .w_full()
            //                 .justify_between()
            //                 .items_center()
            //                 .gap_3()
            //                 .child(
            //                     div()
            //                         .text_xs()
            //                         .text_color(cx.theme().muted_foreground)
            //                         .children(
            //                             self.tree_state
            //                                 .read(cx)
            //                                 .selected_index()
            //                                 .map(|ix| format!("Selected: {}", ix))
            //                         )
            //                 )
            //                 .child(
            //                     div()
            //                         .text_xs()
            //                         .text_color(cx.theme().muted_foreground)
            //                         .children(
            //                             self.selected_item
            //                                 .as_ref()
            //                                 .map(|item| item.label.clone())
            //                         )
            //                 )
            //         )
            // )
    }
}

impl EventEmitter<DbTreeViewEvent> for DbTreeView {}

impl EventEmitter<PanelEvent> for DbTreeView {}

impl Focusable for DbTreeView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for DbTreeView {
    fn panel_name(&self) -> &'static str {
        "DbTreeView"
    }

    fn tab_name(&self, cx: &App) -> Option<SharedString> {
        Some("Database Tree".into())
    }

    fn title(&self, window: &Window, cx: &App) -> AnyElement {
        h_flex()
            .items_center()
            .gap_2()
            .child("Database Explorer")
            .into_any_element()
    }

    fn title_style(&self, cx: &App) -> Option<TitleStyle> {
        None
    }

    fn title_suffix(&self, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
        None
    }

    fn closable(&self, cx: &App) -> bool {
        false
    }

    fn zoomable(&self, cx: &App) -> Option<PanelControl> {
        None
    }

    fn visible(&self, cx: &App) -> bool {
        true
    }

    fn set_active(&mut self, active: bool, window: &mut Window, cx: &mut App) {
        // No special handling needed for active state
    }

    fn set_zoomed(&mut self, zoomed: bool, window: &mut Window, cx: &mut App) {
        // No special handling needed for zoomed state
    }

    fn on_added_to(&mut self, tab_panel: WeakEntity<TabPanel>, window: &mut Window, cx: &mut App) {
        // No special handling needed when added to tab panel
    }

    fn on_removed(&mut self, window: &mut Window, cx: &mut App) {
        // No special handling needed when removed
    }

    fn dropdown_menu(&self, this: PopupMenu, window: &Window, cx: &App) -> PopupMenu {
        this
    }

    fn toolbar_buttons(&self, window: &mut Window, cx: &mut App) -> Option<Vec<Button>> {
        None
    }

    fn dump(&self, cx: &App) -> PanelState {
        PanelState::new(self)
    }

    fn inner_padding(&self, cx: &App) -> bool {
        false
    }
}

