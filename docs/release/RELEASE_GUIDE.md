# Release Automation Guide

This guide covers automated versioning, changelog generation, and publishing.

## Tools We Use

### 1. **cargo-release** (Recommended)
The most popular tool for Rust project releases.

**Installation:**
```bash
cargo install cargo-release
```

**Features:**
- Automatic version bumping (semver)
- Git tag creation
- Changelog generation
- Publishing to crates.io
- Git commit automation

### 2. **git-cliff**
Generate changelogs from git commits using conventional commits.

**Installation:**
```bash
cargo install git-cliff
```

### 3. **cargo-edit**
Manage Cargo.toml from command line.

**Installation:**
```bash
cargo install cargo-edit
```

---

## Quick Start: Publish a New Version

### Option A: Using cargo-release (Recommended)

```bash
# Test run (dry-run) - see what would happen
cargo release patch --test

# Actual release (patch version: 0.1.0 -> 0.1.1)
cargo release patch --execute

# For minor version (0.1.0 -> 0.2.0)
cargo release minor --execute

# For major version (0.1.0 -> 1.0.0)
cargo release major --execute
```

### Option B: Manual Scripts

```bash
# Patch release (bug fixes)
./scripts/release.sh patch

# Minor release (new features, backwards compatible)
./scripts/release.sh minor

# Major release (breaking changes)
./scripts/release.sh major
```

---

## Detailed Workflow

### Step 1: Install Required Tools

```bash
# Core tools
cargo install cargo-release
cargo install git-cliff
cargo install cargo-edit

# Optional but recommended
cargo install cargo-audit      # Security audits
cargo install cargo-outdated   # Check outdated deps
```

### Step 2: Configure Release Settings

The `release.toml` file is already configured with sensible defaults.

Key settings:
- Pre-commit hooks: Run tests and clippy
- Pre-publish hooks: Build and verify
- Post-publish: Push git tags
- Changelog: Auto-generate from commits

### Step 3: Prepare Release

```bash
# Check current version
cargo metadata --format-version=1 | jq -r '.packages[] | select(.name == "selection-capture") | .version'

# Run all checks
make ci

# Generate changelog preview
git cliff --unreleased --preview
```

### Step 4: Execute Release

```bash
# Dry run first
cargo release patch --test

# If everything looks good, execute
cargo release patch --execute
```

What this does:
1. ✅ Runs pre-commit hooks (tests, clippy)
2. ✅ Bumps version in Cargo.toml
3. ✅ Commits changes
4. ✅ Creates git tag
5. ✅ Generates/updates CHANGELOG.md
6. ✅ Pushes to GitHub
7. ✅ Publishes to crates.io

### Step 5: Verify

Check:
- GitHub repository for new tag
- GitHub Actions for CI status
- crates.io for published version
- CHANGELOG.md for updates

---

## Version Numbering

Follow [Semantic Versioning](https://semver.org/):

- **MAJOR.MINOR.PATCH** (e.g., 0.1.0)
- **MAJOR**: Breaking changes
- **MINOR**: New features (backwards compatible)
- **PATCH**: Bug fixes (backwards compatible)

Examples:
- `0.1.0` → `0.1.1` (patch: bug fix)
- `0.1.0` → `0.2.0` (minor: new feature)
- `0.1.0` → `1.0.0` (major: breaking change or stable release)

---

## Changelog Generation

### Using git-cliff

```bash
# Generate full changelog
git cliff -o CHANGELOG.md

# Generate only unreleased changes
git cliff --unreleased --preview

# Generate since last tag
git cliff --latest --preview
```

### Commit Message Format

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add new capture strategy
fix: handle edge case in macOS 15
docs: update README examples
test: add integration tests
refactor: improve error handling
chore: update dependencies
```

Types:
- `feat`: New feature (triggers MINOR version)
- `fix`: Bug fix (triggers PATCH version)
- `BREAKING CHANGE`: Breaking change (triggers MAJOR version)

---

## Publishing to crates.io

### Prerequisites

1. Create account at https://crates.io
2. Get API token from Account Settings
3. Login locally:

```bash
cargo login <your-api-token>
```

### Publish Command

```bash
# Verify package
cargo publish --dry-run

# Actually publish
cargo publish
```

**Note:** `cargo-release` handles this automatically!

---

## Automated Workflows

### GitHub Actions (Auto-publish on Tag)

The `.github/workflows/release.yml` workflow:
- Triggers on git tag push (v*)
- Builds and tests
- Publishes to crates.io
- Creates GitHub Release

### Manual Release Script

For more control, use the manual script:

```bash
./scripts/release.sh patch
```

This script:
1. Checks current branch is main
2. Ensures working directory is clean
3. Runs all tests
4. Runs Clippy
5. Bumps version
6. Updates CHANGELOG
7. Creates git commit
8. Creates git tag
9. Pushes to GitHub
10. Publishes to crates.io

---

## Rollback (If Something Goes Wrong)

### Unpublish from crates.io (within 72 hours)

```bash
cargo unpublish selection-capture@0.1.1
```

### Delete Git Tag

```bash
git tag -d v0.1.1
git push origin :refs/tags/v0.1.1
```

### Revert Version in Cargo.toml

```bash
cargo set-version 0.1.0
git commit -am "revert: back to 0.1.0"
git push
```

---

## Best Practices

### Before Release
- ✅ Run full test suite: `cargo test`
- ✅ Run Clippy: `cargo clippy --all-targets -- -D warnings`
- ✅ Check formatting: `cargo fmt --check`
- ✅ Update documentation
- ✅ Review CHANGELOG
- ✅ Test examples

### After Release
- ✅ Verify crates.io page
- ✅ Check docs.rs build status
- ✅ Monitor GitHub Actions
- ✅ Announce on social media
- ✅ Update any related projects

### Release Frequency
- **Patch releases**: As needed for critical fixes
- **Minor releases**: Every 2-4 weeks for features
- **Major releases**: When API stabilizes or breaks

---

## Example Release Session

```bash
# 1. Check status
git status
git log --oneline -10

# 2. Ensure on main branch
git checkout main
git pull

# 3. Run all checks
make ci

# 4. Preview changelog
git cliff --unreleased --preview

# 5. Dry run release
cargo release patch --test

# 6. Execute release
cargo release patch --execute

# 7. Verify
cargo tree  # Check published version
open https://crates.io/crates/selection-capture
open https://github.com/maemreyo/selection-capture/releases
```

---

## Troubleshooting

### Issue: "There are uncommitted changes"

**Solution:**
```bash
git stash
# or
git commit -am "WIP"
# Then release
cargo release patch --execute
# After release, you can pop stash or amend commit
```

### Issue: "Git tag already exists"

**Solution:**
```bash
# Delete local tag
git tag -d v0.1.1

# Delete remote tag
git push origin :refs/tags/v0.1.1

# Try release again
```

### Issue: "crates.io token expired"

**Solution:**
```bash
cargo logout
cargo login <new-token>
```

### Issue: "Build fails on docs.rs"

**Solution:**
Check `Cargo.toml` has correct `package.metadata.docs.rs` settings:
```toml
[package.metadata.docs.rs]
targets = ["aarch64-apple-darwin", "x86_64-apple-darwin"]
all-features = true
```

---

## Additional Resources

- [cargo-release documentation](https://github.com/crate-ci/cargo-release)
- [git-cliff documentation](https://git-cliff.org)
- [crates.io publishing guide](https://doc.rust-lang.org/cargo/reference/publishing.html)
- [Semantic Versioning spec](https://semver.org)
- [Conventional Commits](https://www.conventionalcommits.org)

---

## Quick Reference Card

```bash
# Install tools
cargo install cargo-release git-cliff cargo-edit

# Check before release
cargo check && cargo test && cargo clippy --all-targets -- -D warnings

# Preview changelog
git cliff --unreleased --preview

# Release (choose one)
cargo release patch --execute    # 0.1.0 -> 0.1.1
cargo release minor --execute    # 0.1.0 -> 0.2.0
cargo release major --execute    # 0.1.0 -> 1.0.0

# Manual version bump
cargo set-version 0.2.0

# Generate changelog
git cliff -o CHANGELOG.md

# Publish manually
cargo publish
```

---

*Last updated: 2026-03-29*
*Project: selection-capture*
