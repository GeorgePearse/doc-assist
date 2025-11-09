#!/bin/bash

# doc-assist Installation Script
# This script installs doc-assist CLI tool

set -e

BOLD='\033[1m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BOLD}doc-assist Installer${NC}"
echo "=============================="
echo

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Error: Rust is not installed${NC}"
    echo "Please install Rust first from https://rustup.rs/"
    echo
    echo "Run this command:"
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# Check if git is installed
if ! command -v git &> /dev/null; then
    echo -e "${RED}Error: Git is not installed${NC}"
    echo "Please install git first"
    exit 1
fi

# Determine installation method
echo "Choose installation method:"
echo "1) Quick install from GitHub (recommended)"
echo "2) Build from source in current directory"
echo
read -p "Enter choice [1-2]: " choice

case $choice in
    1)
        echo
        echo -e "${YELLOW}Installing doc-assist from GitHub...${NC}"
        cargo install --git https://github.com/GeorgePearse/doc-assist
        ;;
    2)
        echo
        echo -e "${YELLOW}Building from source...${NC}"

        # Clone if not already in the repo
        if [ ! -f "Cargo.toml" ] || [ ! -d "src" ]; then
            echo "Cloning repository..."
            git clone https://github.com/GeorgePearse/doc-assist.git
            cd doc-assist
        fi

        echo "Building in release mode..."
        cargo build --release

        echo
        echo "Installing binary..."
        cargo install --path .
        ;;
    *)
        echo -e "${RED}Invalid choice${NC}"
        exit 1
        ;;
esac

echo
echo -e "${GREEN}✓ Installation complete!${NC}"
echo

# Check if cargo bin is in PATH
if [[ ":$PATH:" != *":$HOME/.cargo/bin:"* ]]; then
    echo -e "${YELLOW}Warning: ~/.cargo/bin is not in your PATH${NC}"
    echo "Add this to your shell configuration file (.bashrc, .zshrc, etc.):"
    echo "  export PATH=\"\$HOME/.cargo/bin:\$PATH\""
    echo
fi

# Verify installation
if command -v docassist &> /dev/null; then
    VERSION=$(docassist --version 2>/dev/null || echo "version unknown")
    echo -e "${GREEN}doc-assist installed successfully!${NC}"
    echo "Version: $VERSION"
    echo
    echo "To get started:"
    echo "  1. Set your API key:"
    echo "     export OPENAI_API_KEY='your-key-here'"
    echo "     # or"
    echo "     export ANTHROPIC_API_KEY='your-key-here'"
    echo
    echo "  2. Generate documentation:"
    echo "     docassist ."
    echo
    echo "  3. For help:"
    echo "     docassist --help"
else
    echo -e "${YELLOW}Installation completed but 'docassist' command not found${NC}"
    echo "You may need to:"
    echo "  1. Add ~/.cargo/bin to your PATH"
    echo "  2. Restart your terminal"
    echo "  3. Run: source ~/.bashrc (or ~/.zshrc)"
fi