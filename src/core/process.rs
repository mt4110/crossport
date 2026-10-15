use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub user: String,
    pub uid: Option<u32>,
    pub cmd: String,
    pub cwd: PathBuf,
    pub project_root: Option<PathBuf>,
    pub container_name: Option<String>,
    pub kind: ProcessKind,
    pub port: u16,
    pub local_addr: Option<String>,
    pub args: Vec<String>,
    pub executable: Option<PathBuf>,
    pub parent_pid: Option<u32>,
    pub parent_name: Option<String>,
    pub started_at: u64,
    pub uptime_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessKind {
    System,
    Brew,
    Dev,
    Docker,
    Kubernetes,
    Other,
}

impl ProcessKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProcessKind::System => "system",
            ProcessKind::Brew => "brew",
            ProcessKind::Dev => "dev",
            ProcessKind::Docker => "docker",
            ProcessKind::Kubernetes => "k8s",
            ProcessKind::Other => "other",
        }
    }
}
