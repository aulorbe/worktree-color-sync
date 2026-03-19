use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    Notify { terminal_id: String, cwd: String },
    Status,
    Current { terminal_id: String },
    Doctor { terminal_id: Option<String> },
    CycleColor { worktree_path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Ack {
        changed: bool,
        worktree_key: Option<String>,
        color: String,
    },
    Status {
        running: bool,
        terminals: usize,
        active_worktrees: usize,
    },
    Current {
        terminal_id: String,
        worktree_key: Option<String>,
        color: Option<String>,
    },
    Doctor {
        ok: bool,
        checks: Vec<DoctorCheck>,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorCheck {
    pub name: String,
    pub ok: bool,
    pub details: String,
}
