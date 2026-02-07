use bollard::secret::{ContainerSummary, Network};
use ratatui::widgets::TableState;
use tokio::sync::mpsc;

// ── Enums ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Containers,
    Networks,
}

impl Tab {
    pub fn next(self) -> Self {
        match self {
            Tab::Containers => Tab::Networks,
            Tab::Networks => Tab::Containers,
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Tab::Containers => "Containers",
            Tab::Networks => "Networks",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Detail,
    Logs,
}

#[derive(Debug)]
pub enum DockerAction {
    Start(String),
    Stop(String),
    Kill(String),
    Remove(String),
    StreamLogs { container_id: String },
    StopLogStream,
}

#[derive(Debug)]
pub enum DockerEvent {
    ContainersUpdated(Vec<ContainerSummary>),
    NetworksUpdated(Vec<Network>),
    LogLine(String),
    LogStreamEnded,
    ActionResult { #[allow(dead_code)] success: bool, message: String },
}

// ── App State ──────────────────────────────────────────────────────────────

pub struct App {
    pub tab: Tab,
    pub mode: Mode,
    pub should_quit: bool,

    // Container data
    pub containers: Vec<ContainerSummary>,
    pub container_table_state: TableState,

    // Network data
    pub networks: Vec<Network>,
    pub network_table_state: TableState,

    // Logs
    pub log_lines: Vec<String>,
    pub log_scroll: usize,
    pub log_streaming: bool,

    // Status bar message
    pub status_message: Option<String>,

    // Channels
    pub event_rx: mpsc::Receiver<DockerEvent>,
    pub action_tx: mpsc::Sender<DockerAction>,
}

impl App {
    pub fn new(event_rx: mpsc::Receiver<DockerEvent>, action_tx: mpsc::Sender<DockerAction>) -> Self {
        Self {
            tab: Tab::Containers,
            mode: Mode::Normal,
            should_quit: false,
            containers: Vec::new(),
            container_table_state: TableState::default(),
            networks: Vec::new(),
            network_table_state: TableState::default(),
            log_lines: Vec::new(),
            log_scroll: 0,
            log_streaming: false,
            status_message: None,
            event_rx,
            action_tx,
        }
    }

    // ── Selection helpers ──────────────────────────────────────────────

    pub fn selected_container(&self) -> Option<&ContainerSummary> {
        self.container_table_state
            .selected()
            .and_then(|i| self.containers.get(i))
    }

    pub fn selected_container_id(&self) -> Option<String> {
        self.selected_container()
            .and_then(|c| c.id.clone())
    }

    pub fn selected_container_state(&self) -> Option<&str> {
        self.selected_container()
            .and_then(|c| c.state.as_deref())
    }

    // ── Navigation ─────────────────────────────────────────────────────

    pub fn next_item(&mut self) {
        let len = match self.tab {
            Tab::Containers => self.containers.len(),
            Tab::Networks => self.networks.len(),
        };
        if len == 0 {
            return;
        }
        let table = match self.tab {
            Tab::Containers => &mut self.container_table_state,
            Tab::Networks => &mut self.network_table_state,
        };
        let i = table.selected().map_or(0, |i| (i + 1) % len);
        table.select(Some(i));
    }

    pub fn prev_item(&mut self) {
        let len = match self.tab {
            Tab::Containers => self.containers.len(),
            Tab::Networks => self.networks.len(),
        };
        if len == 0 {
            return;
        }
        let table = match self.tab {
            Tab::Containers => &mut self.container_table_state,
            Tab::Networks => &mut self.network_table_state,
        };
        let i = table.selected().map_or(0, |i| {
            if i == 0 { len - 1 } else { i - 1 }
        });
        table.select(Some(i));
    }

    pub fn switch_tab(&mut self) {
        self.tab = self.tab.next();
    }

    // ── Data updates (preserves selection by ID) ───────────────────────

    pub fn update_containers(&mut self, mut new: Vec<ContainerSummary>) {
        new.sort_by(|a, b| {
            let na = container_name(a);
            let nb = container_name(b);
            na.cmp(&nb)
        });

        // Preserve selection by container ID
        let prev_id = self.selected_container_id();
        self.containers = new;

        if let Some(pid) = prev_id {
            if let Some(pos) = self.containers.iter().position(|c| c.id.as_deref() == Some(&pid)) {
                self.container_table_state.select(Some(pos));
            } else if !self.containers.is_empty() {
                let sel = self.container_table_state.selected().unwrap_or(0);
                self.container_table_state.select(Some(sel.min(self.containers.len() - 1)));
            } else {
                self.container_table_state.select(None);
            }
        } else if !self.containers.is_empty() && self.container_table_state.selected().is_none() {
            self.container_table_state.select(Some(0));
        }
    }

    pub fn update_networks(&mut self, mut new: Vec<Network>) {
        new.sort_by(|a, b| {
            let na = a.name.as_deref().unwrap_or("");
            let nb = b.name.as_deref().unwrap_or("");
            na.cmp(nb)
        });

        let prev_id = self.network_table_state.selected().and_then(|i| {
            self.networks.get(i).and_then(|n| n.id.clone())
        });
        self.networks = new;

        if let Some(pid) = prev_id {
            if let Some(pos) = self.networks.iter().position(|n| n.id.as_deref() == Some(&pid)) {
                self.network_table_state.select(Some(pos));
            } else if !self.networks.is_empty() {
                let sel = self.network_table_state.selected().unwrap_or(0);
                self.network_table_state.select(Some(sel.min(self.networks.len() - 1)));
            } else {
                self.network_table_state.select(None);
            }
        } else if !self.networks.is_empty() && self.network_table_state.selected().is_none() {
            self.network_table_state.select(Some(0));
        }
    }

    // ── Log scrolling ──────────────────────────────────────────────────

    pub fn log_page_down(&mut self, page_height: usize) {
        let max = self.log_lines.len().saturating_sub(page_height);
        self.log_scroll = (self.log_scroll + page_height).min(max);
    }

    pub fn log_page_up(&mut self, page_height: usize) {
        self.log_scroll = self.log_scroll.saturating_sub(page_height);
    }

    pub fn log_top(&mut self) {
        self.log_scroll = 0;
    }

    pub fn log_bottom(&mut self, page_height: usize) {
        self.log_scroll = self.log_lines.len().saturating_sub(page_height);
    }

    pub fn append_log_line(&mut self, line: String) {
        self.log_lines.push(line);
        // Cap at 10k lines
        if self.log_lines.len() > 10_000 {
            self.log_lines.drain(..self.log_lines.len() - 10_000);
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

pub fn container_name(c: &ContainerSummary) -> String {
    c.names
        .as_ref()
        .and_then(|n| n.first())
        .map(|n| n.trim_start_matches('/').to_string())
        .unwrap_or_else(|| "unnamed".into())
}

pub fn container_ports(c: &ContainerSummary) -> String {
    c.ports
        .as_ref()
        .map(|ports| {
            ports
                .iter()
                .filter_map(|p| {
                    let private = p.private_port;
                    if let (Some(public), Some(_ip)) = (p.public_port, p.ip.as_ref()) {
                        Some(format!("{}:{}", public, private))
                    } else {
                        Some(format!("{}", private))
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}
