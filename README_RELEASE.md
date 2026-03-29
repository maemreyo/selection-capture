# 🚀 Release Automation Summary

## ✅ Complete Release System Installed!

Your project now has **professional-grade release automation** with automatic versioning, changelog generation, and publishing.

---

## 📦 What Was Installed

### 1. **Release Tools** (3 core tools)
- ✅ **cargo-release** - Automates entire release process
- ✅ **git-cliff** - Generates changelog from git commits
- ✅ **cargo-edit** - Command-line Cargo.toml management

### 2. **Configuration Files** (2 files)
- ✅ **release.toml** - cargo-release settings
- ✅ **cliff.toml** - git-cliff changelog configuration

### 3. **Scripts** (3 automation scripts)
- ✅ `scripts/install-tools.sh` - Install all tools
- ✅ `scripts/setup-release.sh` - Complete setup
- ✅ `scripts/release.sh` - Manual release workflow

### 4. **Makefile Commands** (9 new commands)
- ✅ `make install-tools`
- ✅ `make release-patch`
- ✅ `make release-minor`
- ✅ `make release-major`
- ✅ `make release-dry-run`
- ✅ `make changelog`
- ✅ `make changelog-preview`
- ✅ `make version-bump`
- ✅ `make publish`

### 5. **Documentation** (3 guides)
- ✅ `RELEASE_GUIDE.md` - Comprehensive guide
- ✅ `QUICKSTART_PUBLISH.md` - Quick start guide
- ✅ `README_RELEASE.md` - This file

---

## ⚡ Quick Start (Choose One)

### Option A: Super Fast (1 command)
```bash
make release-patch
```

### Option B: Recommended Flow
```bash
# 1. Install tools (one-time)
make install-tools

# 2. Test release
make release-dry-run

# 3. Execute release
make release-patch
```

### Option C: Full Control
```bash
# Install tools
cargo install cargo-release git-cliff cargo-edit

# Configure
# Edit release.toml and cliff.toml if needed

# Test
cargo release patch --test

# Execute
cargo release patch --execute
```

---

## 🎯 How It Works

### Automatic Version Bumping

```bash
# Current version: 0.1.0

make release-patch    # → 0.1.1 (bug fixes)
make release-minor    # → 0.2.0 (new features)
make release-major    # → 1.0.0 (breaking changes)
```

### Automatic Changelog Generation

Based on your commit messages:

```bash
git commit -m "feat: add Safari support"
git commit -m "fix: resolve race condition"
git commit -m "docs: update README"

# Run:
git cliff -o CHANGELOG.md

# Result:
## [0.1.1] - 2026-03-29

### Features
- Add Safari support

### Bug Fixes
- Resolve race condition

### Documentation
- Update README
```

### Complete Release Flow

```bash
$ make release-patch

# What happens:
1. ✅ Runs tests (cargo test)
2. ✅ Runs Clippy (cargo clippy)
3. ✅ Checks formatting (cargo fmt)
4. ✅ Bumps version (0.1.0 → 0.1.1)
5. ✅ Updates Cargo.toml
6. ✅ Generates changelog
7. ✅ Creates git commit
8. ✅ Creates git tag (v0.1.1)
9. ✅ Pushes to GitHub
10. ✅ Publishes to crates.io
```

---

## 📖 Commit Message Format

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>: <description>

[optional body]

[optional BREAKING CHANGE footer]
```

### Types & Version Impact

| Type | Example | Triggers |
|------|---------|----------|
| `feat:` | `feat: add clipboard fallback` | MINOR (0.1.0 → 0.2.0) |
| `fix:` | `fix: handle null selection` | PATCH (0.1.0 → 0.1.1) |
| `BREAKING CHANGE:` | In commit body | MAJOR (0.1.0 → 1.0.0) |
| `docs:` | `docs: update installation` | No version bump |
| `test:` | `test: add integration tests` | No version bump |
| `refactor:` | `refactor: simplify engine` | No version bump |
| `chore:` | `chore: update dependencies` | No version bump |

### Examples

```bash
# Good commits that auto-generate changelog:
git commit -m "feat: support Chrome browser"
git commit -m "fix: memory leak in retry logic"
git commit -m "perf: improve capture speed by 20%"
git commit -m "docs: add quickstart guide"

# Breaking change (major version bump):
git commit -m "feat: redesign CaptureOutcome

BREAKING CHANGE: CaptureOutcome is now an enum instead of Result"
```

---

## 🔧 Configuration

### release.toml Settings

```toml
[package]
tag-prefix = "v"              # Tags like v1.0.0
tag-message = "Release {{version}}"
pre-release-commit-message = "release: {{version}}"
publish = true                # Publish to crates.io
push = true                   # Push git changes
verify = true                 # Run tests first
main-branch = "main"          # Release from main
```

### cliff.toml Settings

Configures how changelog is generated from commits:
- Commit type parsing
- Group organization
- Link generation
- Formatting templates

---

## 📊 Complete Workflow

### First Time Setup

```bash
# 1. Install tools
make install-tools

# 2. Get crates.io token
# Visit https://crates.io/settings/tokens
cargo login <your-token>

# 3. Initialize git
git init
git add .
git commit -m "Initial commit"
git branch -M main

# 4. Connect to GitHub
git remote add origin https://github.com/maemreyo/selection-capture.git
git push -u origin main

# 5. Test release
make release-dry-run

# 6. Execute first release
make release-minor    # Creates v0.1.0
```

### Regular Release Workflow

```bash
# Work on features/fixes
git commit -m "feat: add new feature"
git commit -m "fix: fix bug"

# When ready to release:
make ci                 # Run all checks
git cliff --unreleased  # Preview changelog
make release-dry-run    # Test release
make release-patch      # Execute release
```

---

## 🎨 Customization

### Change Version Bump Behavior

Edit `release.toml`:
```toml
[package]
# Auto-detect version from commits
release-type = "auto"

# Or force specific type
# release-type = "patch"
# release-type = "minor"
# release-type = "major"
```

### Customize Changelog Format

Edit `cliff.toml` to change:
- Section ordering
- Commit grouping rules
- Markdown formatting
- Link patterns

### Add Custom Hooks

In `release.toml`:
```toml
[package]
# Run before release
pre-release-hook = ["sh", "-c", "echo 'Preparing release...'"]

# Run after release
post-release-hook = ["sh", "-c", "echo 'Release complete!'"]
```

---

## 🚨 Common Issues & Solutions

### Issue: "Uncommitted changes"
```bash
# Solution: Commit or stash
git commit -am "WIP"
# or
git stash
```

### Issue: "Tests failed"
```bash
# Solution: Fix tests first
cargo test
# Then release
```

### Issue: "Tag already exists"
```bash
# Solution: Delete old tag
git tag -d v0.1.1
git push origin :refs/tags/v0.1.1
```

### Issue: "Not on main branch"
```bash
# Solution: Switch to main
git checkout main
git pull
```

---

## 📈 Advanced Features

### Pre-releases

```bash
# Create beta release (0.1.0-beta.1)
cargo release rc --identifier beta.1 --execute

# Create release candidate (0.1.0-rc.1)
cargo release rc --identifier rc.1 --execute
```

### Manual Version Control

```bash
# Set exact version
cargo set-version 0.2.0

# Commit manually
git commit -am "Bump version to 0.2.0"

# Tag manually
git tag -a v0.2.0 -m "Release v0.2.0"

# Push
git push && git push --tags

# Publish
cargo publish
```

### Rollback

```bash
# Unpublish from crates.io (within 72h)
cargo unpublish selection-capture@0.1.1

# Delete tag
git tag -d v0.1.1
git push origin :refs/tags/v0.1.1

# Revert version
cargo set-version 0.1.0
git commit -am "Revert to 0.1.0"
git push
```

---

## 🎯 Best Practices

### Before Each Release
- ✅ Run full test suite
- ✅ Check Clippy warnings
- ✅ Verify formatting
- ✅ Review changelog preview
- ✅ Test critical examples

### Release Schedule
- **Patch**: As needed (critical fixes)
- **Minor**: Every 2-4 weeks
- **Major**: When API stabilizes

### Commit Hygiene
- Use conventional commits
- Write clear descriptions
- Mention breaking changes
- Reference issues/PRs

---

## 📞 Reference Commands

### Installation
```bash
make install-tools          # Install all tools
make setup-release          # Complete setup
```

### Version Management
```bash
make release-patch          # Bump patch (0.1.0 → 0.1.1)
make release-minor          # Bump minor (0.1.0 → 0.2.0)
make release-major          # Bump major (0.1.0 → 1.0.0)
make release-dry-run        # Test run
make version-bump VER=0.2.0 # Set exact version
```

### Changelog
```bash
make changelog              # Generate full changelog
make changelog-preview      # Preview unreleased
```

### Publishing
```bash
make publish                # Publish to crates.io
make publish-dry-run        # Test publish
```

### Information
```bash
cargo outdated              # Check outdated deps
cargo audit                 # Security audit
cargo tree                  # View dependencies
```

---

## 🌟 Success Metrics

After successful release:

✅ GitHub shows new tag  
✅ GitHub Actions passed  
✅ crates.io updated  
✅ docs.rs building  
✅ Changelog current  
✅ Git history clean  

---

## 📚 Documentation Links

- [RELEASE_GUIDE.md](RELEASE_GUIDE.md) - Full detailed guide
- [QUICKSTART_PUBLISH.md](QUICKSTART_PUBLISH.md) - Quick start
- [release.toml](release.toml) - Configuration
- [cliff.toml](cliff.toml) - Changelog config
- [cargo-release docs](https://github.com/crate-ci/cargo-release)
- [git-cliff docs](https://git-cliff.org)

---

## 🎉 You're All Set!

Your project now has **enterprise-grade release automation**:

- ✨ One-command releases
- ✨ Automatic versioning (semver)
- ✨ Auto-generated changelogs
- ✨ Git tag management
- ✨ crates.io publishing
- ✨ Quality gates (tests, linting)

**Next step:**
```bash
make release-patch
```

And watch the magic happen! 🚀

---

*Created: 2026-03-29*  
*Project: selection-capture*  
*Author: zamery (zaob.ogn@gmail.com)*
