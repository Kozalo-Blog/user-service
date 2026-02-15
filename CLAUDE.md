# CLAUDE.md

This file contains guidance for Claude Code when working in this repository.

## Project Overview

This is a user management microservice built in Rust that exposes both REST (Axum) and gRPC (Tonic) interfaces. The service handles user creation, retrieval, updates, and consent management with PostgreSQL as the backing store.

## Development Commands

```bash
# Build the project
cargo build

# Run tests
cargo test

# Run the service locally
cargo run

# Check code without building
cargo check

# Format code
cargo fmt

# Run linter
cargo clippy

# Prepare SQLx queries for offline compilation
cargo sqlx prepare -- --lib
```

## Architecture

### Dual-Server Setup

The service runs two servers simultaneously:
- **REST API** (Axum): Port 8080 - HTTP/JSON interface at `/api/v1/users`
- **gRPC API** (Tonic): Port 8090 - Protocol buffer interface defined in proto submodule

See `src/main.rs` for the concurrent server startup using `tokio::try_join!`.

### Repository Pattern

Database operations are abstracted through traits:
- `UserRepository` trait in `src/db/repository.rs` defines the interface
- `PostgresUserRepository` in `src/db/postgres.rs` implements PostgreSQL-specific logic
- Uses SQLx with compile-time query verification via `query_as!` macro

This pattern enables dependency injection and makes the code testable without requiring a real database.

### Handler Organization

- `src/handlers/rest.rs`: Axum HTTP handlers for REST endpoints
- `src/handlers/grpc.rs`: Tonic gRPC service implementation
- Both handlers delegate to the same `UserRepository` trait, ensuring consistent business logic

### Proto Definitions

Protocol buffers are managed in a separate submodule at `proto/`:
- Source: https://github.com/Kozalo-Blog/protos
- Location: `proto/user-service/` contains the proto definitions
- Build: `build.rs` compiles protos using `tonic-build`
- Generated code: Available via `proto::*` modules

To update proto definitions:
```bash
git submodule update --remote proto
```

## Database

### Configuration

Database connection is configured via environment variable:
```
DATABASE_URL=postgres://user:password@localhost/dbname
```

### Migrations

Uses SQLx migrations in the `migrations/` directory:
```bash
# Create new migration
sqlx migrate add <name>

# Run migrations
sqlx migrate run
```

### Offline Query Compilation

SQLx verifies SQL queries at compile time. To enable compilation without a running database:

1. Ensure `DATABASE_URL` points to a valid database with up-to-date schema
2. Run: `cargo sqlx prepare -- --lib`
3. Commit the generated `.sqlx/` directory
4. CI/CD can now build without database access using `SQLX_OFFLINE=true`

## Testing

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_name
```

Tests use the repository trait to enable mocking without requiring a database connection.

## Docker Development

The project includes a Dockerfile for containerized deployment. The CI workflow builds and tests in a containerized environment to ensure consistency.

## CI/CD

GitHub Actions workflow (`.github/workflows/rust.yml`):
- Runs on push and PR to main
- Checks formatting with `cargo fmt`
- Runs linter with `cargo clippy`
- Executes test suite
- Verifies the build

## Graceful Shutdown

The service implements graceful shutdown for both servers, listening for SIGTERM/SIGINT signals to cleanly close connections before terminating.

## Distributed Tracing

The service implements comprehensive distributed tracing with OpenTelemetry OTLP export to VictoriaStack/Grafana.

### Configuration

Tracing is configured via environment variables in `.env` or `.env.example`:

```bash
# OpenTelemetry OTLP endpoint (default: http://localhost:4317)
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317

# Log level filtering (default: info,user_service=debug)
RUST_LOG=info,user_service=debug
```

### Implementation Details

**Instrumented Layers:**
- All REST handlers (8 functions in `src/rest/service.rs`)
- All gRPC handlers (4 functions in `src/grpc/server.rs`)
- All repository methods (7 functions in `src/repo/users.rs` and `src/repo/services.rs`)

**Features:**
- Automatic span creation with `#[tracing::instrument]` attributes
- Structured logging with context (tracing::info!, debug!, warn!, error!)
- Error recording in spans with full error details
- Span timing for performance analysis
- Request-level visibility into dual-server architecture

**Trace Context Propagation:**
- REST API: Automatic `traceparent` header propagation via `OtelAxumLayer`
- gRPC: Automatic context propagation through global tracing subscriber
- Cross-service tracing support for distributed request flows

**Output Destinations:**
- Console: Structured formatted output with timestamps, targets, and line numbers
- OTLP Exporter: Batched span export to VictoriaStack/Grafana for visualization

**Initialization:**
- Tracing starts in `main.rs` via `observability::init_tracing()`
- Graceful shutdown flushes pending spans before service termination
- Located in `src/observability.rs`

### Testing Locally

1. **Set up OpenTelemetry collector or VictoriaStack:**
   ```bash
   # Example with Docker - Jaeger all-in-one
   docker run -d --name jaeger \
     -p 4317:4317 \
     -p 16686:16686 \
     jaegertracing/all-in-one:latest
   ```

2. **Configure environment:**
   ```bash
   export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
   export RUST_LOG=debug
   ```

3. **Run the service:**
   ```bash
   cargo run
   ```

4. **Generate traces:**
   ```bash
   # REST request
   curl http://localhost:8080/api/rest/v1/user/1

   # gRPC request (requires grpcurl)
   grpcurl -plaintext -d '{"id": 1, "by_external_id": false}' \
     localhost:8090 user_service.UserService/Get
   ```

5. **View traces:**
   - Open Jaeger UI: http://localhost:16686
   - Or VictoriaStack/Grafana interface
   - Search for service: "user-service"
   - View trace hierarchies showing handler → repository spans

### Metrics Integration

Tracing complements existing observability:
- **Autometrics**: Function-level metrics at `/metrics` endpoint (maintained)
- **Prometheus**: Custom metrics via `axum-prometheus` (maintained)
- **Tracing**: Request-level distributed traces with timing and context (new)

Together, these provide comprehensive observability: metrics for aggregates, traces for request flows.
