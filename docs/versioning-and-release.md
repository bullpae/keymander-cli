# Versioning and Release Rules

This repository uses one product version across all runtime packages:

- `keymander` (CLI/TUI)
- `kmd-core` (shared core)
- `kmd-desktop` (desktop launcher)

## 1) Single Product Version

- Keep the three Cargo package versions identical.
- Use SemVer: `MAJOR.MINOR.PATCH`.
- Git tag format: `vMAJOR.MINOR.PATCH` (example: `v0.2.3`).
- GitHub release title: same as tag.

### Bump Rules

- `MAJOR`: breaking user-facing changes (CLI behavior, config compatibility, major UX break).
- `MINOR`: new user-facing features (commands, providers, UI capabilities).
- `PATCH`: bug fixes, refactors, performance/stability improvements without feature breaks.

## 2) Core Index Schema Version Is Separate

`kmd-core::Index::current_version()` is NOT product SemVer.
It tracks index/data compatibility and can move independently when schema/storage changes.

- Product version can bump without index schema bump.
- Index schema should bump only when index format/compatibility changes.

## 3) Release Flow

1. Ensure CI passes on `main`.
2. Update package versions in:
   - `Cargo.toml` (root `keymander`)
   - `crates/kmd-core/Cargo.toml`
   - `crates/kmd-desktop/Cargo.toml`
3. Commit release changes.
4. Create annotated tag: `vX.Y.Z`.
5. Push commit and tag.
6. Create GitHub Release from the tag with:
   - Summary of features/fixes
   - Test/validation notes

## 4) Practical Policy for This Repo

- Default policy: all three package versions move together.
- Exception policy: if only internal crates are published separately in the future,
  split policy can be introduced, but only with explicit docs update.
