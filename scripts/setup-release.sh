#!/bin/bash
# Quick setup for first-time release
set -e

echo "🚀 Setting up release automation..."
echo ""

# Install tools
./scripts/install-tools.sh

# Initialize git if needed
if [ ! -d ".git" ]; then
    echo "Initializing git repository..."
    git init
    git add .
    git commit -m "Initial commit"
fi

# Ensure on main branch
git branch -M main 2>/dev/null || true

echo ""
echo "✅ Setup complete!"
echo ""
echo "Your next steps:"
echo "  1. Add remote: git remote add origin https://github.com/maemreyo/selection-capture.git"
echo "  2. Push code: git push -u origin main"
echo "  3. Login to crates.io: cargo login <your-token>"
echo "  4. Test release: cargo release patch --test"
echo "  5. Execute release: cargo release patch --execute"
echo ""
