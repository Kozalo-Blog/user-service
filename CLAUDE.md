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
