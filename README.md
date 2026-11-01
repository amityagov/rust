# rust

Shared Rust building blocks for services: graceful shutdown, an HTTP server with bearer and
mutual-TLS authentication and a uniform JSON error format, Postgres pools and distributed locks,
console logging, and a currency type that works in JSON and in SQL.

The repository is a Cargo workspace of small crates. The `lib` crate re-exports all of them behind
feature flags, so a service depends on `lib` and switches on only what it needs.

- [Quick start](#quick-start)
- [Feature flags](#feature-flags)
- [`signal`: graceful shutdown](#signal-graceful-shutdown)
- [`server`: HTTP server, authentication, errors](#server-http-server-authentication-errors)
- [`database`: Postgres pool](#database-postgres-pool)
- [`locks`: distributed locks](#locks-distributed-locks)
- [`logging`: console logging](#logging-console-logging)
- [`currency`: fiat and crypto currencies](#currency-fiat-and-crypto-currencies)
- [Versioning](#versioning)
- [Development](#development)

## Quick start

Add the crate as a git dependency, pinned to a release tag, with the features you need:

```toml
[dependencies]
lib = { git = "https://github.com/amityagov/rust.git", tag = "v0.5.0", features = ["server", "signal", "logging"] }
```

A service with logging and graceful shutdown:

```rust
use axum::{Router, routing::get};
use lib::logging::{LogLevel, init_tracing};
use lib::server::run_server;
use lib::signal;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing(LogLevel::Info)?;

    let (tx, rx) = signal::shutdown_signal();
    tokio::spawn(signal::subscribe_shutdown_signal(tx));

    let app = Router::new().route("/", get(|| async { "ok" }));
    run_server("example", app, "0.0.0.0:8080", rx).await
}
```

Requires Rust 1.85 or newer (edition 2024).

## Feature flags

Everything in `lib` is optional; `default` is empty.

| Feature | Enables |
| --- | --- |
| `signal` | [`lib::signal`](#signal-graceful-shutdown) |
| `server` | [`lib::server`](#server-http-server-authentication-errors); use it with `signal`, which provides its shutdown latch |
| `tls-server` | `server` plus `lib::server::tls` and `run_tls_server` (mutual TLS) |
| `database` | [`lib::database`](#database-postgres-pool) |
| `locks` | [`lib::locks`](#locks-distributed-locks) |
| `logging` | [`lib::logging`](#logging-console-logging) |
| `currency` | [`lib::currency`](#currency-fiat-and-crypto-currencies) |
| `currency-sqlx` | `currency` plus `sqlx` encoding and decoding of currencies (alias: `currency-full`) |
| `full` | `logging`, `server`, `signal`, `locks`, `database`, `currency`; not `tls-server` or `currency-sqlx` |

The member crates can also be used on their own by path or by git, without `lib`.

## `signal`: graceful shutdown

A shutdown latch shared by every task that must stop when the process is asked to.

- `shutdown_signal()` returns `(Sender<()>, ShutdownReceiver)`. Closing the sender requests shutdown.
- `subscribe_shutdown_signal(tx)` waits for Ctrl+C or SIGTERM (Ctrl+C only off Unix), then drops the
  sender. It panics if the operating system refuses to install a handler.
- `wait_shutdown_signal(rx)` resolves once the sender is gone. `ShutdownReceiver` is cloneable, so each
  task gets its own copy.

```rust
let (tx, rx) = lib::signal::shutdown_signal();
tokio::spawn(lib::signal::subscribe_shutdown_signal(tx));

let worker_rx = rx.clone();
tokio::spawn(async move {
    lib::signal::wait_shutdown_signal(worker_rx).await;
    // flush and stop
});
```

## `server`: HTTP server, authentication, errors

Built on [axum](https://docs.rs/axum) 0.8.

### Running a server

- `run_server(name, router, addr, shutdown)` binds `addr`, logs it, serves the router and finishes
  gracefully when the shutdown latch fires. Open connections get `DEFAULT_SHUTDOWN_GRACE` (30 s) to
  finish; then the server stops anyway, so one stuck request cannot keep the process alive.
  `run_server_with_grace` takes the period as an argument.
- `run_tls_server(name, router, addr, tls_config, shutdown)` does the same over mutual TLS
  (feature `tls-server`).

### Errors: `ApplicationError`

Every error renders as JSON `{"code": "...", "message": "..."}`. `code` is a stable snake_case
identifier that clients can branch on; `message` is human-readable.

| Constructor | Status | Code |
| --- | --- | --- |
| `ApplicationError::NotFound` | 404 | `not_found` |
| `ApplicationError::Unauthorized` | 401 | `unauthorized` |
| `ApplicationError::Forbidden` | 403 | `forbidden` |
| `ApplicationError::InternalServerError` (or `internal()`) | 500 | `internal_error` |
| `ApplicationError::bad_request(code, message)` | 400 | yours |
| `ApplicationError::unauthorized(code, message)` | 401 | yours |
| `ApplicationError::forbidden(code, message)` | 403 | yours |
| `ApplicationError::not_found(code, message)` | 404 | yours |
| `ApplicationError::conflict(code, message)` | 409 | yours |
| `ApplicationError::new(status, code, message)` | any | yours |

The unit variants keep their fixed messages. Log the cause of an internal failure and return
`internal()`, so database and library errors never reach the client.

```rust
use axum::Json;
use lib::server::error::ApplicationError;

async fn create_wallet() -> Result<Json<&'static str>, ApplicationError> {
    let has_keys = false;
    if !has_keys {
        return Err(ApplicationError::conflict(
            "workspace_keys_missing",
            "The workspace has no signing keys yet.",
        ));
    }
    Ok(Json("created"))
}
```

`ApplicationError` also exposes `status()`, `code()` and `message()`, and implements `Display` and
`std::error::Error`.

### Bearer authentication

`Authentication::bearer(audience, issuer, public_key_pem_base64)` verifies ES256 JWTs. The key is an
EC public key in PEM form encoded as URL-safe base64. A token is accepted only if its signature,
`exp`, `aud` and `iss` are valid and it carries the `sub` and `scope` claims. Anything else is
`401 Unauthorized`.

Install the `Authentication` as an `Extension`; handlers then take an `AuthenticatedUser`
(`id` from `sub`, `role`). Bearer authentication never sets `role`, so `require_role` from the
`RoleContainer` trait answers `403 Forbidden` until the application assigns one.

```rust
use axum::{Extension, Router, routing::get};
use lib::server::auth::{AuthenticatedUser, Authentication, RoleContainer};
use lib::server::error::ApplicationError;

async fn me(user: AuthenticatedUser) -> String {
    user.id
}

async fn admin_only(user: AuthenticatedUser) -> Result<&'static str, ApplicationError> {
    user.require_role("admin")?;
    Ok("welcome")
}

fn app(public_key_pem_base64: &str) -> anyhow::Result<Router> {
    let auth = Authentication::bearer("my-api", "https://issuer.example", public_key_pem_base64)?;
    Ok(Router::new()
        .route("/me", get(me))
        .route("/admin", get(admin_only))
        .layer(Extension(auth)))
}
```

### Mutual TLS (feature `tls-server`)

`lib::server::tls::tls()` builds a `RustlsConfig` for a server that requires client certificates.
The server certificate chain, its private key and the CA that signs client certificates are all
required. Each is base64-encoded PEM, given directly or read from an environment variable;
whitespace in the base64 is ignored. Keys may be PKCS#8, PKCS#1 or SEC1.

```rust
use lib::server::{run_tls_server, tls};

async fn internal(rx: lib::signal::ShutdownReceiver) -> anyhow::Result<()> {
    let config = tls::tls()
        .server_cert_from_env("SERVER_CERT")
        .server_key_from_env("SERVER_KEY")
        .root_ca_from_env("ROOT_CA")
        .build()?;

    let app = axum::Router::new().route("/", axum::routing::get(|| async { "ok" }));
    run_tls_server("internal", app, "0.0.0.0:8443".parse()?, config, rx).await
}
```

`lib::server::tls::ca` also loads a root store on its own (`load_root_ca`, which reads `ROOT_CA`,
`load_root_ca_from_env`, `load_root_ca_from_value`), for example to build a client that trusts the
same CA.

## `database`: Postgres pool

Connects an [sqlx](https://docs.rs/sqlx) `PgPool` from a configuration your application already has.

- `DatabaseConfig { connection_string, max_connections }` is `Deserialize`; the pool defaults to
  10 connections.
- Implement `DatabaseConfigSource` for your own config type and pass it to
  `create_database(&config)`, which returns `Database { pool }`.

```rust
use lib::database::{DatabaseConfig, DatabaseConfigSource, create_database};

struct Config {
    database: DatabaseConfig,
}

impl DatabaseConfigSource for Config {
    fn get_config(&self) -> &DatabaseConfig {
        &self.database
    }
}

async fn connect(config: &Config) -> anyhow::Result<sqlx::PgPool> {
    Ok(create_database(config).await?.pool)
}
```

## `locks`: distributed locks

Named locks with a time to live, stored in Postgres. A lock that is never released expires by itself,
so a crashed holder cannot block the others forever.

Run `lib::locks::SCHEMA_SQL` as one of your migrations: it creates the `public.locks` table and the two
functions this crate calls.

```sql
public.acquire_lock(lock_name text, lock_owner text, ttl_seconds bigint) returns boolean
public.release_lock(lock_name text, lock_owner text) returns boolean
```

- `DbLock::new(owner, name, ttl_seconds)` describes a lock; the TTL defaults to 60 seconds and must be
  positive. `owner` only labels who is asking (a service instance, say); it appears in the table to help
  debugging.
- `try_acquire(&pool)` returns `Some(LockGuard)` if the lock was free, `None` if someone holds it.
- `acquire_timeout(&pool, retry_delay, timeout)` waits, retrying with a random spread of ±50%, and
  returns `None` at the deadline. `acquire(&pool, retry_delay)` waits forever.
- `LockGuard::release()` releases the lock, waits for the database and says whether it was still ours.
- Dropping a `LockGuard` releases the lock in a background task instead. That is not guaranteed to
  finish (the process may exit first, and outside a Tokio runtime it is skipped with a warning); the
  TTL frees the lock in those cases.

Every acquisition is a separate holder, even for the same `DbLock` or the same `owner`: two callers
never hold one lock at once.

**Choose the TTL for the longest the work can take.** Nothing renews a lock, so if the work outlives the
TTL the lock expires, another holder gets in, and the first one is not told. Do not cancel `acquire`
(for example in a `select!`): if it is cancelled while the database grants the lock, the lock stays
taken until the TTL runs out. Use `acquire_timeout`.

```rust
use std::time::Duration;
use lib::locks::DbLock;

async fn nightly_job(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    let lock = DbLock::new("worker-1", "nightly-job", Some(120))?;

    if let Some(guard) = lock.try_acquire(pool).await? {
        // Runs on one worker at a time.
        guard.release().await?;
    }

    let timeout = Duration::from_secs(30);
    if let Some(_guard) = lock
        .acquire_timeout(pool, Duration::from_secs(1), timeout)
        .await?
    {
        // The lock is released when the guard is dropped.
    }
    Ok(())
}
```

## `logging`: console logging

`init_tracing(level)` installs a global `tracing` subscriber that prints events at `level` and above.
`LogLevel` is `Trace`, `Debug`, `Info`, `Warn` or `Error` and converts into a `LevelFilter`. Colors
are on in debug builds only. It returns an error, instead of panicking, if a subscriber is already
installed.

## `currency`: fiat and crypto currencies

One `Currency` type, written as its code everywhere.

- `FiatCurrency` is an ISO 4217 currency (`iso_currency::Currency`), for example `USD`.
- `CryptoCurrency` is `UsdcSol` (`USDC_SOL`), `Btc` (`BTC`) or `Eth` (`ETH`).
- `Currency` is `Fiat(..)` or `Crypto(..)`. It is `Copy`, `Eq` and `Hash`, and offers `all()`,
  `all_fiat()`, `all_crypto()`, `is_fiat()`, `is_crypto()` and `From` conversions from both kinds.
- The text form is the same in `Display`, `FromStr` (fiat codes first, then crypto codes; failure
  is `UnknownCurrency`) and serde (a JSON string).
- With the `sqlx` feature (`currency-sqlx` in `lib`) `Currency` and `CryptoCurrency` are stored as
  text columns holding the code, for any sqlx database.

```rust
use lib::currency::{CryptoCurrency, Currency};

let usd: Currency = "USD".parse()?;
let usdc = Currency::crypto(CryptoCurrency::UsdcSol);

assert!(usd.is_fiat() && usdc.is_crypto());
assert_eq!(serde_json::to_string(&usdc)?, r#""USDC_SOL""#);
```

## Versioning

Changes are listed in [CHANGELOG.md](CHANGELOG.md).

All crates share one version, which is also the git tag (`v0.5.0`). Depend on a tag, not on a
branch, so a build does not change when `main` moves. The crates are `0.x`: expect breaking changes
between minor versions.

## Development

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo test --workspace
cargo test -p server --features tls-server
cargo check -p lib --features "full,tls-server,currency-sqlx"
```

## License

MIT, see [LICENSE](LICENSE).
