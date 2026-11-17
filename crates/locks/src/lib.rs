//! Distributed locks kept in Postgres.
//!
//! A lock is a row managed by two SQL functions that the application schema must provide:
//!
//! - `public.acquire_lock(name text, owner text, ttl_seconds bigint) returns boolean`
//! - `public.release_lock(name text, owner text) returns boolean`
//!
//! [`SCHEMA_SQL`] creates the table and both functions; run it as one of your migrations.
//!
//! A lock expires on its own after its TTL, so a crashed owner cannot block others forever. The
//! converse also holds: work that outlives the TTL is no longer protected, because nothing renews
//! the lock. Choose a TTL longer than the longest the protected work can take.
//!
//! Every successful acquisition is a separate holder, even for the same [`DbLock`] value or the
//! same `owner` string: two tasks, or two calls, never hold one lock at the same time.
//!
//! # Long work: a short TTL that is renewed
//!
//! A TTL sized for the longest work means a crashed owner blocks the lock for that long. Take the
//! lock with a short TTL instead and call [`LockGuard::keep_alive`]: a background task renews it
//! every third of the TTL, so a crash frees the lock within one TTL, and the work can run as long
//! as it needs. The work must watch [`LockGuard::lost`] (or [`LockGuard::is_lost`]): when the lock
//! could not be renewed before it ran out, another holder may already have it, and the work has to
//! stop.

use std::sync::Arc;
use std::time::{Duration, Instant};

use sqlx::PgPool;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use uuid::Uuid;

/// SQL that creates the `public.locks` table and the two functions this crate calls. It can be run
/// more than once.
pub const SCHEMA_SQL: &str = include_str!("../schema.sql");

/// Lock lifetime used when none is given.
const DEFAULT_TTL_SECONDS: i64 = 60;

/// A named lock that one holder at a time can take.
#[derive(Clone)]
pub struct DbLock {
    owner: String,
    name: String,
    ttl_seconds: i64,
}

/// Proof that the lock is held.
///
/// Call [`LockGuard::release`] to release it and learn whether that worked. Dropping the guard
/// releases it in the background instead, which is not guaranteed to finish (for example when the
/// process exits); the TTL then frees the lock.
pub struct LockGuard {
    name: String,
    holder: String,
    ttl_seconds: i64,
    pool: PgPool,
    armed: bool,
    /// Turns `true` once the lock is known to be lost; fed by the heartbeat.
    lost: Arc<watch::Sender<bool>>,
    heartbeat: Option<JoinHandle<()>>,
}

impl DbLock {
    /// Describes the lock `name` for `owner`; `ttl_seconds` defaults to 60 and must be positive.
    ///
    /// `owner` identifies who is asking, for example a service instance; it only shows up in the
    /// `locks` table to help whoever debugs a stuck lock.
    pub fn new(owner: &str, name: &str, ttl_seconds: Option<i64>) -> anyhow::Result<Self> {
        let ttl_seconds = ttl_seconds.unwrap_or(DEFAULT_TTL_SECONDS);
        anyhow::ensure!(
            ttl_seconds > 0,
            "lock '{name}': ttl_seconds must be positive, got {ttl_seconds}"
        );
        Ok(Self {
            owner: owner.to_owned(),
            name: name.to_owned(),
            ttl_seconds,
        })
    }

    /// Takes the lock if it is free (or expired); `None` if someone else holds it.
    pub async fn try_acquire(&self, pool: &PgPool) -> anyhow::Result<Option<LockGuard>> {
        // A fresh holder id per attempt: the SQL lets a holder re-acquire its own lock, so sharing
        // one id between callers would let them all in.
        let holder = format!("{}#{}", self.owner, Uuid::new_v4());
        let acquired: bool = sqlx::query_scalar("SELECT public.acquire_lock($1, $2, $3)")
            .bind(&self.name)
            .bind(&holder)
            .bind(self.ttl_seconds)
            .fetch_one(pool)
            .await?;

        Ok(acquired.then(|| LockGuard {
            name: self.name.clone(),
            holder,
            ttl_seconds: self.ttl_seconds,
            pool: pool.clone(),
            armed: true,
            lost: Arc::new(watch::channel(false).0),
            heartbeat: None,
        }))
    }

    /// Waits for the lock, retrying about every `retry_delay` (randomly spread by up to ±50% so
    /// that waiters do not hit the database in step).
    ///
    /// Waits forever; prefer [`DbLock::acquire_timeout`]. Do not cancel this future (for example
    /// by dropping it in a `select!`): if it is cancelled while the database is granting the lock,
    /// the lock stays taken until its TTL expires.
    pub async fn acquire(&self, pool: &PgPool, retry_delay: Duration) -> anyhow::Result<LockGuard> {
        loop {
            if let Some(guard) = self.try_acquire(pool).await? {
                tracing::trace!("Lock '{}' acquired by {}", self.name, self.owner);
                return Ok(guard);
            }
            tracing::trace!("Lock '{}' busy, retrying...", self.name);
            tokio::time::sleep(jitter(retry_delay)).await;
        }
    }

    /// Like [`DbLock::acquire`], but gives up after `timeout` and returns `None`.
    ///
    /// The deadline is checked between attempts, so the call can overrun it by at most one query.
    pub async fn acquire_timeout(
        &self,
        pool: &PgPool,
        retry_delay: Duration,
        timeout: Duration,
    ) -> anyhow::Result<Option<LockGuard>> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(guard) = self.try_acquire(pool).await? {
                tracing::trace!("Lock '{}' acquired by {}", self.name, self.owner);
                return Ok(Some(guard));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                tracing::trace!("Lock '{}' still busy at the deadline", self.name);
                return Ok(None);
            }
            tokio::time::sleep(jitter(retry_delay).min(remaining)).await;
        }
    }
}

impl LockGuard {
    /// Extends the lock by its TTL, counted from now. `false` means the lock is no longer ours: it
    /// ran out (even if nobody took it since, because work may have gone on unprotected in the
    /// gap) or another holder has it. An error leaves the lock as it was.
    pub async fn renew(&self) -> anyhow::Result<bool> {
        renew(&self.pool, &self.name, &self.holder, self.ttl_seconds).await
    }

    /// Renews the lock in the background, every third of the TTL, until the guard is released or
    /// dropped. Calling it again does nothing.
    ///
    /// When a renewal finds the lock lost, or none succeeded for a whole TTL (the database is not
    /// reachable, so it may have been taken), [`LockGuard::lost`] completes and the heartbeat
    /// stops. The work that the lock protects has to stop then.
    pub fn keep_alive(&mut self) {
        if self.heartbeat.is_some() {
            return;
        }
        let (pool, name, holder) = (self.pool.clone(), self.name.clone(), self.holder.clone());
        let (ttl_seconds, lost) = (self.ttl_seconds, Arc::clone(&self.lost));
        let ttl = Duration::from_secs(ttl_seconds.unsigned_abs());
        self.heartbeat = Some(tokio::spawn(async move {
            // The lock was taken just now. Times are measured from before each request, so the
            // estimate of when the lock runs out is never later than the database's.
            let mut last_renewed = Instant::now();
            loop {
                tokio::time::sleep(ttl / 3).await;
                let started = Instant::now();
                match renew(&pool, &name, &holder, ttl_seconds).await {
                    Ok(true) => last_renewed = started,
                    Ok(false) => {
                        tracing::error!("Lock '{name}' was lost: it ran out or has another holder");
                        break;
                    }
                    Err(e) if last_renewed.elapsed() >= ttl => {
                        tracing::error!(
                            "Lock '{name}' is presumed lost, no renewal for {ttl:?}: {e}"
                        );
                        break;
                    }
                    Err(e) => tracing::warn!("Could not renew lock '{name}', will try again: {e}"),
                }
            }
            lost.send_replace(true);
        }));
    }

    /// Whether the lock is known to be lost. Only meaningful after [`LockGuard::keep_alive`]:
    /// without a heartbeat nothing finds out.
    pub fn is_lost(&self) -> bool {
        *self.lost.borrow()
    }

    /// Completes when the lock is known to be lost (see [`LockGuard::keep_alive`]); never, for a
    /// guard that is not kept alive. Meant for a `select!` next to the protected work.
    pub async fn lost(&self) {
        let mut receiver = self.lost.subscribe();
        // `wait_for` returns at once when the value already matches, and errors only if the sender
        // is gone, which cannot happen while `self` holds it.
        let _ = receiver.wait_for(|lost| *lost).await;
    }

    /// Releases the lock and waits for the database; `false` if the lock was no longer ours
    /// (it had expired and someone else took it).
    ///
    /// On an error the lock stays held until its TTL expires.
    pub async fn release(mut self) -> anyhow::Result<bool> {
        // Disarm first: the guard is dropped when this function returns, and must not release twice.
        self.armed = false;
        // The heartbeat goes first: a renewal after the release must not find a lock to extend.
        if let Some(heartbeat) = self.heartbeat.take() {
            heartbeat.abort();
        }
        release(&self.pool, &self.name, &self.holder).await
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        if let Some(heartbeat) = self.heartbeat.take() {
            heartbeat.abort();
        }
        if !self.armed {
            return;
        }
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            tracing::warn!(
                "Lock '{}' not released on drop: no Tokio runtime; it expires with its TTL",
                self.name
            );
            return;
        };

        let pool = self.pool.clone();
        let name = self.name.clone();
        let holder = self.holder.clone();
        runtime.spawn(async move {
            tracing::trace!("Releasing lock '{name}'");
            match release(&pool, &name, &holder).await {
                Ok(_) => tracing::trace!("Lock '{name}' released"),
                Err(e) => tracing::error!("Failed to release lock '{name}': {e}"),
            }
        });
    }
}

/// Extends the lock only while it is ours and has not run out; never inserts a row, so a renewal
/// that is late (after a release, or after the lock ran out and was cleaned up) cannot bring the
/// lock back.
async fn renew(pool: &PgPool, name: &str, holder: &str, ttl_seconds: i64) -> anyhow::Result<bool> {
    let renewed = sqlx::query(
        "UPDATE public.locks
            SET expires_at = now() + make_interval(secs => $3)
          WHERE name = $1 AND owner_id = $2 AND expires_at > now()",
    )
    .bind(name)
    .bind(holder)
    .bind(ttl_seconds)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(renewed == 1)
}

async fn release(pool: &PgPool, name: &str, holder: &str) -> anyhow::Result<bool> {
    let released: bool = sqlx::query_scalar("SELECT public.release_lock($1, $2)")
        .bind(name)
        .bind(holder)
        .fetch_one(pool)
        .await?;
    Ok(released)
}

/// `delay` scaled by a random factor in `[0.5, 1.5)`.
fn jitter(delay: Duration) -> Duration {
    let fraction = (Uuid::new_v4().as_u128() % 1000) as f64 / 1000.0;
    delay.mul_f64(0.5 + fraction)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    /// Connects to `TEST_DATABASE_URL` (a scratch database) and installs the schema; `None` when
    /// the variable is unset, so the database tests are skipped on machines without Postgres.
    async fn pool() -> Option<PgPool> {
        let url = std::env::var("TEST_DATABASE_URL").ok()?;
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .unwrap();
        // The tests run in parallel and Postgres rejects concurrent `create or replace function`.
        static SCHEMA: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();
        SCHEMA
            .get_or_init(|| async {
                sqlx::raw_sql(SCHEMA_SQL).execute(&pool).await.unwrap();
            })
            .await;
        Some(pool)
    }

    /// A lock name no other test uses.
    fn unique_name() -> String {
        format!("test/{}", Uuid::new_v4())
    }

    #[test]
    fn rejects_a_non_positive_ttl() {
        assert!(DbLock::new("o", "n", Some(0)).is_err());
        assert!(DbLock::new("o", "n", Some(-5)).is_err());
        assert!(DbLock::new("o", "n", None).is_ok());
    }

    #[test]
    fn jitter_stays_within_half_to_one_and_a_half() {
        for _ in 0..1000 {
            let spread = jitter(Duration::from_millis(100));
            assert!(spread >= Duration::from_millis(50) && spread < Duration::from_millis(150));
        }
    }

    #[tokio::test]
    async fn two_holders_with_one_owner_exclude_each_other() {
        let Some(pool) = pool().await else { return };
        let lock = DbLock::new("same-owner", &unique_name(), None).unwrap();

        let first = lock.try_acquire(&pool).await.unwrap();
        let second = lock.try_acquire(&pool).await.unwrap();

        assert!(first.is_some());
        assert!(second.is_none(), "the same owner must not get in twice");
    }

    #[tokio::test]
    async fn release_frees_the_lock_and_reports_it() {
        let Some(pool) = pool().await else { return };
        let lock = DbLock::new("o", &unique_name(), None).unwrap();

        let guard = lock.try_acquire(&pool).await.unwrap().unwrap();
        assert!(guard.release().await.unwrap());

        assert!(lock.try_acquire(&pool).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn dropping_a_guard_releases_in_the_background() {
        let Some(pool) = pool().await else { return };
        let lock = DbLock::new("o", &unique_name(), None).unwrap();

        drop(lock.try_acquire(&pool).await.unwrap().unwrap());

        let guard = lock
            .acquire_timeout(&pool, Duration::from_millis(20), Duration::from_secs(5))
            .await
            .unwrap();
        assert!(guard.is_some());
    }

    #[tokio::test]
    async fn an_expired_lock_can_be_taken_over() {
        let Some(pool) = pool().await else { return };
        let lock = DbLock::new("o", &unique_name(), Some(1)).unwrap();

        let stale = lock.try_acquire(&pool).await.unwrap().unwrap();
        tokio::time::sleep(Duration::from_millis(1200)).await;
        let fresh = lock.try_acquire(&pool).await.unwrap();

        assert!(fresh.is_some());
        // The stale holder lost the lock, so its release must not free the new holder's.
        assert!(!stale.release().await.unwrap());
        assert!(
            lock.try_acquire(&pool).await.unwrap().is_none(),
            "the new holder still holds the lock"
        );
    }

    #[tokio::test]
    async fn acquire_timeout_gives_up_on_a_held_lock() {
        let Some(pool) = pool().await else { return };
        let lock = DbLock::new("o", &unique_name(), None).unwrap();
        let _held = lock.try_acquire(&pool).await.unwrap().unwrap();

        let started = Instant::now();
        let got = lock
            .acquire_timeout(&pool, Duration::from_millis(20), Duration::from_millis(300))
            .await
            .unwrap();

        assert!(got.is_none());
        assert!(started.elapsed() >= Duration::from_millis(300));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn only_one_of_many_concurrent_callers_wins() {
        let Some(pool) = pool().await else { return };
        let lock = DbLock::new("o", &unique_name(), None).unwrap();

        let attempts = (0..8).map(|_| {
            let (lock, pool) = (lock.clone(), pool.clone());
            tokio::spawn(async move { lock.try_acquire(&pool).await.unwrap() })
        });
        let mut winners = 0;
        let mut guards = Vec::new();
        for attempt in attempts {
            if let Some(guard) = attempt.await.unwrap() {
                winners += 1;
                guards.push(guard);
            }
        }

        assert_eq!(winners, 1);
    }

    #[tokio::test]
    async fn a_renewal_extends_the_lock_and_a_lost_lock_cannot_be_renewed() {
        let Some(pool) = pool().await else { return };
        let lock = DbLock::new("o", &unique_name(), Some(2)).unwrap();

        let guard = lock.try_acquire(&pool).await.unwrap().unwrap();
        tokio::time::sleep(Duration::from_millis(1200)).await;
        assert!(guard.renew().await.unwrap());
        // 1.2 s on, the original TTL would have run out; the renewal pushed it to 3.2 s.
        tokio::time::sleep(Duration::from_millis(1200)).await;
        assert!(lock.try_acquire(&pool).await.unwrap().is_none());

        // After it ran out and someone else took it, the old holder cannot renew.
        tokio::time::sleep(Duration::from_millis(2200)).await;
        let _new = lock.try_acquire(&pool).await.unwrap().unwrap();
        assert!(!guard.renew().await.unwrap());
    }

    #[tokio::test]
    async fn a_renewal_does_not_bring_back_a_released_lock() {
        let Some(pool) = pool().await else { return };
        let lock = DbLock::new("o", &unique_name(), None).unwrap();

        let guard = lock.try_acquire(&pool).await.unwrap().unwrap();
        let (name, holder) = (guard.name.clone(), guard.holder.clone());
        assert!(guard.release().await.unwrap());

        assert!(!renew(&pool, &name, &holder, 60).await.unwrap());
        assert!(lock.try_acquire(&pool).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn a_kept_alive_lock_outlives_its_ttl_and_frees_when_released() {
        let Some(pool) = pool().await else { return };
        let lock = DbLock::new("o", &unique_name(), Some(1)).unwrap();

        let mut guard = lock.try_acquire(&pool).await.unwrap().unwrap();
        guard.keep_alive();
        guard.keep_alive();
        tokio::time::sleep(Duration::from_millis(2500)).await;
        assert!(!guard.is_lost());
        assert!(
            lock.try_acquire(&pool).await.unwrap().is_none(),
            "still held after more than two TTLs"
        );

        assert!(guard.release().await.unwrap());
        assert!(lock.try_acquire(&pool).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn a_lock_that_is_taken_over_is_reported_lost() {
        let Some(pool) = pool().await else { return };
        let name = unique_name();
        let lock = DbLock::new("o", &name, Some(1)).unwrap();

        let mut guard = lock.try_acquire(&pool).await.unwrap().unwrap();
        guard.keep_alive();
        // Someone removes the row (an operator, a clean-up) and another holder takes the lock.
        sqlx::query("DELETE FROM public.locks WHERE name = $1")
            .bind(&name)
            .execute(&pool)
            .await
            .unwrap();
        let _other = lock.try_acquire(&pool).await.unwrap().unwrap();

        tokio::time::timeout(Duration::from_secs(3), guard.lost())
            .await
            .expect("the loss is reported within a TTL");
        assert!(guard.is_lost());
    }

    #[tokio::test]
    async fn a_guard_that_is_not_kept_alive_never_reports_a_loss() {
        let Some(pool) = pool().await else { return };
        let lock = DbLock::new("o", &unique_name(), None).unwrap();
        let guard = lock.try_acquire(&pool).await.unwrap().unwrap();

        let waited = tokio::time::timeout(Duration::from_millis(200), guard.lost()).await;
        assert!(waited.is_err());
        assert!(!guard.is_lost());
    }
}
