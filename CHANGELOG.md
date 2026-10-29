# Changelog

All notable changes are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/),
and all crates share one version, which is also the git tag. While the version is `0.x`, a minor release
may contain breaking changes.

## [Unreleased]

### Added

- CI: format, clippy, tests (including `tls-server`) and a feature check of `lib` on every push and pull request.

### Changed

- The bearer-auth tests generate a P-256 key pair at run time instead of embedding a private key.

## [0.3.0] - 2026-10-29

### Added

- `README.md` documenting every crate, feature and example.
- `currency`: `FromStr` for `Currency` and `CryptoCurrency`, an `UnknownCurrency` error, `Copy`, `Eq` and
  `Hash`; serde and `Display` use the same text form (`USD`, `USDC_SOL`, `BTC`, `ETH`).
- `logging`: `From<LogLevel> for LevelFilter`.
- `server`: tests for bearer authentication and the TLS source handling.

### Changed

- **Breaking:** `CryptoCurrency::ETH` is now `CryptoCurrency::Eth`; its text form is still `ETH`.
- **Breaking:** `logging::init_tracing` returns an error, instead of panicking, when a subscriber is
  already installed.
- `locks`: both SQL functions are schema-qualified (`public.acquire_lock`, `public.release_lock`), and
  dropping a `LockGuard` outside a Tokio runtime logs a warning instead of panicking; the TTL then
  releases the lock.
- `server`: simpler bearer authentication; the TLS certificate, key and CA sources share one `Source`
  type.
- `signal`: documented, with a test.

### Fixed

- The `currency-sqlx` feature of `lib` did not build.

## [0.2.0] - 2026-10-29

### Added

- `server::error::ApplicationError` can carry a custom status, code and message, with the constructors
  `new`, `bad_request`, `unauthorized`, `forbidden`, `not_found`, `conflict` and `internal`. Every error
  renders as JSON `{"code", "message"}`.

## Earlier

Versions before 0.2.0 were not tagged. The history starts with the `database`, `locks` and `currency`
crates (October 2025) and adds JWT bearer authentication, the feature-based `lib` crate (December 2025)
and the mutual-TLS server (April 2026).

[Unreleased]: https://github.com/amityagov/rust/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/amityagov/rust/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/amityagov/rust/releases/tag/v0.2.0
