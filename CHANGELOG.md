# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Platform-neutral strategy model (`CaptureMethod::{AccessibilityPrimary, AccessibilityRange, ClipboardBorrow, SyntheticCopy}`)
- `PlatformCapabilities` boundary type
- Feature-gated platform modules: `windows-beta`, `linux-alpha`, `async`
- Windows beta scaffold with dispatch mapping and smoke tests
- Linux alpha scaffold with dispatch mapping and smoke tests
- Shared engine integration tests (`tests/shared_engine.rs`)
- Test matrix documentation (`docs/technical/TEST_MATRIX.md`)
- Optional async wrapper API: `capture_async(...)`
- Profile-aware default method prioritization helper (`last_success_method`)
- Performance smoke test for profile-based method ordering
- Experimental monitoring API surface: `MonitorPlatform` and `CaptureMonitor<P>`
- Monitoring technical documentation (`docs/technical/MONITORING.md`)
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
- CI now runs feature-gated Windows beta and Linux alpha tests on native runners
- README examples and links updated to match current public API and docs paths
- Cargo package include paths updated to include current technical docs and tests
- Cross-platform testing guide rewritten around current CI/test strategy
- Updated README.md with badges, architecture diagram, and improved documentation
- Changed license to dual MIT/Apache-2.0 for broader compatibility
- Enhanced Cargo.toml metadata for crates.io publishing

### Fixed
- `clippy::items-after-test-module` ordering issue in `src/types.rs`
- formatting drift across integration test files and async wrapper
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
