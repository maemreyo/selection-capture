# Releases

This file documents the release history and changes for selection-capture.

## Release Process

### Version Numbering

This project follows [Semantic Versioning](https://semver.org/) (MAJOR.MINOR.PATCH):

- **MAJOR** version for incompatible API changes
- **MINOR** version for backwards-compatible functionality additions
- **PATCH** version for backwards-compatible bug fixes

### Steps to Release

1. Update version number in `Cargo.toml`
2. Update `CHANGELOG.md` with release date and changes
3. Run all tests: `cargo test`
4. Run Clippy: `cargo clippy --all-targets -- -D warnings`
5. Format code: `cargo fmt`
6. Build release: `cargo build --release`
7. Commit changes with message: `release: v{version}`
8. Create git tag: `git tag -a v{version} -m "Release v{version}"`
9. Push to GitHub: `git push && git push --tags`
10. Publish to crates.io: `cargo publish`

### Pre-release Checklist

- [ ] All CI checks pass
- [ ] Documentation is up-to-date
- [ ] Examples work as expected
- [ ] No breaking changes (or properly documented if there are)
- [ ] Changelog is complete
- [ ] Version numbers updated
- [ ] Dependencies are up-to-date

## Publishing to crates.io

Before publishing, ensure you have:

1. An account at [crates.io](https://crates.io)
2. Logged in via `cargo login <your-api-token>`
3. All metadata fields in `Cargo.toml` are correct

Then run:

```bash
cargo publish
```

## Release Notes Template

```markdown
## [X.Y.Z] - YYYY-MM-DD

### Added
- New features

### Changed
- Changes to existing functionality

### Deprecated
- Soon-to-be removed features

### Removed
- Removed features

### Fixed
- Bug fixes

### Security
- Security improvements
```
