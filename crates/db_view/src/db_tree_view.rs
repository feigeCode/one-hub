use core::storage::StoredConnection;
use std::collections::{HashMap, HashSet};
use gpui::{App, AppContext, Context, Entity, IntoElement, InteractiveElement, ParentElement, Render, Styled, Window, div, StatefulInteractiveElement, EventEmitter, SharedString, Focusable, FocusHandle, WeakEntity};
use tracing::log::trace;
use gpui_component::{
    ActiveTheme, IconName,
    h_flex,
    list::ListItem,
    menu::{ContextMenuExt, PopupMenuItem},
    tree::TreeItem,
    v_flex,
};
use db::{GlobalDbState, DbNode, DbNodeType, spawn_result, DbConnectionConfig};
use gpui_component::context_menu_tree::{context_menu_tree, ContextMenuTreeState};
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
    /// 为指定数据库创建新查询
    CreateNewQuery { database: String },
    /// 节点被选中（用于更新 objects panel）
    NodeSelected { node_id: String },
    /// 导入数据
    ImportData { database: String, table: Option<String> },
    /// 导出数据
    ExportData { database: String, tables: Vec<String> },
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
    // 数据库连接配置
    config: DbConnectionConfig,
}

impl DbTreeView {
    pub fn new(connection: StoredConnection, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let tree_state = cx.new(|cx| {
            ContextMenuTreeState::new(cx)
        });
        Self {
            focus_handle,
            tree_state,
            selected_item: None,
            db_nodes: HashMap::new(),
            loaded_children: HashSet::new(),
            loading_nodes: HashSet::new(),
            expanded_nodes: HashSet::new(),
            items: vec![],
            connection_name: None,
            config: connection.to_db_connection(),
        }
    }

    /// 重新加载指定节点的子节点
    pub fn reload_children(&mut self, node_id: String, cx: &mut Context<Self>) {
        self.loaded_children.remove(&node_id);
        if let Some(n) = self.db_nodes.get_mut(&node_id) {
            n.children_loaded = false;
            n.children.clear();
        }
        self.lazy_load_children(node_id, cx);
    }


    /// 设置连接名称
    pub fn set_connection_name(&mut self, name: String) {
        self.connection_name = Some(name);
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
        let config = self.config.clone();
        
        cx.spawn(async move |this, cx| {
            // Get plugin
            let plugin = match global_state.db_manager.get_plugin(&config.database_type) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Failed to get plugin: {}", e);
                    return;
                }
            };
            
            let conn_arc = match global_state.connection_pool.get_connection(config, &global_state.db_manager).await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to get connection: {}", e);
                    return;
                }
            };
            
            let conn = conn_arc.read().await;
            let databases = match plugin.list_databases(&**conn).await {
                Ok(dbs) => dbs,
                Err(e) => {
                    eprintln!("Failed to list databases: {}", e);
                    return;
                }
            };
            
            eprintln!("Loaded {} databases", databases.len());
            
            // 构建树结构
            this.update(cx, |this: &mut Self, cx| {
                // 清空旧数据
                this.db_nodes.clear();
                this.loaded_children.clear();
                this.loading_nodes.clear();
                this.expanded_nodes.clear();
                
                // 创建数据库节点
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
                
                // 转换为 TreeItem
                let items: Vec<TreeItem> = db_nodes_vec
                    .iter()
                    .map(|node| Self::db_node_to_tree_item(node))
                    .collect();
                
                this.items = items.clone();
                
                // 更新树状态
                tree_state.update(cx, |state, cx| {
                    state.set_items(items, cx);
                });
                
                // 自动展开第一个数据库
                if let Some(first_db) = db_nodes_vec.first() {
                    let db_id = first_db.id.clone();
                    eprintln!("Auto-expanding first database: {}", db_id);
                    this.expanded_nodes.insert(db_id.clone());
                    this.lazy_load_children(db_id, cx);
                }
                
                cx.notify();
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
        let config = self.config.clone();
        cx.spawn(async move |this, cx| {
            // 使用 DatabasePlugin 的方法加载子节点
            let children_result = spawn_result(async move {
                // 检查是否已连接
                let plugin = global_state.db_manager.get_plugin(&config.database_type)?;
                let conn_arc = global_state.connection_pool.get_connection(config, &global_state.db_manager).await?;
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
            Some(DbNodeType::Database) => IconName::DATABASE,
            Some(DbNodeType::TablesFolder) | Some(DbNodeType::ViewsFolder) |
            Some(DbNodeType::FunctionsFolder) | Some(DbNodeType::ProceduresFolder) |
            Some(DbNodeType::TriggersFolder) | Some(DbNodeType::SequencesFolder) => {
                if is_expanded { IconName::FolderOpen } else { IconName::Folder }
            }
            Some(DbNodeType::Table) => IconName::TABLE,
            Some(DbNodeType::View) => IconName::TABLE,
            Some(DbNodeType::Function) | Some(DbNodeType::Procedure) => IconName::Settings,
            Some(DbNodeType::Column) => IconName::COLUMN,
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

    /// 获取节点信息（公开方法）
    pub fn get_node(&self, node_id: &str) -> Option<&DbNode> {
        self.db_nodes.get(node_id)
    }

    /// 获取当前选中的数据库名称
    pub fn get_selected_database(&self) -> Option<String> {
        if let Some(item) = &self.selected_item {
            // 从选中的节点ID中提取数据库名
            if let Some(node) = self.db_nodes.get(item.id.as_ref()) {
                match node.node_type {
                    db::types::DbNodeType::Database => {
                        return Some(node.name.clone());
                    }
                    _ => {
                        // 从父节点上下文中查找数据库
                        return self.find_parent_database(item.id.as_ref());
                    }
                }
            }
        }
        None
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
                                        trace!("node_id: {}, item: {}", &node_id, &item.label);
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
                                                                    let db_name = node.name.clone();
                                                                    let db_name_for_query = db_name.clone();
                                                                    let db_name_for_import = db_name.clone();
                                                                    let db_name_for_export = db_name.clone();

                                                                    menu = menu
                                                                        .item(
                                                                            PopupMenuItem::new("New Query")
                                                                                .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                    cx.emit(DbTreeViewEvent::CreateNewQuery {
                                                                                        database: db_name_for_query.clone(),
                                                                                    });
                                                                                }))
                                                                        )
                                                                        .separator()
                                                                        .item(
                                                                            PopupMenuItem::new("Import Data")
                                                                                .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                    cx.emit(DbTreeViewEvent::ImportData {
                                                                                        database: db_name_for_import.clone(),
                                                                                        table: None,
                                                                                    });
                                                                                }))
                                                                        )
                                                                        .item(
                                                                            PopupMenuItem::new("Export Database")
                                                                                .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                    cx.emit(DbTreeViewEvent::ExportData {
                                                                                        database: db_name_for_export.clone(),
                                                                                        tables: vec![],
                                                                                    });
                                                                                }))
                                                                        )
                                                                        .separator();
                                                                }
                                                                DbNodeType::Table => {
                                                                    let table_name = node.name.clone();
                                                                    // TODO 获取真实数据
                                                                    let database_name = "ai_app".to_string();
                                                                    let table_for_import = table_name.clone();
                                                                    let db_for_import = database_name.clone();
                                                                    let table_for_export = table_name.clone();
                                                                    let db_for_export = database_name.clone();

                                                                    menu = menu
                                                                        .item(PopupMenuItem::new("View Table Data"))
                                                                        .item(
                                                                            PopupMenuItem::new("Edit Table")
                                                                            .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                cx.emit(DbTreeViewEvent::OpenTableStructure {
                                                                                    database: database_name.clone(),
                                                                                    table: table_name.clone(),
                                                                                });
                                                                            }))
                                                                        )
                                                                        .separator()
                                                                        .item(
                                                                            PopupMenuItem::new("Import to Table")
                                                                                .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                    cx.emit(DbTreeViewEvent::ImportData {
                                                                                        database: db_for_import.clone(),
                                                                                        table: Some(table_for_import.clone()),
                                                                                    });
                                                                                }))
                                                                        )
                                                                        .item(
                                                                            PopupMenuItem::new("Export Table")
                                                                                .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                    cx.emit(DbTreeViewEvent::ExportData {
                                                                                        database: db_for_export.clone(),
                                                                                        tables: vec![table_for_export.clone()],
                                                                                    });
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
                                            // 发出节点选择事件
                                            cx.emit(DbTreeViewEvent::NodeSelected {
                                                node_id: item.id.to_string(),
                                            });
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
    }
}

impl EventEmitter<DbTreeViewEvent> for DbTreeView {}


impl Focusable for DbTreeView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

