#!/bin/bash
# Setup script for prek pre-commit hooks

echo "Setting up prek pre-commit hooks for doc-assist..."

# Install prek if not already installed
if ! command -v prek &> /dev/null; then
    echo "Installing prek..."
    cargo install prek
fi

# Initialize prek in the repository
prek init

echo "Prek pre-commit hooks configured successfully!"
echo ""
echo "Hooks installed:"
echo "  - Format check (cargo fmt)"
echo "  - Clippy pedantic mode (with nursery and cargo lints)"
echo "  - Compilation check"
echo "  - Security audit"
echo "  - Documentation check"
echo ""
echo "To run hooks manually: prek run"
echo "To skip hooks temporarily: git commit --no-verify"