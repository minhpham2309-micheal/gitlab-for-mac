# Repository Rules

## Branch & Pull Request Policy (Mandatory)

- **Never commit or push directly to `main`.** No direct pushes and no force-pushes to `main`.
- Always create a topic branch from `origin/main` (`feat/<slug>`, `fix/<slug>`, `chore/<slug>`, `docs/<slug>`), push the branch, and open a Pull Request targeting `main` for human review and merge.
- Before every `git push`, verify the target ref is not `main`.

## Verification Before Opening a PR

```bash
cargo clippy --all-targets -- -D warnings
cargo test
```

## Code & Hygiene Boundaries

- Keep `src/gitlab/` free of `gpui-kit` UI dependencies.
- Keep individual source files focused (under ~200 lines).
- Never commit secrets, real tokens, `.env*` files, local paths, or private/internal hostnames — use synthetic `*.example.com` / `*.test` fixtures.
- Use Conventional Commits (`feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `chore:`) with no AI attribution lines.
