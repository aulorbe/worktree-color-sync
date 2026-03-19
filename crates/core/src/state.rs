use crate::paths::ensure_parent;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct RuntimeState {
    assignments: HashMap<String, String>,
    terminal_contexts: HashMap<String, TerminalContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedState {
    assignments: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct TerminalContext {
    pub worktree_key: Option<String>,
    pub color: String,
}

impl RuntimeState {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read state file {}", path.display()))?;
        let persisted: PersistedState =
            serde_json::from_str(&raw).context("failed to parse state json")?;

        Ok(Self {
            assignments: persisted.assignments,
            terminal_contexts: HashMap::new(),
        })
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        ensure_parent(path)?;
        let persisted = PersistedState {
            assignments: self.assignments.clone(),
        };
        let raw = serde_json::to_string_pretty(&persisted)?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, raw)
            .with_context(|| format!("failed to write temp state file {}", tmp.display()))?;
        std::fs::rename(&tmp, path)
            .with_context(|| format!("failed to move state file into place {}", path.display()))?;
        Ok(())
    }

    pub fn assignment_for(&self, key: &str) -> Option<String> {
        self.assignments.get(key).cloned()
    }

    pub fn set_assignment(&mut self, key: String, color: String) -> bool {
        let changed = self
            .assignments
            .get(&key)
            .map(|c| c != &color)
            .unwrap_or(true);
        self.assignments.insert(key, color);
        changed
    }

    pub fn set_terminal_context(
        &mut self,
        terminal_id: String,
        worktree_key: Option<String>,
        color: String,
    ) -> bool {
        let changed = self
            .terminal_contexts
            .get(&terminal_id)
            .map(|existing| existing.worktree_key != worktree_key || existing.color != color)
            .unwrap_or(true);

        self.terminal_contexts.insert(
            terminal_id,
            TerminalContext {
                worktree_key,
                color,
            },
        );

        changed
    }

    pub fn current_for_terminal(&self, terminal_id: &str) -> Option<TerminalContext> {
        self.terminal_contexts.get(terminal_id).cloned()
    }

    pub fn counts(&self) -> (usize, usize) {
        let terminals = self.terminal_contexts.len();
        let active_worktrees = self
            .terminal_contexts
            .values()
            .filter_map(|ctx| ctx.worktree_key.as_ref())
            .collect::<HashSet<_>>()
            .len();

        (terminals, active_worktrees)
    }

    pub fn assigned_colors_excluding_key(&self, exclude_key: Option<&str>) -> HashSet<String> {
        self.assignments
            .iter()
            .filter(|(key, _)| match exclude_key {
                Some(exclude) => key.as_str() != exclude,
                None => true,
            })
            .map(|(_, color)| color.clone())
            .collect()
    }

    pub fn active_colors_excluding_key(&self, exclude_key: Option<&str>) -> HashSet<String> {
        self.terminal_contexts
            .values()
            .filter(|ctx| match (&ctx.worktree_key, exclude_key) {
                (Some(key), Some(exclude)) => key != exclude,
                _ => true,
            })
            .map(|ctx| ctx.color.clone())
            .collect()
    }

    pub fn terminals_for_worktree(&self, worktree_key: &str) -> Vec<String> {
        self.terminal_contexts
            .iter()
            .filter(|(_, ctx)| {
                ctx.worktree_key
                    .as_ref()
                    .map_or(false, |key| key == worktree_key)
            })
            .map(|(terminal_id, _)| terminal_id.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn roundtrip_state_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("state.json");

        let mut state = RuntimeState::default();
        state.set_assignment("repo\0wt".into(), "#111111".into());
        state.save(&path).unwrap();

        let loaded = RuntimeState::load(&path).unwrap();
        assert_eq!(loaded.assignment_for("repo\0wt"), Some("#111111".into()));
    }

    #[test]
    fn set_terminal_context_detects_changes_and_noops() {
        let mut state = RuntimeState::default();

        let changed = state.set_terminal_context(
            "/dev/ttys001".into(),
            Some("repo\0wt".into()),
            "#111111".into(),
        );
        assert!(changed);

        let changed = state.set_terminal_context(
            "/dev/ttys001".into(),
            Some("repo\0wt".into()),
            "#111111".into(),
        );
        assert!(!changed);

        let changed = state.set_terminal_context(
            "/dev/ttys001".into(),
            Some("repo\0wt2".into()),
            "#222222".into(),
        );
        assert!(changed);
    }

    #[test]
    fn counts_distinct_worktrees_across_terminals() {
        let mut state = RuntimeState::default();
        state.set_terminal_context(
            "/dev/ttys001".into(),
            Some("repo\0wt-a".into()),
            "#111111".into(),
        );
        state.set_terminal_context(
            "/dev/ttys002".into(),
            Some("repo\0wt-a".into()),
            "#111111".into(),
        );
        state.set_terminal_context(
            "/dev/ttys003".into(),
            Some("repo\0wt-b".into()),
            "#222222".into(),
        );
        state.set_terminal_context("/dev/ttys004".into(), None, "#1f1f1f".into());

        let (terminals, active_worktrees) = state.counts();
        assert_eq!(terminals, 4);
        assert_eq!(active_worktrees, 2);
    }

    #[test]
    fn assigned_colors_excluding_key_works() {
        let mut state = RuntimeState::default();
        state.set_assignment("a".into(), "#111111".into());
        state.set_assignment("b".into(), "#222222".into());

        let colors = state.assigned_colors_excluding_key(Some("a"));
        assert!(colors.contains("#222222"));
        assert!(!colors.contains("#111111"));
    }

    #[test]
    fn active_colors_excluding_key_works() {
        let mut state = RuntimeState::default();
        state.set_terminal_context("/dev/ttys001".into(), Some("a".into()), "#111111".into());
        state.set_terminal_context("/dev/ttys002".into(), Some("b".into()), "#222222".into());

        let colors = state.active_colors_excluding_key(Some("a"));
        assert!(colors.contains("#222222"));
        assert!(!colors.contains("#111111"));
    }

    #[test]
    fn terminals_for_worktree_returns_matching_terminals() {
        let mut state = RuntimeState::default();
        state.set_terminal_context("/dev/ttys001".into(), Some("repo\0wt-a".into()), "#111111".into());
        state.set_terminal_context("/dev/ttys002".into(), Some("repo\0wt-a".into()), "#111111".into());
        state.set_terminal_context("/dev/ttys003".into(), Some("repo\0wt-b".into()), "#222222".into());
        state.set_terminal_context("/dev/ttys004".into(), None, "#333333".into());

        let terminals = state.terminals_for_worktree("repo\0wt-a");
        assert_eq!(terminals.len(), 2);
        assert!(terminals.contains(&"/dev/ttys001".to_string()));
        assert!(terminals.contains(&"/dev/ttys002".to_string()));

        let terminals_b = state.terminals_for_worktree("repo\0wt-b");
        assert_eq!(terminals_b.len(), 1);
        assert!(terminals_b.contains(&"/dev/ttys003".to_string()));

        let terminals_none = state.terminals_for_worktree("nonexistent");
        assert_eq!(terminals_none.len(), 0);
    }

    #[test]
    fn cycle_color_workflow_updates_all_terminals() {
        let mut state = RuntimeState::default();
        let worktree_key = "repo\0wt-a";

        // Initial state: two terminals in the same worktree with color #111111
        state.set_assignment(worktree_key.into(), "#111111".into());
        state.set_terminal_context("/dev/ttys001".into(), Some(worktree_key.into()), "#111111".into());
        state.set_terminal_context("/dev/ttys002".into(), Some(worktree_key.into()), "#111111".into());

        // Verify both terminals have the old color
        let ctx1 = state.current_for_terminal("/dev/ttys001").unwrap();
        let ctx2 = state.current_for_terminal("/dev/ttys002").unwrap();
        assert_eq!(ctx1.color, "#111111");
        assert_eq!(ctx2.color, "#111111");

        // Simulate cycle-color: change assignment to new color
        state.set_assignment(worktree_key.into(), "#999999".into());

        // Get terminals for this worktree (what cycle-color does)
        let terminals = state.terminals_for_worktree(worktree_key);
        assert_eq!(terminals.len(), 2);

        // Update each terminal's context with the new color (what cycle-color should do)
        for terminal_id in terminals {
            state.set_terminal_context(
                terminal_id,
                Some(worktree_key.into()),
                "#999999".into(),
            );
        }

        // Verify both terminals now have the new color
        let ctx1_after = state.current_for_terminal("/dev/ttys001").unwrap();
        let ctx2_after = state.current_for_terminal("/dev/ttys002").unwrap();
        assert_eq!(ctx1_after.color, "#999999");
        assert_eq!(ctx2_after.color, "#999999");

        // Verify assignment was updated
        assert_eq!(state.assignment_for(worktree_key), Some("#999999".into()));
    }
}
