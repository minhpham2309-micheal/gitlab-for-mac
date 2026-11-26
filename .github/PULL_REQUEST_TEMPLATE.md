## Summary

<!-- Briefly explain what changed and why. -->

## Type of Change

- [ ] `feat`: New feature
- [ ] `fix`: Bug fix
- [ ] `refactor`: Code restructuring without behavior change
- [ ] `docs`: Documentation only
- [ ] `test`: Test coverage additions or updates
- [ ] `chore` / `ci`: Tooling, dependencies, or CI/CD

## Verification

- [ ] `cargo clippy --locked --lib --bins -- -D warnings`
- [ ] `cargo clippy --locked --all-targets -- -D warnings`
- [ ] `cargo test --locked`
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps`
- [ ] `cargo deny --locked check` (when dependencies change)
- [ ] `.github/scripts/check-repo-hygiene.sh`
- [ ] No real tokens, `.env*` files, local paths, or internal hostnames committed
