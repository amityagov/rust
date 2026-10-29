//! Distributed locks kept in Postgres.
//!
//! A lock is a row managed by two SQL functions that the application schema must provide:
//!
//! - `public.acquire_lock(name text, owner text, ttl_seconds bigint) returns boolean`
//! - `public.release_lock(name text, owner text) returns boolean`
//!
//! A lock expires on its own after its TTL, so a crashed owner cannot block others forever.

use std::time::Duration;

use sqlx::PgPool;

/// Lock lifetime used when none is given.
const DEFAULT_TTL_SECONDS: i64 = 60;

/// A named lock that one owner at a time can hold.
#[derive(Clone)]
pub struct DbLock {
    owner: String,
    name: String,
    ttl_seconds: i64,
}

/// Proof that the lock is held. Dropping it releases the lock in the background.
pub struct LockGuard {
    lock: DbLock,
    pool: PgPool,
}

impl DbLock {
    /// Describes the lock `name` for `owner`; `ttl_seconds` defaults to 60.
    pub fn new(owner: &str, name: &str, ttl_seconds: Option<i64>) -> Self {
        Self {
            owner: owner.to_owned(),
            name: name.to_owned(),
            ttl_seconds: ttl_seconds.unwrap_or(DEFAULT_TTL_SECONDS),
        }
    }

    /// Takes the lock if it is free (or expired); `None` if someone else holds it.
    pub async fn try_acquire(&self, pool: &PgPool) -> anyhow::Result<Option<LockGuard>> {
        let acquired: bool = sqlx::query_scalar("SELECT public.acquire_lock($1, $2, $3)")
            .bind(&self.name)
            .bind(&self.owner)
            .bind(self.ttl_seconds)
            .fetch_one(pool)
            .await?;

        Ok(acquired.then(|| LockGuard {
            lock: self.clone(),
            pool: pool.clone(),
        }))
    }

    /// Waits for the lock, retrying every `retry_delay`.
    pub async fn acquire(&self, pool: &PgPool, retry_delay: Duration) -> anyhow::Result<LockGuard> {
        loop {
            if let Some(guard) = self.try_acquire(pool).await? {
                tracing::trace!("Lock '{}' acquired by {}", self.name, self.owner);
                return Ok(guard);
            }
            tracing::trace!("Lock '{}' busy, retrying...", self.name);
            tokio::time::sleep(retry_delay).await;
        }
    }

    /// Releases the lock; `false` if this owner did not hold it.
    pub async fn release(&self, pool: &PgPool) -> anyhow::Result<bool> {
        let released: bool = sqlx::query_scalar("SELECT public.release_lock($1, $2)")
            .bind(&self.name)
            .bind(&self.owner)
            .fetch_one(pool)
            .await?;
        Ok(released)
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            tracing::warn!(
                "Lock '{}' not released on drop: no Tokio runtime; it expires with its TTL",
                self.lock.name
            );
            return;
        };

        let pool = self.pool.clone();
        let lock = self.lock.clone();
        runtime.spawn(async move {
            tracing::trace!("Releasing lock '{}'", lock.name);
            match lock.release(&pool).await {
                Ok(_) => tracing::trace!("Lock '{}' released", lock.name),
                Err(e) => tracing::error!("Failed to release lock '{}': {}", lock.name, e),
            }
        });
    }
}
