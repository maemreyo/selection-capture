#!/bin/bash
set -e

# Manual release script for selection-capture
# Usage: ./scripts/release.sh [patch|minor|major]

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Functions
print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Check if running from correct directory
if [ ! -f "Cargo.toml" ]; then
    print_error "Must run from project root (where Cargo.toml is located)"
    exit 1
fi

# Get current version
CURRENT_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
print_info "Current version: $CURRENT_VERSION"

# Determine bump type
BUMP_TYPE=${1:-patch}
case $BUMP_TYPE in
    patch|minor|major)
        ;;
    *)
        print_error "Invalid bump type: $BUMP_TYPE"
        echo "Usage: $0 [patch|minor|major]"
        exit 1
        ;;
esac

# Calculate new version
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"
case $BUMP_TYPE in
    patch)
        PATCH=$((PATCH + 1))
        ;;
    minor)
        MINOR=$((MINOR + 1))
        PATCH=0
        ;;
    major)
        MAJOR=$((MAJOR + 1))
        MINOR=0
        PATCH=0
        ;;
esac
NEW_VERSION="$MAJOR.$MINOR.$PATCH"

print_info "New version will be: $NEW_VERSION"
echo ""

# Pre-flight checks
print_info "Running pre-flight checks..."

# Check if on main branch
BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$BRANCH" != "main" ]; then
    print_warning "Not on main branch (currently on: $BRANCH)"
    read -p "Continue anyway? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# Check for uncommitted changes
if [ -n "$(git status --porcelain)" ]; then
    print_warning "Working directory has uncommitted changes"
    read -p "Continue anyway? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# Run tests
print_info "Running tests..."
if ! cargo test --quiet; then
    print_error "Tests failed. Aborting release."
    exit 1
fi
print_success "Tests passed"

# Run Clippy
print_info "Running Clippy..."
if ! cargo clippy --all-targets --all-features -- -D warnings; then
    print_error "Clippy failed. Aborting release."
    exit 1
fi
print_success "Clippy passed"

# Check formatting
print_info "Checking code formatting..."
if ! cargo fmt --check; then
    print_error "Code is not formatted. Run 'cargo fmt' first."
    exit 1
fi
print_success "Code is formatted"

# Generate changelog
print_info "Generating changelog..."
if command -v git-cliff &> /dev/null; then
    git cliff --unreleased --output CHANGELOG.md.tmp || true
    if [ -f CHANGELOG.md.tmp ]; then
        # Merge with existing changelog
        if [ -f CHANGELOG.md ]; then
            # Extract header from existing changelog
            head -n 10 CHANGELOG.md > CHANGELOG.md.header
            cat CHANGELOG.md.tmp >> CHANGELOG.md.header
            mv CHANGELOG.md.header CHANGELOG.md
            rm -f CHANGELOG.md.tmp
        else
            mv CHANGELOG.md.tmp CHANGELOG.md
        fi
        print_success "Changelog updated"
    fi
else
    print_warning "git-cliff not found. Skipping automatic changelog generation."
    print_info "Install with: cargo install git-cliff"
fi

# Update version in Cargo.toml
print_info "Updating version in Cargo.toml..."
sed -i.bak "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" Cargo.toml
rm -f Cargo.toml.bak
print_success "Version updated to $NEW_VERSION"

# Commit changes
print_info "Creating release commit..."
git add Cargo.toml CHANGELOG.md
git commit -m "release: v$NEW_VERSION"
print_success "Release commit created"

# Create git tag
TAG="v$NEW_VERSION"
print_info "Creating git tag: $TAG"
git tag -a "$TAG" -m "Release $TAG"
print_success "Git tag created"

# Push to GitHub
print_info "Pushing to GitHub..."
git push origin main
git push origin "$TAG"
print_success "Changes pushed to GitHub"

# Publish to crates.io
print_info "Publishing to crates.io..."
if command -v cargo &> /dev/null; then
    if cargo publish --dry-run; then
        read -p "Ready to publish to crates.io. Continue? (y/N): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            cargo publish
            print_success "Published to crates.io!"
        else
            print_warning "Skipping crates.io publish"
            print_info "You can publish manually later with: cargo publish"
        fi
    else
        print_error "crates.io publish check failed. Fix issues before publishing."
        exit 1
    fi
fi

# Summary
echo ""
print_success "🎉 Release $TAG completed successfully!"
echo ""
echo "Summary:"
echo "  - Version: $CURRENT_VERSION → $NEW_VERSION"
echo "  - Git tag: $TAG"
echo "  - Commit: $(git rev-parse --short HEAD)"
echo ""
echo "Next steps:"
echo "  1. Check GitHub Actions: https://github.com/maemreyo/selection-capture/actions"
echo "  2. Verify on crates.io: https://crates.io/crates/selection-capture"
echo "  3. Create GitHub Release: https://github.com/maemreyo/selection-capture/releases/new"
echo ""
