user-service
============

[![CI Build](https://github.com/Kozalo-Blog/user-service/actions/workflows/ci-build.yaml/badge.svg?branch=main&event=push)](https://github.com/Kozalo-Blog/user-service/actions/workflows/ci-build.yaml)

This is a simple CRUD application written in Rust which is supposed to be a centralized storage of users' information
and options throughout the whole ecosystem of my projects (like Telegram bots in [kozalosev](https://github.com/kozalosev)'s
repositories).

Features
--------
* REST endpoints (using [axum](https://github.com/tokio-rs/axum));
* ~~gRPC services (using [tonic](https://github.com/hyperium/tonic))~~ _(not available yet but this work is in progress)_;
* Prometheus-like metrics;
* [sqlx](https://github.com/launchbadge/sqlx) connection pool for PostgreSQL and macros to check queries statically at compile time. 

Technical stuff
---------------

### Requirements to run
* PostgreSQL;
* _\[optional]_ Docker (it makes the configuration a lot easier);
* a frontal proxy server with TLS support ([nginx-proxy](https://github.com/nginx-proxy/nginx-proxy), for example).

### How to rebuild .sqlx queries?
_(to build the application without a running RDBMS)_

```shell
cargo sqlx prepare -- --tests
```
