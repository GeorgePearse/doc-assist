# doc-assist

A documentation assistant tool.

## Inspiration

This project was inspired by [upstash/context7#824](https://github.com/upstash/context7/issues/824).

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
