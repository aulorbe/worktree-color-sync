# Contributing

Thanks for contributing to Worktree Color Sync.

## Workflow

1. Fork the repo (or work from a write-enabled clone).
2. Create a feature branch from `main`:
   - `git checkout -b feat/short-description`
3. Make your change with tests and docs updates when needed.
4. Run checks before opening a PR:
   - `cargo fmt --all`
   - `cargo check --workspace`
   - `cargo test --workspace`
5. Push your branch and open a Pull Request.
6. Request review before merge.

## Branch Rules

- `main` is protected.
- No direct pushes to `main`.
- All changes must go through a feature branch + Pull Request review.

## Pull Request Guidelines

- Keep PRs focused and reasonably small.
- Explain what changed and why.
- Call out risks, edge cases, and follow-up work.
- Include reproduction/verification steps.

## Code Style

- Rust 2021 edition.
- Use `cargo fmt` for formatting.
- Prefer clear, explicit logic over cleverness.
- Preserve non-destructive behavior for user config files.

