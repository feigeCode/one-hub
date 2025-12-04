use one_core::storage::StoredConnection;
use std::collections::{HashMap, HashSet};
use gpui::{App, AppContext, Context, Entity, IntoElement, InteractiveElement, ParentElement, Render, Styled, Window, div, StatefulInteractiveElement, EventEmitter, SharedString, Focusable, FocusHandle};
use tracing::log::trace;
use gpui_component::{ActiveTheme, IconName, h_flex, list::ListItem, menu::{ContextMenuExt, PopupMenuItem}, tree::TreeItem, v_flex, Icon, Sizable, Size};
use db::{GlobalDbState, DbNode, DbNodeType, spawn_result};
use gpui_component::context_menu_tree::{context_menu_tree, ContextMenuTreeState};
// ============================================================================
// DbTreeView Events
// ============================================================================

/// 数据库树视图事件
#[derive(Debug, Clone)]
pub enum DbTreeViewEvent {
    /// 打开表数据标签页
    OpenTableData { node: DbNode },
    /// 打开视图数据标签页
    OpenViewData { node: DbNode },
    /// 打开表结构标签页
    OpenTableStructure { node: DbNode },
    /// 为指定数据库创建新查询
    CreateNewQuery { node: DbNode },
    /// 节点被选中（用于更新 objects panel）
    NodeSelected { node: DbNode },
    /// 导入数据
    ImportData { node: DbNode },
    /// 导出数据
    ExportData { node: DbNode },
    /// 关闭连接
    CloseConnection { node: DbNode },
    /// 编辑连接
    EditConnection { node: DbNode },
    /// 删除连接
    DeleteConnection { node: DbNode },
    /// 编辑数据库
    EditDatabase { node: DbNode },
    /// 关闭数据库
    CloseDatabase { node: DbNode },
    /// 删除数据库
    DeleteDatabase { node: DbNode },
    /// 删除表
    DeleteTable { node: DbNode },
    /// 重命名表
    RenameTable { node: DbNode },
    /// 清空表
    TruncateTable { node: DbNode },
    /// 删除视图
    DeleteView { node: DbNode },
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
    // 当前连接名称或者工作区名称
    connection_name: Option<String>,
    // 工作区ID
    _workspace_id: Option<i64>,
}

impl DbTreeView {
    pub fn new(connections: &Vec<StoredConnection>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let mut db_nodes = HashMap::new();
        let mut init_nodes = vec![];
        let mut workspace_id = None;
        if connections.is_empty() {
            let node =  DbNode::new("root", "No Database Connected", DbNodeType::Connection, "".to_string());
            db_nodes.insert(
                "root".to_string(),
                node.clone()
            );
            init_nodes.push( node)
        }else {
            for conn in connections {
                workspace_id = conn.workspace_id.clone();
                let id = conn.id.unwrap().to_string();
                let node = DbNode::new(id.clone(), conn.name.to_string(), DbNodeType::Connection, id.clone());
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
            _workspace_id: workspace_id,
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

    /// 设置连接名称
    pub fn set_connection_name(&mut self, name: String) {
        self.connection_name = Some(name);
    }


    /// 刷新指定节点及其子节点
    /// 
    /// 这个方法会：
    /// 1. 清除节点的子节点缓存
    /// 2. 递归清除所有后代节点
    /// 3. 重新加载子节点
    /// 4. 如果节点已展开，保持展开状态
    pub fn refresh_tree(&mut self, node_id: String, cx: &mut Context<Self>) {
        eprintln!("Refreshing node: {}", node_id);
        
        // 递归清除节点及其所有后代
        self.clear_node_descendants(&node_id);
        
        // 清除加载状态
        self.loaded_children.remove(&node_id);
        self.loading_nodes.remove(&node_id);
        
        // 重置节点状态
        if let Some(node) = self.db_nodes.get_mut(&node_id) {
            node.children_loaded = false;
            node.children.clear();
        }
        
        // 如果节点已展开，重新加载子节点
        if self.expanded_nodes.contains(&node_id) {
            self.lazy_load_children(node_id, cx);
        } else {
            // 如果节点未展开，只需重建树以更新占位符
            self.rebuild_tree(cx);
        }
    }
    
    /// 递归清除节点的所有后代
    fn clear_node_descendants(&mut self, node_id: &str) {
        // 获取当前节点的所有子节点ID
        let child_ids: Vec<String> = if let Some(node) = self.db_nodes.get(node_id) {
            node.children.iter().map(|c| c.id.clone()).collect()
        } else {
            return;
        };
        
        // 递归清除每个子节点
        for child_id in child_ids {
            self.clear_node_descendants(&child_id);
            
            // 从所有集合中移除子节点
            self.db_nodes.remove(&child_id);
            self.loaded_children.remove(&child_id);
            self.loading_nodes.remove(&child_id);
            self.expanded_nodes.remove(&child_id);
        }
    }

    /// 懒加载节点的子节点
    fn lazy_load_children(&mut self, node_id: String, cx: &mut Context<Self>) {
        // 如果已经加载过或正在加载，跳过
        if self.loaded_children.contains(&node_id) || self.loading_nodes.contains(&node_id) {
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
        // if !node.has_children {
        //     eprintln!("Node {} has no children capability", node_id);
        //     return;
        // }

        // 标记为正在加载
        self.loading_nodes.insert(node_id.clone());
        cx.notify();

        let global_state = cx.global::<GlobalDbState>().clone();
        let clone_node_id = node_id.clone();
        let connection_id = node.connection_id.clone();
        cx.spawn(async move |this, cx| {
            // 使用 DatabasePlugin 的方法加载子节点
            let children_result = spawn_result(async move {
                let (plugin, conn_arc) = global_state.get_plugin_and_connection(&connection_id).await?;
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

    /// 递归切换树项的展开状态
    fn toggle_tree_item_expanded(item: &TreeItem, target_id: &str, current_expanded: bool) -> TreeItem {
        let mut new_item = TreeItem::new(item.id.clone(), item.label.clone())
            .expanded(if item.id.as_ref() == target_id {
                !current_expanded
            } else {
                item.is_expanded()
            });

        for child in &item.children {
            new_item = new_item.child(Self::toggle_tree_item_expanded(child, target_id, current_expanded));
        }

        new_item
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
    fn get_icon_for_node(&self, node_id: &str, is_expanded: bool, cx: &mut Context<Self>) -> Icon {
        let node = self.db_nodes.get(node_id);
        match node.map(|n| &n.node_type) {
            Some(DbNodeType::Connection) => Icon::from(IconName::MySQLLineColor.color().with_size(Size::Large)),
            Some(DbNodeType::Database) => Icon::from(IconName::Database).text_color(cx.theme().primary),
            Some(DbNodeType::TablesFolder) | Some(DbNodeType::ViewsFolder) |
            Some(DbNodeType::FunctionsFolder) | Some(DbNodeType::ProceduresFolder) |
            Some(DbNodeType::TriggersFolder) | Some(DbNodeType::SequencesFolder) => {
                if is_expanded { Icon::new(IconName::FolderOpen).text_color(cx.theme().primary) } else { Icon::from(IconName::Folder).text_color(cx.theme().primary) }
            }
            Some(DbNodeType::Table) => Icon::from(IconName::Table).text_color(cx.theme().primary),
            Some(DbNodeType::View) => Icon::from(IconName::Table),
            Some(DbNodeType::Function) | Some(DbNodeType::Procedure) => Icon::from(IconName::Settings),
            Some(DbNodeType::Column) => Icon::from(IconName::Column).text_color(cx.theme().primary),
            Some(DbNodeType::ColumnsFolder) | Some(DbNodeType::IndexesFolder) => {
                if is_expanded { Icon::from(IconName::FolderOpen).text_color(cx.theme().primary) } else { Icon::from(IconName::Folder).text_color(cx.theme().primary) }
            }
            Some(DbNodeType::Index) => Icon::from(IconName::Settings),
            Some(DbNodeType::Trigger) => Icon::from(IconName::Settings),
            Some(DbNodeType::Sequence) => Icon::from(IconName::ArrowRight),
            _ => Icon::from(IconName::File),
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
                            node
                        });
                    }
                }
                DbNodeType::View => {
                    // 查找所属数据库
                    if let Some(database) = self.find_parent_database(&node.id) {
                        eprintln!("Opening view data tab: {}.{}", database, node.name);
                        cx.emit(DbTreeViewEvent::OpenViewData {
                            node
                        });
                    }
                }
                DbNodeType::Connection | DbNodeType::Database => {
                    let node_id = item.id.to_string();
                    let is_expanded = self.expanded_nodes.contains(&node_id);
                    
                    // 切换展开状态
                    if is_expanded {
                        self.expanded_nodes.remove(&node_id);
                    } else {
                        self.expanded_nodes.insert(node_id.clone());
                    }
                    
                    // 手动切换树状态中的展开状态
                    let tree_state = self.tree_state.clone();
                    tree_state.update(cx, |state, cx| {
                        // 找到根节点并切换展开状态
                        let new_items: Vec<TreeItem> = state.entries
                            .iter()
                            .filter(|e| e.depth == 0 && e.item.id == node_id)
                            .map(|e| {
                                Self::toggle_tree_item_expanded(&e.item, &node_id, is_expanded)
                            })
                            .collect();
                        
                        state.set_items(new_items, cx);
                    });
                    
                    // 如果是展开操作，加载子节点
                    if !is_expanded {
                        self.lazy_load_children(node_id, cx);
                    }
                }
                _ => {
                    // 其他类型的节点暂不处理双击
                }
            }
        }
        cx.notify();
    }

    
    fn handle_item_click(&mut self, item: TreeItem, cx: &mut Context<Self>) {
        self.selected_item = Some(item.clone());
        if let Some(node) = self.db_nodes.get(item.id.as_ref()).cloned() {
            // 发出节点选择事件
            cx.emit(DbTreeViewEvent::NodeSelected {
                node
            });
            cx.notify();
        }
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
                return match node.node_type {
                    DbNodeType::Database => {
                        Some(node.name.clone())
                    }
                    _ => {
                        // 从父节点上下文中查找数据库
                        self.find_parent_database(item.id.as_ref())
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
                                    move |ix, item, _depth, _selected, _window, cx| {
                                        let node_id = item.id.to_string();
                                        let (icon, label_text, _item_clone) = view.update(cx, |this, cx| {
                                            let icon = this.get_icon_for_node(&node_id, item.is_expanded(),cx);

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

                                                            let mut menu = menu
                                                                .label(format!("Type: {}", node_type))
                                                                .separator();
                                                            
                                                            // 根据节点类型添加不同的菜单项
                                                            match node.node_type {
                                                                DbNodeType::Connection => {
                                                                    let node1 = node.clone();
                                                                    let node2 = node.clone();
                                                                    let node3 = node.clone();
                                                                    
                                                                    menu = menu
                                                                        .item(
                                                                            PopupMenuItem::new("关闭连接")
                                                                                .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                    cx.emit(DbTreeViewEvent::CloseConnection {
                                                                                        node: node1.clone()
                                                                                    });
                                                                                }))
                                                                        )
                                                                        .item(
                                                                            PopupMenuItem::new("编辑连接")
                                                                                .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                    cx.emit(DbTreeViewEvent::EditConnection {
                                                                                        node: node2.clone()
                                                                                    });
                                                                                }))
                                                                        )
                                                                        .separator()
                                                                        .item(
                                                                            PopupMenuItem::new("删除连接")
                                                                                .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                    cx.emit(DbTreeViewEvent::DeleteConnection {
                                                                                        node: node3.clone()
                                                                                    });
                                                                                }))
                                                                        )
                                                                        .separator();
                                                                }
                                                                DbNodeType::Database => {
                                                                    let node1 = node.clone();
                                                                    let node2 = node.clone();
                                                                    let node3 = node.clone();
                                                                    let node4 = node.clone();
                                                                    let node5 = node.clone();
                                                                    let node6 = node.clone();
                                                                    
                                                                    menu = menu
                                                                        .item(
                                                                            PopupMenuItem::new("新建查询")
                                                                                .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                    cx.emit(DbTreeViewEvent::CreateNewQuery {
                                                                                        node: node1.clone()
                                                                                    });
                                                                                }))
                                                                        )
                                                                        .separator()
                                                                        .item(
                                                                            PopupMenuItem::new("编辑数据库")
                                                                                .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                    cx.emit(DbTreeViewEvent::EditDatabase {
                                                                                        node: node2.clone()
                                                                                    });
                                                                                }))
                                                                        )
                                                                        .item(
                                                                            PopupMenuItem::new("关闭数据库")
                                                                                .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                    cx.emit(DbTreeViewEvent::CloseDatabase {
                                                                                        node: node3.clone()
                                                                                    });
                                                                                }))
                                                                        )
                                                                        .item(
                                                                            PopupMenuItem::new("删除数据库")
                                                                                .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                    cx.emit(DbTreeViewEvent::DeleteDatabase {
                                                                                        node: node4.clone()
                                                                                    });
                                                                                }))
                                                                        )
                                                                        .separator()
                                                                        .item(
                                                                            PopupMenuItem::new("导入数据")
                                                                                .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                    cx.emit(DbTreeViewEvent::ImportData {
                                                                                        node: node5.clone()
                                                                                    });
                                                                                }))
                                                                        )
                                                                        .item(
                                                                            PopupMenuItem::new("导出数据库")
                                                                                .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                    cx.emit(DbTreeViewEvent::ExportData {
                                                                                        node: node6.clone()
                                                                                    });
                                                                                }))
                                                                        )
                                                                        .separator();
                                                                }
                                                                DbNodeType::Table => {
                                                                    let node1 = node.clone();
                                                                    let node2 = node.clone();
                                                                    let node3 = node.clone();
                                                                    let node4 = node.clone();
                                                                    let node5 = node.clone();
                                                                    let node6 = node.clone();
                                                                    
                                                                    menu = menu
                                                                        .item(
                                                                            PopupMenuItem::new("查看表数据")
                                                                                .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                    cx.emit(DbTreeViewEvent::OpenTableData {
                                                                                        node: node1.clone()
                                                                                    });
                                                                                }))
                                                                        )
                                                                        .item(
                                                                            PopupMenuItem::new("编辑表结构")
                                                                            .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                cx.emit(DbTreeViewEvent::OpenTableStructure {
                                                                                    node: node2.clone()
                                                                                });
                                                                            }))
                                                                        )
                                                                        .separator()
                                                                        .item(
                                                                            PopupMenuItem::new("重命名表")
                                                                                .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                    cx.emit(DbTreeViewEvent::RenameTable {
                                                                                        node: node3.clone()
                                                                                    });
                                                                                }))
                                                                        )
                                                                        .item(
                                                                            PopupMenuItem::new("清空表")
                                                                                .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                    cx.emit(DbTreeViewEvent::TruncateTable {
                                                                                        node: node4.clone()
                                                                                    });
                                                                                }))
                                                                        )
                                                                        .item(
                                                                            PopupMenuItem::new("删除表")
                                                                                .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                    cx.emit(DbTreeViewEvent::DeleteTable {
                                                                                        node: node5.clone()
                                                                                    });
                                                                                }))
                                                                        )
                                                                        .separator()
                                                                        .item(
                                                                            PopupMenuItem::new("导出表")
                                                                                .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                    cx.emit(DbTreeViewEvent::ExportData {
                                                                                        node: node6.clone()
                                                                                    });
                                                                                }))
                                                                        )
                                                                        .separator();
                                                                }
                                                                DbNodeType::View => {
                                                                    let node1 = node.clone();
                                                                    let node2 = node.clone();
                                                                    
                                                                    menu = menu
                                                                        .item(
                                                                            PopupMenuItem::new("查看视图数据")
                                                                                .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                    cx.emit(DbTreeViewEvent::OpenViewData {
                                                                                        node: node1.clone()
                                                                                    });
                                                                                }))
                                                                        )
                                                                        .separator()
                                                                        .item(
                                                                            PopupMenuItem::new("删除视图")
                                                                                .on_click(window.listener_for(&view_clone, move |_this, _, _, cx| {
                                                                                    cx.emit(DbTreeViewEvent::DeleteView {
                                                                                        node: node2.clone()
                                                                                    });
                                                                                }))
                                                                        )
                                                                        .separator();
                                                                }
                                                                _ => {}
                                                            }

                                                            let view_ref2 = view_clone.clone();
                                                            let id_clone = node_id_clone.clone();
                                                            menu.item(
                                                                PopupMenuItem::new("Refresh")
                                                                    .on_click(window.listener_for(&view_ref2, move |this, _, _, cx| {
                                                                        this.refresh_tree(id_clone.clone(), cx);
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
                                           this.handle_item_click(item.clone(), cx)
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

