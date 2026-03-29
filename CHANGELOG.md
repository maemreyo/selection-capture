# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Comprehensive open-source project structure
- CONTRIBUTING.md with contribution guidelines
- CODE_OF_CONDUCT.md (Contributor Covenant v2.1)
- SECURITY.md with vulnerability reporting process
- DEVELOPMENT.md with developer guide
- RELEASES.md with release process documentation
- Makefile with common development tasks
- Issue templates (bug report, feature request, question)
- Pull request template
- FUNDING.yml for GitHub Sponsors
- Dual licensing (MIT OR Apache-2.0)
- Enhanced CI workflow with multiple jobs:
  - Format checking (rustfmt)
  - Linting (clippy)
  - Testing on macOS
  - Build verification on Linux
  - Security audit (cargo-audit)
  - Documentation build
- rustfmt.toml for consistent formatting
- Enhanced .gitignore with comprehensive exclusions
- Release optimization profiles in Cargo.toml

### Changed
- Updated README.md with badges, architecture diagram, and improved documentation
- Changed license to dual MIT/Apache-2.0 for broader compatibility
- Enhanced Cargo.toml metadata for crates.io publishing

### Fixed
- Profile configuration inconsistencies

## [0.1.0] - 2026-03-29

### Added
- Initial extraction from `zmr-koe` into standalone `selection-capture` crate
- Core synchronous capture engine
- Platform trait design (`CapturePlatform`)
- macOS implementation (`MacOSPlatform`)
- App profile system with merge-based updates
- Strategy pattern with automatic fallback
- Retry logic with configurable budgets
- Cooperative cancellation via `CancelSignal` trait
- Detailed capture tracing
- Automatic clipboard cleanup
- Publish-ready metadata (`repository`, `documentation`, keywords/categories)
- Basic README, MIT license, CI workflow scaffold

[Unreleased]: https://github.com/maemreyo/selection-capture/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/maemreyo/selection-capture/releases/tag/v0.1.0
