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

## Implementation Notes

### Data Schema (Qdrant)

```json
{
  "package": "numpy",
  "version": "1.24.0",
  "module": "numpy.array",
  "object_type": "function",
  "signature": "array(object, dtype=None, ...)",
  "docstring": "...",
  "content_hash": "sha256...",
  "source": "pypi|readthedocs|generated",
  "vector": [...]
}
```

### Multi-Language Support Roadmap

Start with **Python** (80% of use case), then expand:

- **Python**: AST parsing with `ast` module (subprocess or `rustpython-parser`)
- **JavaScript/TypeScript**: `swc_ecma_parser` (Rust native) or TypeScript compiler API
- **Rust**: `syn` crate for AST parsing
- **Go**: Call `go doc` command, parse output
- **Java**: JavaParser library

### Cost & Performance Estimates

**Storage (Qdrant)**:
- ~1KB per function/class doc (compressed)
- Top 1000 packages × 100 functions avg × 5 versions = 500K entries
- ~500MB vector data
- Qdrant Cloud free tier: 1GB (sufficient for MVP)

**Embedding**:
- Use sentence-transformers (local, open-source)
- Models: `all-MiniLM-L6-v2` (384 dims, fast) or `all-mpnet-base-v2` (768 dims, better quality)
- Embed ~500K docs: ~2-4 hours CPU, ~30 min GPU

**LLM Generation (Tier 3)**:
- Assume 10% of queries need generation initially
- Cache hit rate improves over time (90%+ after month)
- Cost: minimal with caching strategy

### MVP Roadmap

**Week 1**: Core indexing
- Scrape top 50 Python packages
- Parse docstrings only (no narrative docs)
- Embed with sentence-transformers
- Store in local Qdrant (Docker)

**Week 2**: API layer
- Build REST API with Axum
- Semantic search endpoint
- Version filtering

**Week 3**: Intelligence layer
- Add Tier 3 (LLM generation) for sparse docs
- Implement caching

**Week 4**: Deployment
- Deploy to Fly.io
- Migrate to Qdrant Cloud
- Set up continuous scraping pipeline

### Alternative Approaches Considered

1. **PostgreSQL + pgvector**: More versatile for mixed data, but chose Qdrant for Rust-native performance
2. **Upstash Vector**: Serverless and fitting (inspired by Context7), but Qdrant offers better self-hosting
3. **Diff-based storage**: More complex than content-addressed hashing for version management
4. **GitHub mining**: Too storage-intensive compared to PyPI API approach
5. **Push-based webhooks**: More complex than pull-based polling for incremental updates

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
