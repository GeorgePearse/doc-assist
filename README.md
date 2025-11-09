# doc-assist

A high-performance Rust CLI tool for automatically generating comprehensive documentation for codebases using Large Language Models (LLMs).

## Features

- **Multi-language support**: Analyzes Rust, Python, JavaScript, TypeScript, Go, Java, C++, and more
- **Intelligent analysis**: Parses AST to understand code structure, APIs, and dependencies
- **Flexible depth levels**: From quick overviews to exhaustive documentation
- **Resume capability**: Continue from where you left off if interrupted
- **Rate limiting**: Respects API rate limits automatically
- **Cost estimation**: Shows estimated costs before generation
- **Multiple LLM providers**: Supports OpenAI and Anthropic models

## Installation

### Quick Install (Using Cargo)

```bash
# Install directly from GitHub
cargo install --git https://github.com/GeorgePearse/doc-assist

# The binary will be installed as 'docassist' in your cargo bin directory
# Usually ~/.cargo/bin/docassist
```

### Install from Source

```bash
# Clone the repository
git clone https://github.com/GeorgePearse/doc-assist.git
cd doc-assist

# Install using cargo (recommended)
cargo install --path .

# Or build and install manually
cargo build --release
sudo cp target/release/docassist /usr/local/bin/

# Or add to PATH without sudo
mkdir -p ~/.local/bin
cp target/release/docassist ~/.local/bin/
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

### Verify Installation

```bash
# Check if docassist is available
docassist --version

# Get help
docassist --help
```

## Usage

### Basic Usage

```bash
# Set your API key (OpenAI or Anthropic)
export OPENAI_API_KEY=your-api-key
# or
export ANTHROPIC_API_KEY=your-api-key

# Generate documentation for current directory
docassist

# Generate documentation for a specific path
docassist /path/to/project

# Use a specific model
docassist --model claude-3-5-sonnet-20241022
docassist --model gpt-4
```

### Documentation Depth Levels

```bash
# Quick overview (20 queries)
docassist --depth quick

# Standard documentation (60 queries) - default
docassist --depth standard

# Comprehensive documentation (100+ queries)
docassist --depth comprehensive

# Continuous mode (keeps going until stopped)
docassist --depth continuous

# Custom query count
docassist --queries 50
```

### Advanced Options

```bash
# Dry run - see query plan without executing
docassist --dry-run

# Resume from previous run
docassist --resume

# Force regeneration (ignore cache)
docassist --force

# Custom output directory
docassist --output ./my-docs

# Rate limiting
docassist --rate-limit 30

# Include/exclude patterns
docassist --include "src/**/*.rs" --exclude "tests/**"

# Verbose output
docassist --verbose
```

## Output Structure

The tool generates a comprehensive documentation structure:

```
docs/
├── README.md                 # Main documentation overview
├── ARCHITECTURE.md          # System architecture
├── SUMMARY.md              # Table of contents
├── api/
│   └── README.md           # API reference
├── modules/
│   ├── README.md           # Module index
│   └── [module].md         # Individual module docs
├── guides/
│   ├── getting-started.md
│   ├── installation.md
│   └── ...
├── examples/
│   └── ...                 # Code examples
└── .metadata.json          # Generation metadata
```

## How It Works

1. **Analysis Phase**: Scans the codebase, identifies languages, modules, and APIs
2. **Planning Phase**: Creates an optimized query plan based on the codebase structure
3. **Generation Phase**: Executes LLM queries with intelligent context management
4. **Assembly Phase**: Organizes responses into structured documentation

## Configuration

### Environment Variables

- `OPENAI_API_KEY`: OpenAI API key
- `ANTHROPIC_API_KEY`: Anthropic API key
- `DOCASSIST_MODEL`: Default model to use

### Supported Models

**OpenAI:**
- `gpt-4`
- `gpt-4-turbo`
- `gpt-3.5-turbo`

**Anthropic:**
- `claude-3-5-sonnet-20241022`
- `claude-3-opus-20240229`
- `claude-3-sonnet-20240229`

## Development

### Building from source

```bash
cargo build
```

### Running tests

```bash
cargo test
```

### Running with verbose logging

```bash
RUST_LOG=debug cargo run -- --verbose
```

## Architecture

doc-assist is built with a modular architecture:

- **Analyzer**: Parses source code and extracts structure
- **Planner**: Creates optimized query plans
- **Generator**: Manages LLM interactions with rate limiting
- **Assembler**: Organizes responses into documentation
- **State Manager**: Handles persistence and resume capability
- **Context Manager**: Optimizes context window usage

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT License - see LICENSE file for details

## Acknowledgments

Built with:
- [litellm-rs](https://crates.io/crates/litellm-rs) - Multi-provider LLM client
- [tree-sitter](https://tree-sitter.github.io/tree-sitter/) - Code parsing
- [tokio](https://tokio.rs/) - Async runtime
- [clap](https://clap.rs/) - CLI framework