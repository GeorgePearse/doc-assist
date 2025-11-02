# doc-assist

A documentation assistant tool.

## Inspiration

This project was inspired by [upstash/context7#824](https://github.com/upstash/context7/issues/824).

## Tech Stack

- **Web Framework**: Axum (built on Tokio)
  - Type-safe, fast async framework
  - Excellent ergonomics and tower middleware ecosystem
  - Native async/await support

## Data Storage

Using **Qdrant** for vector storage:
- Rust-native vector database
- Great Rust client support
- Can run locally with Docker or use Qdrant Cloud
- Excellent for semantic search over documentation

## Documentation Scraping Architecture

### 3-Tier Hybrid Approach

#### Tier 1: Static Pre-Indexed (Core)
- **Source**: PyPI packages via JSON API (`https://pypi.org/pypi/{package}/json`)
- **Method**: Download source distributions (`.tar.gz`), parse with AST
- **Extract**: Docstrings, type hints, function signatures, class hierarchies
- **Embed**: sentence-transformers (local, open-source models)
- **Storage**: Qdrant with content-addressed deduplication (SHA256 hashing)

#### Tier 2: Scraped Documentation (Enrichment)
- **Source**: ReadTheDocs, official documentation sites
- **Content**: Narrative docs, tutorials, examples
- **Method**: Headless scraping for popular packages
- **Frequency**: Weekly scrapes

#### Tier 3: On-Demand Generation (Fallback)
- **When**: Sparse/missing docstrings detected
- **Method**: LLM generates explanations at query time (DeepWiki-style)
- **Process**: Parse code → extract context → LLM explains → cache result
- **Caching**: Store generated content back to Qdrant with TTL

### Version Management

**Content-Addressed Storage**:
- Hash each docstring with SHA256
- Store unique content once, versions reference hashes
- Automatic deduplication (80-90% space savings)
- Efficient cross-version queries

### Pipeline

**Bootstrap** (one-time):
1. Fetch top 50-1000 PyPI packages by downloads
2. Download latest + all major versions (1.x, 2.x, etc.)
3. Parse, embed, and store

**Incremental Updates** (continuous):
1. Poll PyPI RSS feed every 6 hours
2. Download and index new versions only
3. Auto-deduplicate via content hash

**On-Demand** (user-triggered):
1. User searches for unindexed package
2. Queue for scraping or immediate index if small

## Deployment Options

### Easiest Options:

1. **Shuttle.rs** - Purpose-built for Rust web services
   - Zero config deployment
   - Built-in support for Tokio/Axum/Actix
   - Free tier available
   - Literally `cargo shuttle deploy`

2. **Fly.io** - Very popular for Rust apps
   - Good free tier (256MB RAM, 3 VMs)
   - Simple `flyctl deploy`
   - Great for long-running services
   - Excellent docs for Rust

3. **Railway** - Simple and developer-friendly
   - Auto-detects Rust projects
   - Good free tier ($5 credit/month)
   - Nice dashboard
