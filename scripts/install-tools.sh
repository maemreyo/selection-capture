#!/bin/bash
# Install all release tools
set -e

echo "🔧 Installing release automation tools..."
echo ""

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust/Cargo not found. Please install Rust first:"
    echo "   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

echo "Installing cargo-release..."
cargo install cargo-release
echo "✅ cargo-release installed"
echo ""

echo "Installing git-cliff..."
cargo install git-cliff
echo "✅ git-cliff installed"
echo ""

echo "Installing cargo-edit..."
cargo install cargo-edit
echo "✅ cargo-edit installed"
echo ""

echo "Installing cargo-audit (optional but recommended)..."
cargo install cargo-audit
echo "✅ cargo-audit installed"
echo ""

echo "Installing cargo-outdated (optional)..."
cargo install cargo-outdated
echo "✅ cargo-outdated installed"
echo ""

echo "🎉 All tools installed successfully!"
echo ""
echo "Available commands:"
echo "  - cargo release [patch|minor|major] --execute"
echo "  - git cliff [options]"
echo "  - cargo set-version <version>"
echo "  - cargo audit"
echo "  - cargo outdated"
echo ""
echo "Configuration files:"
echo "  - release.toml (cargo-release settings)"
echo "  - cliff.toml (git-cliff settings)"
echo ""
echo "Usage examples:"
echo "  # Preview release (dry-run)"
echo "  cargo release patch --test"
echo ""
echo "  # Execute release"
echo "  cargo release patch --execute"
echo ""
echo "  # Generate changelog"
echo "  git cliff -o CHANGELOG.md"
echo ""
echo "  # Bump version manually"
echo "  cargo set-version 0.2.0"
echo ""
