FROM rust:alpine as builder
WORKDIR /build

RUN apk update && apk add --no-cache musl-dev protobuf-dev

# Create an unprivileged user
ENV USER=appuser
ENV UID=10001
RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"

COPY src/ src/
COPY migrations/ migrations/
COPY proto/ proto/
COPY .sqlx/ .sqlx/
COPY Cargo.* build.rs ./

ENV RUSTFLAGS='-C target-feature=-crt-static'
RUN cargo build --release && mv target/release/user-service /user-service

FROM alpine
RUN apk update && apk add --no-cache libgcc
COPY --from=builder /user-service /usr/local/bin/
# Import the user and group files from the builder
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group
# Use the unprivileged user
USER appuser:appuser

EXPOSE 8080
EXPOSE 8090
ARG RUST_LOG
ARG DATABASE_URL
ARG DATABASE_MAX_CONNECTIONS
ENTRYPOINT [ "/usr/local/bin/user-service" ]

LABEL org.opencontainers.image.source=https://github.com/Kozalo-Blog/user-service
LABEL org.opencontainers.image.description="A microservice to store and manage the information about users in the ecosystem and their subscriptions and settings"
LABEL org.opencontainers.image.licenses=MIT
