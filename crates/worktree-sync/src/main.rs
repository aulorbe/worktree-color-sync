use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use worktree_color_sync_core::allocator::ColorAllocator;
use worktree_color_sync_core::config::Config;
use worktree_color_sync_core::git::resolve_worktree;
use worktree_color_sync_core::paths::ensure_parent;
use worktree_color_sync_core::protocol::{DoctorCheck, Request, Response};
use worktree_color_sync_core::state::RuntimeState;
use worktree_color_sync_integrations::cursor::apply_cursor_workspace_color;
use worktree_color_sync_integrations::ghostty::{
    apply_background_color_to_tty, doctor_check as ghostty_doctor_check,
    reset_dynamic_colors_for_tty,
};

#[derive(Parser, Debug)]
#[command(name = "worktree-sync")]
#[command(about = "Sync terminal + Cursor colors to active git worktree")]
struct Cli {
    #[arg(long, global = true)]
    config: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Daemon,
    Notify {
        #[arg(long)]
        terminal_id: String,
        #[arg(long)]
        cwd: Option<String>,
    },
    Status,
    Current {
        #[arg(long)]
        terminal_id: String,
    },
    Doctor {
        #[arg(long)]
        terminal_id: Option<String>,
    },
}

#[derive(Debug)]
struct AppState {
    config: Config,
    runtime: RuntimeState,
    allocator: ColorAllocator,
    state_path: PathBuf,
}

impl AppState {
    fn new(config: Config) -> Result<Self> {
        let palette = config.colors.palette.clone().unwrap_or_default();
        let allocator = ColorAllocator::new(palette, config.colors.strict_palette);
        let state_path = config.state_path()?;
        let runtime = RuntimeState::load(&state_path).unwrap_or_default();

        Ok(Self {
            config,
            runtime,
            allocator,
            state_path,
        })
    }

    fn handle_request(&mut self, request: Request) -> Response {
        match request {
            Request::Notify { terminal_id, cwd } => match self.handle_notify(&terminal_id, &cwd) {
                Ok(response) => response,
                Err(err) => Response::Error {
                    message: format!("notify failed: {err:#}"),
                },
            },
            Request::Status => {
                let (terminals, active_worktrees) = self.runtime.counts();
                Response::Status {
                    running: true,
                    terminals,
                    active_worktrees,
                }
            }
            Request::Current { terminal_id } => {
                let current = self.runtime.current_for_terminal(&terminal_id);
                Response::Current {
                    terminal_id,
                    worktree_key: current.as_ref().and_then(|c| c.worktree_key.clone()),
                    color: current.map(|c| c.color),
                }
            }
            Request::Doctor { terminal_id } => {
                let checks = self.doctor_checks(terminal_id.as_deref());
                let ok = checks.iter().all(|c| c.ok);
                Response::Doctor { ok, checks }
            }
        }
    }

    fn handle_notify(&mut self, terminal_id: &str, cwd: &str) -> Result<Response> {
        let git_timeout = Duration::from_millis(self.config.daemon.git_timeout_ms);
        let worktree = match resolve_worktree(&PathBuf::from(cwd), git_timeout) {
            Ok(w) => w,
            Err(err) => {
                warn!(
                    cwd,
                    "git resolution failed, falling back to neutral color: {err:#}"
                );
                None
            }
        };

        let mut assignment_changed = false;
        let (worktree_key, color, worktree_path) = if let Some(worktree) = worktree {
            let key = worktree.key.as_string();
            let existing = self.runtime.assignment_for(&key);
            // Persist color identity per worktree across leave/re-enter cycles.
            // Use assigned colors (persisted map) for uniqueness pressure instead of only active ones.
            let assigned_colors = self.runtime.assigned_colors_excluding_key(Some(&key));
            let color = self
                .allocator
                .allocate(&key, existing.as_deref(), &assigned_colors)?;

            assignment_changed = self.runtime.set_assignment(key.clone(), color.clone());
            (Some(key), color, Some(worktree.key.worktree_path))
        } else {
            (None, self.config.daemon.neutral_color.to_lowercase(), None)
        };

        let context_changed = self.runtime.set_terminal_context(
            terminal_id.to_string(),
            worktree_key.clone(),
            color.clone(),
        );

        self.apply_integrations(terminal_id, worktree_path.as_ref(), &color)?;

        if assignment_changed {
            self.runtime
                .save(&self.state_path)
                .context("failed to persist assignment state")?;
        }

        Ok(Response::Ack {
            changed: assignment_changed || context_changed,
            worktree_key,
            color,
        })
    }

    fn apply_integrations(
        &self,
        terminal_id: &str,
        worktree_path: Option<&PathBuf>,
        color: &str,
    ) -> Result<()> {
        if self.config.integrations.ghostty.enabled {
            let ghostty_result = if worktree_path.is_some() {
                apply_background_color_to_tty(terminal_id, color)
            } else {
                reset_dynamic_colors_for_tty(terminal_id)
            };

            if let Err(err) = ghostty_result {
                warn!(
                    terminal_id,
                    "ghostty tab-specific update failed, using global fallback file: {err:#}"
                );
                self.write_ghostty_global_fallback(color)?;
            }
        }

        if self.config.integrations.cursor.enabled {
            if let Some(path) = worktree_path {
                apply_cursor_workspace_color(path, color)
                    .with_context(|| format!("failed Cursor update for {}", path.display()))?;
            }
        }

        Ok(())
    }

    fn write_ghostty_global_fallback(&self, color: &str) -> Result<()> {
        let fallback = self.config.ghostty_global_fallback_path()?;
        ensure_parent(&fallback)?;
        std::fs::write(
            &fallback,
            format!("# managed by worktree-sync\nbackground = {color}\n"),
        )
        .with_context(|| format!("failed to write ghostty fallback {}", fallback.display()))?;
        Ok(())
    }

    fn doctor_checks(&self, terminal_id: Option<&str>) -> Vec<DoctorCheck> {
        let mut checks = Vec::new();

        match self.config.socket_path() {
            Ok(path) => {
                let parent_ok = path.parent().map(|p| p.exists()).unwrap_or(false);
                checks.push(DoctorCheck {
                    name: "socket_parent".to_string(),
                    ok: parent_ok,
                    details: format!("socket path: {}", path.display()),
                });
            }
            Err(err) => checks.push(DoctorCheck {
                name: "socket_parent".to_string(),
                ok: false,
                details: format!("invalid socket path: {err:#}"),
            }),
        }

        match self.config.state_path() {
            Ok(path) => {
                checks.push(DoctorCheck {
                    name: "state_path".to_string(),
                    ok: true,
                    details: format!("state path: {}", path.display()),
                });
            }
            Err(err) => checks.push(DoctorCheck {
                name: "state_path".to_string(),
                ok: false,
                details: format!("invalid state path: {err:#}"),
            }),
        }

        let (tty_ok, tty_details) = ghostty_doctor_check(terminal_id);
        checks.push(DoctorCheck {
            name: "ghostty_tty".to_string(),
            ok: tty_ok,
            details: tty_details,
        });

        checks.push(DoctorCheck {
            name: "cursor_settings".to_string(),
            ok: true,
            details: "Cursor integration writes only managed keys in .vscode/settings.json"
                .to_string(),
        });

        checks
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "worktree_sync=info".into()),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Daemon => run_daemon(cli.config.as_deref()).await,
        Commands::Notify { terminal_id, cwd } => {
            let cwd = cwd.unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .display()
                    .to_string()
            });

            let request = Request::Notify { terminal_id, cwd };
            let response = send_request(cli.config.as_deref(), request).await?;
            print_response(response);
            Ok(())
        }
        Commands::Status => {
            let response = send_request(cli.config.as_deref(), Request::Status).await?;
            print_response(response);
            Ok(())
        }
        Commands::Current { terminal_id } => {
            let response =
                send_request(cli.config.as_deref(), Request::Current { terminal_id }).await?;
            print_response(response);
            Ok(())
        }
        Commands::Doctor { terminal_id } => {
            let request = Request::Doctor { terminal_id };
            match send_request(cli.config.as_deref(), request).await {
                Ok(response) => {
                    print_response(response);
                    Ok(())
                }
                Err(err) => {
                    warn!("daemon unreachable, running local doctor checks: {err:#}");
                    let config = Config::load(cli.config.as_deref())?;
                    let state = AppState::new(config)?;
                    let response = Response::Doctor {
                        ok: false,
                        checks: state.doctor_checks(None),
                    };
                    print_response(response);
                    Ok(())
                }
            }
        }
    }
}

async fn run_daemon(config_path: Option<&str>) -> Result<()> {
    let config = Config::load(config_path)?;
    let socket_path = config.socket_path()?;
    ensure_parent(&socket_path)?;

    if socket_path.exists() {
        std::fs::remove_file(&socket_path)
            .with_context(|| format!("failed to remove stale socket {}", socket_path.display()))?;
    }

    let listener = UnixListener::bind(&socket_path)
        .with_context(|| format!("failed to bind socket {}", socket_path.display()))?;

    let state = Arc::new(Mutex::new(AppState::new(config)?));
    info!(socket = %socket_path.display(), "daemon started");

    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            _ = &mut shutdown => {
                info!("received shutdown signal");
                break;
            }
            accepted = listener.accept() => {
                let (stream, _) = accepted?;
                let state = Arc::clone(&state);
                tokio::spawn(async move {
                    if let Err(err) = handle_stream(stream, state).await {
                        error!("connection error: {err:#}");
                    }
                });
            }
        }
    }

    let _ = std::fs::remove_file(&socket_path);
    Ok(())
}

async fn handle_stream(mut stream: UnixStream, state: Arc<Mutex<AppState>>) -> Result<()> {
    let mut request_bytes = Vec::new();
    stream
        .read_to_end(&mut request_bytes)
        .await
        .context("failed reading request")?;

    if request_bytes.is_empty() {
        return Ok(());
    }

    let request: Request =
        serde_json::from_slice(&request_bytes).context("failed to decode json request")?;

    let response = {
        let mut locked = state.lock().await;
        locked.handle_request(request)
    };

    let response_bytes = serde_json::to_vec(&response)?;
    stream.write_all(&response_bytes).await?;
    stream.shutdown().await?;

    Ok(())
}

async fn send_request(config_path: Option<&str>, request: Request) -> Result<Response> {
    let config = Config::load(config_path)?;
    let socket = config.socket_path()?;

    let mut stream = UnixStream::connect(&socket)
        .await
        .with_context(|| format!("failed to connect to {}", socket.display()))?;

    let request_bytes = serde_json::to_vec(&request)?;
    stream.write_all(&request_bytes).await?;
    stream.shutdown().await?;

    let mut response_bytes = Vec::new();
    stream.read_to_end(&mut response_bytes).await?;
    let response: Response =
        serde_json::from_slice(&response_bytes).context("failed to decode daemon response")?;
    Ok(response)
}

fn print_response(response: Response) {
    match response {
        Response::Ack {
            changed,
            worktree_key,
            color,
        } => {
            println!(
                "ack changed={changed} color={color} worktree={}",
                worktree_key.unwrap_or_else(|| "<none>".to_string())
            );
        }
        Response::Status {
            running,
            terminals,
            active_worktrees,
        } => {
            println!("running={running} terminals={terminals} active_worktrees={active_worktrees}");
        }
        Response::Current {
            terminal_id,
            worktree_key,
            color,
        } => {
            println!(
                "terminal_id={} worktree={} color={}",
                terminal_id,
                worktree_key.unwrap_or_else(|| "<none>".to_string()),
                color.unwrap_or_else(|| "<none>".to_string())
            );
        }
        Response::Doctor { ok, checks } => {
            println!("doctor ok={ok}");
            for check in checks {
                let status = if check.ok { "ok" } else { "fail" };
                println!("- {}: {} ({})", check.name, status, check.details);
            }
        }
        Response::Error { message } => {
            println!("error: {message}");
        }
    }
}
