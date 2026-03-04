# Worktree Color Sync (CODEX MVP)

Worktree Color Sync is a macOS-first Rust daemon that syncs visual context to your active git worktree.

This MVP includes:
- Ghostty terminal color sync (tab-specific, based on TTY)
- Cursor workspace titlebar tint sync (`.vscode/settings.json`)
- Deterministic, collision-aware color assignment for active worktrees
- Local daemon + CLI (`daemon`, `notify`, `status`, `current`, `doctor`)

## Why
When juggling multiple worktrees, it is easy to run commands in the wrong one. This tool makes context visible through color.

## Install / Build

```bash
cargo build --release -p worktree-sync
```

Binary path:
- `target/release/worktree-sync`

## Quick Start

1. Start daemon:
```bash
worktree-sync daemon
```

2. Add shell hook (`shell/hook.zsh`) to your `~/.zshrc`.

3. In Ghostty, each tab/session sends updates using a zsh hook with:
- `terminal_id = $(tty)` (tab-specific)
- notification when entering a git root (worktree/repo root)
- notification when leaving git-root context so terminal resets to defaults
- transition detection so repeated prompts do not spam updates

4. Check health:
```bash
worktree-sync status
worktree-sync doctor --terminal-id "$(tty)"
```

## CLI

```bash
worktree-sync daemon
worktree-sync notify --terminal-id /dev/ttys012 --cwd /path/to/repo-or-worktree
worktree-sync status
worktree-sync current --terminal-id /dev/ttys012
worktree-sync doctor --terminal-id /dev/ttys012
```

## Cursor Workflow (New Window)

The zsh hook defines a `cursor` shell function for worktree ergonomics:
- `cursor` (no args): opens the current git root in a **new Cursor window**
- if currently inside a git checkout, it normalizes to the worktree/project root
- before launching Cursor, it sends `worktree-sync notify` so terminal and workspace settings stay aligned
- if args are provided, it preserves them and adds `--new-window` unless you already passed `-n/--new-window` or `-r/--reuse-window`

Examples:
```bash
cursor                    # opens current git root in new window
cursor .                  # opens cwd in new window
cursor -r .               # explicitly reuse existing window
cursor --new-window .     # explicit new window
```

## Configuration

Default config path is optional. You can run with defaults or pass:

```bash
worktree-sync --config ~/.config/worktree-sync/config.toml daemon
```

Example config:
- `config/worktree-sync.toml.example`

## How It Works

- `notify` event resolves worktree via:
  - `git rev-parse --show-toplevel`
  - `git rev-parse --git-common-dir`
  - `git worktree list --porcelain`
- Worktree identity key is `repo_root + worktree_path`.
- Color allocator is deterministic, preserves persisted assignments to disk, and reuses the same worktree color across leave/re-enter cycles.
- Ghostty update behavior is context-aware per tab TTY:
  - in worktree context: writes OSC 11 (background) + OSC 10 (foreground)
  - outside worktree context: writes OSC reset sequences (110/111/112) to restore terminal defaults
- Cursor update merges managed titlebar keys into `.vscode/settings.json`, and sets integrated terminal defaults:
  - `terminal.integrated.cwd = ${workspaceFolder}`
  - `terminal.integrated.splitCwd = initial`

## Known Constraints and Disclaimers

- `MVP scope`: Terminal + Cursor only. Browser integration is intentionally excluded.
- `Platform`: Designed for macOS and Ghostty + Cursor workflows.
- `Ghostty behavior`: Tab-specific updates rely on OSC support (OSC 11 + OSC 10) and TTY accessibility; behavior can vary by terminal/version and theme settings.
- `Fallback`: If tab-specific TTY update fails, daemon writes a global Ghostty fallback config snippet at:
  - `~/.config/ghostty/worktree-sync-global.conf`
  - You must manually include this file in your Ghostty config if you want fallback to apply.
- `Shell hook requirement`: To get tab-specific behavior, use `$(tty)` as `terminal_id` (not window/resource path).
- `Root-transition trigger`: Current hook updates on entering roots and leaving root context.
- `Cursor wrapper`: The hook defines a shell function named `cursor`; if you need the raw binary, use `command cursor ...`.
- `Cursor settings`: This tool writes to workspace-local `.vscode/settings.json` and may create the file.
- `Terminal defaults`: Workspace terminal cwd/split behavior is set to project-root oriented defaults.
- `Certainty`: Active collisions are avoided in-memory; extremely large active sets may require fallback generated colors.
- `Security`: The daemon trusts local socket clients on your user account. Do not expose the socket path to other users.
- `No warranty`: This is open source software provided "as is".

## Open Source

License: MIT (`LICENSE`)

## Development

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
```

Current tested commands during implementation:
- `cargo check --workspace`
- `cargo test --workspace`

