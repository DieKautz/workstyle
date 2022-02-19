use std::collections::BTreeMap;
use swayipc_async::{Connection, EventStream, EventType, Node, NodeType};

trait NodeExt {
    fn is_workspace(&self) -> bool;
    fn is_window(&self) -> bool;
    fn name(&self) -> Option<String>;
    fn app_id(&self) -> Option<String>;
    fn window_properties_class(&self) -> Option<String>;
    fn windows_in_node(&self) -> Vec<Window>;
    fn workspaces_in_node(&self) -> Result<BTreeMap<String, Vec<Window>>, &'static str>;
}

impl NodeExt for Node {
    fn is_workspace(&self) -> bool {
        self.node_type == NodeType::Workspace
    }
    fn is_window(&self) -> bool {
        matches!(self.node_type, NodeType::Con | NodeType::FloatingCon)
    }
    fn name(&self) -> Option<String> {
        self.name.clone()
    }
    fn app_id(&self) -> Option<String> {
        self.app_id.clone()
    }
    fn window_properties_class(&self) -> Option<String> {
        self.window_properties
            .as_ref()
            .and_then(|prop| prop.class.clone())
    }
    /// Recursively find all windows names in this node
    fn windows_in_node(&self) -> Vec<Window> {
        let mut res = Vec::new();
        for node in self.nodes.iter().chain(self.floating_nodes.iter()) {
            res.extend(node.windows_in_node());
            if node.is_window() {
                if let Some(window) = Window::from_node(node) {
                    res.push(window);
                }
            }
        }
        res
    }
    /// Recursively find all workspaces in this node and the list of open windows for each of these
    /// workspaces
    fn workspaces_in_node(&self) -> Result<BTreeMap<String, Vec<Window>>, &'static str> {
        let mut res = BTreeMap::new();
        for node in &self.nodes {
            if node.is_workspace() {
                res.insert(
                    node.name().ok_or("Expected some node name")?,
                    node.windows_in_node(),
                );
            } else {
                let workspaces = node.workspaces_in_node()?;
                for (k, v) in workspaces {
                    res.insert(k, v);
                }
            }
        }
        Ok(res)
    }
}

#[derive(Debug)]
pub struct Window {
    name: Option<String>,
    app_id: Option<String>,
    window_properties_class: Option<String>,
}

impl Window {
    fn from_node(node: &Node) -> Option<Self> {
        if node.is_window() {
            let name = node.name();
            let app_id = node.app_id();
            let window_properties_class = node.window_properties_class();
            if name.is_some() || app_id.is_some() || window_properties_class.is_some() {
                Some(Self {
                    name,
                    app_id,
                    window_properties_class,
                })
            } else {
                None
            }
        } else {
            None
        }
    }
    pub fn matches(&self, pattern: &str) -> bool {
        self.name
            .as_ref()
            .map(|s| s.to_lowercase().contains(pattern))
            .unwrap_or(false)
            || self
                .app_id
                .as_ref()
                .map(|s| s.to_lowercase().contains(pattern))
                .unwrap_or(false)
            || self
                .window_properties_class
                .as_ref()
                .map(|s| s.to_lowercase().contains(pattern))
                .unwrap_or(false)
    }
}

pub struct WindowManager {
    connection: Connection,
}

impl WindowManager {
    pub async fn connect() -> Result<(Self, EventStream), &'static str> {
        let stream = Connection::new()
            .await
            .map_err(|_| "Couldn't connect to sway")?
            .subscribe(&[EventType::Window])
            .await
            .map_err(|_| "Couldn't subscribe to events of type Window with sway")?;
        Ok((
            Self {
                connection: Connection::new()
                    .await
                    .map_err(|_| "Couldn't connect to Sway/I3")?,
            },
            stream,
        ))
    }
    pub async fn get_windows_in_each_workspace(
        &mut self,
    ) -> Result<BTreeMap<String, Vec<Window>>, &'static str> {
        self.connection
            .get_tree()
            .await
            .map_err(|_| "Failed to get_tree with sway")?
            .workspaces_in_node()
    }

    pub async fn rename_workspace(&mut self, old: &str, new: &str) -> Result<(), &'static str> {
        self.connection
            .run_command(&format!("rename workspace \"{old}\" to \"{new}\"",))
            .await
            .map_err(|_| "Failed to run_command with sway")?;
        Ok(())
    }
}
