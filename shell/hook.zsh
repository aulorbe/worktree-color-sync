# Worktree Sync zsh hook (Ghostty, tab-specific)
# Requires daemon running: worktree-sync daemon
# Behavior:
# - Notifies when entering a git root (worktree/repo root).
# - Notifies when leaving root context (into subdir or outside git) so terminal resets.
# - Provides a `cursor` wrapper that opens the git root in a NEW window.

# Runtime guard state
typeset -g __WORKTREE_SYNC_AT_ROOT=0
typeset -g __WORKTREE_SYNC_LAST_ROOT=""

function __worktree_sync_git_root() {
  git -C "$1" rev-parse --show-toplevel 2>/dev/null
}

function __worktree_sync_notify_for_cwd() {
  local tty_path="$1"
  local cwd="$2"
  worktree-sync notify \
    --terminal-id "$tty_path" \
    --cwd "$cwd" >/dev/null 2>&1
}

function __worktree_sync_maybe_notify() {
  [[ -z "$GHOSTTY_RESOURCES_DIR" ]] && return

  local tty_path root
  tty_path="$(tty 2>/dev/null)"
  [[ "$tty_path" == /dev/* ]] || return

  root="$(__worktree_sync_git_root "$PWD")" || {
    # Left git context entirely: reset this tab back to terminal defaults once.
    if [[ "$__WORKTREE_SYNC_AT_ROOT" -eq 1 ]]; then
      __worktree_sync_notify_for_cwd "$tty_path" "$PWD"
    fi
    __WORKTREE_SYNC_AT_ROOT=0
    __WORKTREE_SYNC_LAST_ROOT=""
    return
  }

  # Inside a git worktree: apply color if this is a new worktree or first time here
  if [[ "$root" != "$__WORKTREE_SYNC_LAST_ROOT" ]]; then
    __worktree_sync_notify_for_cwd "$tty_path" "$root"
    __WORKTREE_SYNC_LAST_ROOT="$root"
  fi

  __WORKTREE_SYNC_AT_ROOT=1
}

# Cursor wrapper:
# - no args: open current git root in a NEW window
# - with args: preserve args, but default to NEW window unless user explicitly passed reuse/new flags
function cursor() {
  local root project_path tty_path
  root="$(__worktree_sync_git_root "$PWD")"
  if [[ -n "$root" ]]; then
    project_path="$root"
  else
    project_path="$PWD"
  fi

  tty_path="$(tty 2>/dev/null)"
  if [[ -n "$GHOSTTY_RESOURCES_DIR" && "$tty_path" == /dev/* && -n "$root" ]]; then
    __worktree_sync_notify_for_cwd "$tty_path" "$root" || true
    __WORKTREE_SYNC_AT_ROOT=1
    __WORKTREE_SYNC_LAST_ROOT="$root"
  fi

  if [[ "$#" -eq 0 ]]; then
    command cursor --new-window "$project_path"
    return
  fi

  local has_window_flag=0
  local arg
  for arg in "$@"; do
    if [[ "$arg" == "-n" || "$arg" == "--new-window" || "$arg" == "-r" || "$arg" == "--reuse-window" ]]; then
      has_window_flag=1
      break
    fi
  done

  if [[ "$has_window_flag" -eq 1 ]]; then
    command cursor "$@"
  else
    command cursor --new-window "$@"
  fi
}

# Register once per shell session.
if [[ " ${chpwd_functions[*]} " != *" __worktree_sync_maybe_notify "* ]]; then
  chpwd_functions+=(__worktree_sync_maybe_notify)
fi
if [[ " ${precmd_functions[*]} " != *" __worktree_sync_maybe_notify "* ]]; then
  precmd_functions+=(__worktree_sync_maybe_notify)
fi
