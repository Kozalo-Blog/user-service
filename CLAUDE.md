# CLAUDE.md

This file contains guidance for Claude Code when working in this repository.

## Project Overview

This is a user management microservice built in Rust that exposes both REST (Axum) and gRPC (Tonic) interfaces. The service handles user creation, retrieval, updates, and consent management with PostgreSQL as the backing store.

## Development Commands

```bash
# Build and run
cargo build
cargo run

# Testing
cargo test
cargo test -- --nocapture  # with output
cargo test test_name       # specific test

# Code quality
cargo check
cargo fmt
cargo clippy

# SQLx offline compilation
cargo sqlx prepare -- --lib
```

## Architecture

### Dual-Server Setup

The service runs two servers concurrently:
- **REST API** (Axum): Port 8080 - HTTP/JSON interface at `/api/rest/v1/*`
- **gRPC API** (Tonic): Port 8090 - Protocol buffer interface

See `src/main.rs` for concurrent server startup.

### Module Organization

```
src/
├── main.rs           - Server initialization and startup
├── env.rs            - Environment variable utilities
├── observability.rs  - Distributed tracing setup
├── dto/              - Data transfer objects and domain models
├── repo/             - Repository pattern (database abstraction)
│   ├── users.rs      - Users trait and PostgreSQL implementation
│   └── services.rs   - Services trait and PostgreSQL implementation
├── rest/             - REST API handlers and error handling
│   ├── service.rs    - Axum route handlers
│   └── error.rs      - RestErrorExt extension trait
└── grpc/             - gRPC API handlers
    ├── server.rs     - Tonic service implementation
    └── error.rs      - gRPC Status conversion utilities
```

### Repository Pattern

Database operations use trait-based abstraction:
- `Users` and `Services` traits define repository interfaces
- `UsersPostgres` and `ServicesPostgres` implement PostgreSQL-specific logic
- SQLx with compile-time query verification via `query_as!` macro
- Enables dependency injection and testing without database

### Proto Definitions

Protocol buffers managed as git submodule:
- Source: https://github.com/Kozalo-Blog/protos
- Build: `build.rs` compiles protos using `tonic-build`
- Update: `git submodule update --remote proto`

## Configuration

Environment variables (see `.env.example`):

```bash
# Database
DATABASE_URL=postgres://userservice:us4pwd@localhost:5432/userservicedb
DATABASE_MAX_CONNECTIONS=10

# Logging
RUST_LOG=info,user_service=debug

# OpenTelemetry (optional)
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
```

## Database

### Migrations

```bash
sqlx migrate add <name>  # Create migration
sqlx migrate run         # Apply migrations
```

### Offline Compilation

For CI/CD builds without database:
1. Run `cargo sqlx prepare -- --lib` with valid DATABASE_URL
2. Commit generated `.sqlx/` directory
3. Build with `SQLX_OFFLINE=true`

## Observability

### Distributed Tracing

OpenTelemetry instrumentation with OTLP export:
- Automatic span creation via `#[tracing::instrument]` attributes
- Structured logging with `tracing::*!` macros
- Trace context propagation across REST/gRPC boundaries
- Console output + OTLP export to Grafana/Jaeger

Configuration in `src/observability.rs`:
- Service name from `CARGO_PKG_NAME`
- EnvFilter applied first for efficiency
- Global tracer provider for automatic context propagation

### Metrics

- **Autometrics**: Function-level metrics at `/metrics`
- **Prometheus**: Custom metrics via `axum-prometheus`
- **Tracing**: Request-level distributed traces

## Error Handling

Extension traits for idiomatic error handling:
- `RestErrorExt` in `src/rest/error.rs`: `.log_route_error()`, `.log_route_warn()`
- `IntoStatusExt` in `src/grpc/error.rs`: `.into_status()`, `.into_invalid_argument()`

Replaces verbose `.map_err()` blocks with single-line calls.

## Code Style

- Use Result/Option combinators (`.inspect()`, `.and_then()`, `.transpose()`) over verbose match expressions
- Move logging to delegated functions when possible
- Prefer `env::get_value_or_default()` for optional environment variables
- Use `#[tracing::instrument]` for automatic span creation
- Keep functions focused and single-purpose
