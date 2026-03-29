# 🚀 Quick Start: Publish Your First Release

This is your step-by-step guide to publish `selection-capture` as an open-source project.

---

## ⚡ Super Quick Start (5 Minutes)

If you want to publish **RIGHT NOW**:

```bash
# 1. Install release tools
make install-tools

# 2. Login to crates.io
cargo login <your-api-token-from-crates-io>

# 3. Test release (dry-run)
make release-dry-run

# 4. Actually release
make release-patch
```

That's it! Your library will be published to crates.io automatically.

---

## 📋 Complete Setup Guide

### Step 1: Install Automation Tools

```bash
# Option A: Using Makefile (recommended)
make install-tools

# Option B: Manual installation
cargo install cargo-release git-cliff cargo-edit cargo-audit cargo-outdated
```

**What gets installed:**
- `cargo-release` - Automates versioning and publishing
- `git-cliff` - Generates changelog from git commits
- `cargo-edit` - Command-line Cargo.toml management
- `cargo-audit` - Security vulnerability checking
- `cargo-outdated` - Check for outdated dependencies

---

### Step 2: Get crates.io API Token

1. Go to https://crates.io
2. Click "Log in" → Sign in with GitHub
3. Go to Account Settings → API Tokens
4. Click "New Token"
5. Name it (e.g., "selection-capture")
6. Copy the token (you won't see it again!)

```bash
# Login with your token
cargo login <your-token>
```

---

### Step 3: Initialize Git Repository

```bash
# If not already done
git init
git add .
git commit -m "Initial commit: selection-capture v0.1.0"

# Ensure on main branch
git branch -M main
```

---

### Step 4: Connect to GitHub

```bash
# Add remote repository
git remote add origin https://github.com/maemreyo/selection-capture.git

# Push code
git push -u origin main
```

---

### Step 5: Test Release (Dry Run)

```bash
# See what would happen without actually doing anything
make release-dry-run

# Or using cargo-release directly
cargo release patch --test
```

This will show you:
- ✅ Version bump (0.1.0 → 0.1.1)
- ✅ Files that will be modified
- ✅ Git tag that will be created
- ✅ Commands that will run

---

### Step 6: Execute Release

```bash
# For bug fixes (0.1.0 → 0.1.1)
make release-patch

# For new features (0.1.0 → 0.2.0)
make release-minor

# For breaking changes (0.1.0 → 1.0.0)
make release-major
```

**What happens automatically:**
1. ✅ Runs tests and Clippy
2. ✅ Updates version in Cargo.toml
3. ✅ Creates git commit
4. ✅ Creates git tag (v0.1.1)
5. ✅ Generates changelog
6. ✅ Pushes to GitHub
7. ✅ Publishes to crates.io

---

## 🎯 Common Scenarios

### Scenario 1: First Release (v0.1.0)

You're ready to publish your first version!

```bash
# Make sure everything works
make ci

# Test release
cargo release minor --test

# Execute (creates 0.1.0)
cargo release minor --execute
```

---

### Scenario 2: Bug Fix Release

You fixed a bug and want to release quickly.

```bash
# Commit your fix with conventional commit
git commit -m "fix: handle edge case in macOS 15"

# Release patch version
make release-patch
```

---

### Scenario 3: New Feature Release

You added a new feature.

```bash
# Commit feature
git commit -m "feat: add support for Safari browser"

# Release minor version
make release-minor
```

---

### Scenario 4: Breaking Changes

You made incompatible API changes.

```bash
# Commit with BREAKING CHANGE notice
git commit -m "feat: redesign CaptureOutcome API

BREAKING CHANGE: CaptureOutcome now returns Result instead of Option"

# Release major version
make release-major
```

---

## 🔧 Useful Commands

### Version Management

```bash
# Check current version
cargo metadata | jq -r '.packages[] | select(.name == "selection-capture") | .version'

# Set version manually
cargo set-version 0.2.0

# Bump version
cargo release patch --execute
```

### Changelog

```bash
# Generate full changelog
git cliff -o CHANGELOG.md

# Preview unreleased changes
git cliff --unreleased --preview

# Show latest release
git cliff --latest
```

### Publishing

```bash
# Test publish
cargo publish --dry-run

# Actually publish
cargo publish

# Unpublish (within 72 hours)
cargo unpublish selection-capture@0.1.1
```

### Information

```bash
# Check for outdated deps
cargo outdated

# Security audit
cargo audit

# View package info
cargo tree
```

---

## 🎨 Commit Message Format

Use [Conventional Commits](https://www.conventionalcommits.org/) for automatic changelog generation:

```
<type>: <description>

[optional body]

[optional footer]
```

### Types

- `feat:` New feature (triggers MINOR version)
- `fix:` Bug fix (triggers PATCH version)
- `docs:` Documentation only
- `test:` Adding tests
- `refactor:` Code refactoring
- `chore:` Maintenance tasks
- `perf:` Performance improvements

### Examples

```bash
# Good commit messages
git commit -m "feat: add clipboard fallback strategy"
git commit -m "fix: resolve race condition in retry logic"
git commit -m "docs: update README examples"
git commit -m "test: add integration tests for macOS"
git commit -m "refactor: simplify error handling"

# With BREAKING CHANGE
git commit -m "feat: redesign public API

This changes how CaptureOutcome works.

BREAKING CHANGE: CaptureOutcome is now an enum instead of Result"
```

---

## 🚨 Troubleshooting

### Problem: "Working directory has uncommitted changes"

**Solution:**
```bash
# Commit or stash changes
git commit -am "WIP"
# or
git stash

# Then release
cargo release patch --execute
```

---

### Problem: "Not on main branch"

**Solution:**
```bash
# Switch to main
git checkout main
git pull

# Release from main
cargo release patch --execute
```

---

### Problem: "Tests failed"

**Solution:**
```bash
# Fix failing tests first
cargo test

# Then release
cargo release patch --execute
```

---

### Problem: "Token expired"

**Solution:**
```bash
# Get new token from crates.io
cargo logout
cargo login <new-token>
```

---

### Problem: "Git tag already exists"

**Solution:**
```bash
# Delete old tag
git tag -d v0.1.1
git push origin :refs/tags/v0.1.1

# Try again
cargo release patch --execute
```

---

## 📊 What Gets Published

When you run `cargo release`, these things happen:

### 1. Local Changes
- ✅ Version bumped in `Cargo.toml`
- ✅ CHANGELOG.md updated
- ✅ Git commit created
- ✅ Git tag created (e.g., v0.1.1)

### 2. Remote Changes
- ✅ Commit pushed to GitHub
- ✅ Tag pushed to GitHub
- ✅ Package published to crates.io
- ✅ GitHub Actions triggered

### 3. After Publication
- ✅ crates.io page updated
- ✅ docs.rs starts building docs
- ✅ Download stats tracked
- ✅ Version visible to users

---

## 🎯 Best Practices

### Before Each Release
- [ ] Run `make ci` (all checks pass)
- [ ] Review changelog preview
- [ ] Test examples manually
- [ ] Update documentation if needed

### Release Frequency
- **Patch releases**: As needed for critical fixes
- **Minor releases**: Every 2-4 weeks
- **Major releases**: When API stabilizes

### Version Numbers
- **0.1.x**: Initial development
- **0.x.0**: Feature-complete but unstable
- **1.0.0**: Stable API
- **2.0.0**: Breaking changes

---

## 🎉 Success Checklist

After successful release, verify:

- [ ] GitHub shows new tag
- [ ] GitHub Actions passed
- [ ] crates.io shows new version
- [ ] docs.rs building docs
- [ ] CHANGELOG.md updated
- [ ] Git history clean

---

## 📞 Need Help?

### Resources
- [cargo-release docs](https://github.com/crate-ci/cargo-release)
- [git-cliff docs](https://git-cliff.org)
- [crates.io guide](https://doc.rust-lang.org/cargo/reference/publishing.html)

### Commands Reference
```bash
# Full release workflow
make install-tools          # Install tools
make ci                     # Run all checks
make release-dry-run        # Test release
make release-patch          # Execute release
make changelog              # Generate changelog
make publish                # Publish to crates.io
```

---

## 🌟 You're Ready!

Your library is about to reach thousands of Rust developers. Good luck! 🚀

```bash
# The magic command to publish:
make release-patch
```

---

*Last updated: 2026-03-29*
*Project: selection-capture*
