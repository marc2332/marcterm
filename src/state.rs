use std::path::PathBuf;

use freya::{
    prelude::{
        AccessibilityId,
        Focus,
        UseId,
    },
    terminal::*,
};
use freya::radio::RadioChannel;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(pub usize);

impl TabId {
    pub fn new() -> Self {
        Self(UseId::<TabId>::get_in_hook())
    }
}

#[derive(Clone, PartialEq)]
pub enum PanelNode {
    Leaf(AccessibilityId, TerminalHandle),
    Horizontal(Box<PanelNode>, Box<PanelNode>),
    Vertical(Box<PanelNode>, Box<PanelNode>),
}

fn make_handle(shell: &str, cwd: Option<PathBuf>) -> TerminalHandle {
    let mut cmd = CommandBuilder::new(shell);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("LANG", "en_GB.UTF-8");
    if let Some(dir) = cwd {
        cmd.cwd(dir);
    }
    TerminalHandle::new(TerminalId::new(), cmd, None).expect("failed to spawn PTY")
}

impl PanelNode {
    pub fn new_leaf(shell: &str, cwd: Option<PathBuf>) -> (AccessibilityId, Self) {
        let id = Focus::new_id();
        (id, PanelNode::Leaf(id, make_handle(shell, cwd)))
    }

    /// Returns the `PanelId` if this node is a `Leaf`, otherwise `None`.
    /// Find the neighbour of `target` in the given direction.
    /// Walks the tree looking for the closest split that can resolve the move.
    pub fn find_neighbour(
        &self,
        target: AccessibilityId,
        dir: NavDirection,
    ) -> Option<AccessibilityId> {
        match self {
            PanelNode::Leaf(_, _) => None,
            PanelNode::Horizontal(a, b) => {
                let in_a = a.contains(target);
                let in_b = b.contains(target);
                match dir {
                    NavDirection::Right if in_a => Some(b.leaves()[0]),
                    NavDirection::Left if in_b => Some(a.leaves()[0]),
                    _ if in_a => a.find_neighbour(target, dir),
                    _ if in_b => b.find_neighbour(target, dir),
                    _ => None,
                }
            }
            PanelNode::Vertical(a, b) => {
                let in_a = a.contains(target);
                let in_b = b.contains(target);
                match dir {
                    NavDirection::Down if in_a => Some(b.leaves()[0]),
                    NavDirection::Up if in_b => Some(a.leaves()[0]),
                    _ if in_a => a.find_neighbour(target, dir),
                    _ if in_b => b.find_neighbour(target, dir),
                    _ => None,
                }
            }
        }
    }

    pub fn contains(&self, id: AccessibilityId) -> bool {
        match self {
            PanelNode::Leaf(pid, _) => *pid == id,
            PanelNode::Horizontal(a, b) | PanelNode::Vertical(a, b) => {
                a.contains(id) || b.contains(id)
            }
        }
    }

    pub fn leaves(&self) -> Vec<AccessibilityId> {
        match self {
            PanelNode::Leaf(id, _) => vec![*id],
            PanelNode::Horizontal(a, b) | PanelNode::Vertical(a, b) => {
                let mut v = a.leaves();
                v.extend(b.leaves());
                v
            }
        }
    }

    pub fn handle(&self, id: AccessibilityId) -> Option<&TerminalHandle> {
        match self {
            PanelNode::Leaf(pid, h) if *pid == id => Some(h),
            PanelNode::Leaf(_, _) => None,
            PanelNode::Horizontal(a, b) | PanelNode::Vertical(a, b) => {
                a.handle(id).or_else(|| b.handle(id))
            }
        }
    }

    pub fn replace_leaf(self, target: AccessibilityId, replacement: PanelNode) -> PanelNode {
        match self {
            PanelNode::Leaf(id, _) if id == target => replacement,
            PanelNode::Leaf(_, _) => self,
            PanelNode::Horizontal(a, b) => PanelNode::Horizontal(
                Box::new(a.replace_leaf(target, replacement.clone())),
                Box::new(b.replace_leaf(target, replacement)),
            ),
            PanelNode::Vertical(a, b) => PanelNode::Vertical(
                Box::new(a.replace_leaf(target, replacement.clone())),
                Box::new(b.replace_leaf(target, replacement)),
            ),
        }
    }

    pub fn remove_leaf(self, target: AccessibilityId) -> Option<PanelNode> {
        match self {
            PanelNode::Leaf(id, _) if id == target => None,
            PanelNode::Leaf(_, _) => Some(self),
            PanelNode::Horizontal(a, b) => {
                if a.contains(target) {
                    if matches!(*a, PanelNode::Leaf(id, _) if id == target) {
                        return Some(*b);
                    }
                    let new_a = a.remove_leaf(target)?;
                    Some(PanelNode::Horizontal(Box::new(new_a), b))
                } else {
                    if matches!(*b, PanelNode::Leaf(id, _) if id == target) {
                        return Some(*a);
                    }
                    let new_b = b.remove_leaf(target)?;
                    Some(PanelNode::Horizontal(a, Box::new(new_b)))
                }
            }
            PanelNode::Vertical(a, b) => {
                if a.contains(target) {
                    if matches!(*a, PanelNode::Leaf(id, _) if id == target) {
                        return Some(*b);
                    }
                    let new_a = a.remove_leaf(target)?;
                    Some(PanelNode::Vertical(Box::new(new_a), b))
                } else {
                    if matches!(*b, PanelNode::Leaf(id, _) if id == target) {
                        return Some(*a);
                    }
                    let new_b = b.remove_leaf(target)?;
                    Some(PanelNode::Vertical(a, Box::new(new_b)))
                }
            }
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct Tab {
    pub id: TabId,
    pub title: String,
    pub panels: PanelNode,
    pub active_panel: AccessibilityId,
}

impl Tab {
    pub fn new(index: usize, shell: &str, cwd: Option<PathBuf>) -> Self {
        let (active_panel, root) = PanelNode::new_leaf(shell, cwd);
        Self {
            id: TabId::new(),
            title: format!("Terminal {}", index + 1),
            panels: root,
            active_panel,
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct AppState {
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    pub font_size: f32,
    pub shell: String,
}

impl AppState {
    pub fn new(font_size: f32, shell: String) -> Self {
        let tab = Tab::new(0, &shell, None);
        Self {
            tabs: vec![tab],
            active_tab: 0,
            font_size,
            shell,
        }
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active_tab)
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active_tab)
    }

    pub fn new_tab(&mut self) {
        let index = self.tabs.len();
        let cwd = self
            .active_tab()
            .and_then(|tab| tab.panels.handle(tab.active_panel))
            .and_then(|h| h.cwd());
        let tab = Tab::new(index, &self.shell.clone(), cwd);
        self.tabs.push(tab);
        self.active_tab = self.tabs.len() - 1;
    }

    pub fn close_active_tab(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }
        self.tabs.remove(self.active_tab);
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        }
    }

    pub fn next_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        self.active_tab = (self.active_tab + 1) % self.tabs.len();
    }

    pub fn prev_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        self.active_tab = self
            .active_tab
            .checked_sub(1)
            .unwrap_or(self.tabs.len() - 1);
    }

    pub fn split_horizontal(&mut self) {
        let shell = self.shell.clone();
        let cwd = self
            .active_tab()
            .and_then(|tab| tab.panels.handle(tab.active_panel))
            .and_then(|h| h.cwd());
        let (new_id, new_leaf) = PanelNode::new_leaf(&shell, cwd);
        if let Some(tab) = self.active_tab_mut() {
            let split = PanelNode::Horizontal(
                Box::new(PanelNode::Leaf(
                    tab.active_panel,
                    tab.panels.handle(tab.active_panel).cloned().unwrap(),
                )),
                Box::new(new_leaf),
            );
            tab.panels = tab.panels.clone().replace_leaf(tab.active_panel, split);
            tab.active_panel = new_id;
        }
    }

    pub fn split_vertical(&mut self) {
        let shell = self.shell.clone();
        let cwd = self
            .active_tab()
            .and_then(|tab| tab.panels.handle(tab.active_panel))
            .and_then(|h| h.cwd());
        let (new_id, new_leaf) = PanelNode::new_leaf(&shell, cwd);
        if let Some(tab) = self.active_tab_mut() {
            let split = PanelNode::Vertical(
                Box::new(PanelNode::Leaf(
                    tab.active_panel,
                    tab.panels.handle(tab.active_panel).cloned().unwrap(),
                )),
                Box::new(new_leaf),
            );
            tab.panels = tab.panels.clone().replace_leaf(tab.active_panel, split);
            tab.active_panel = new_id;
        }
    }

    pub fn close_active_panel(&mut self) {
        if let Some(tab) = self.active_tab_mut() {
            if let Some(new_root) = tab.panels.clone().remove_leaf(tab.active_panel) {
                let leaves = new_root.leaves();
                tab.panels = new_root;
                if let Some(panel) = leaves.into_iter().last() {
                    tab.active_panel = panel;
                    Focus::new_for_id(panel).request_focus();
                }
            }
        }
    }

    pub fn navigate(&mut self, dir: NavDirection) {
        if let Some(tab) = self.active_tab_mut() {
            if let Some(neighbour) = tab.panels.find_neighbour(tab.active_panel, dir) {
                tab.active_panel = neighbour;
            }
        }
    }

    pub fn increase_font_size(&mut self) {
        self.font_size = (self.font_size + 1.0).min(48.0);
    }

    pub fn decrease_font_size(&mut self) {
        self.font_size = (self.font_size - 1.0).max(6.0);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavDirection {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppChannel {
    Tabs,
}

impl RadioChannel<AppState> for AppChannel {}
