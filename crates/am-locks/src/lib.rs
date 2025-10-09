use sqlx::PgPool;
use std::time::Duration;

#[derive(Clone)]
pub struct DbLock {
    owner: String,
    name: String,
    ttl_seconds: i64,
}

pub struct LockGuard {
    lock: DbLock,
}

impl DbLock {
    pub fn new(owner: &str, name: &str, ttl_seconds: Option<i64>) -> Self {
        let ttl_seconds = ttl_seconds.unwrap_or(60);
        Self {
            owner: owner.to_string(),
            name: name.to_string(),
            ttl_seconds,
        }
    }

    pub async fn try_acquire(&self, pool: &PgPool) -> anyhow::Result<bool> {
        let mut conn = pool.acquire().await?;
        let acquired: bool = sqlx::query_scalar(
            r#"
            SELECT acquire_lock($1, $2, $3)
            "#,
        )
        .bind(&self.name)
        .bind(&self.owner)
        .bind(self.ttl_seconds)
        .fetch_one(&mut *conn)
        .await?;
        Ok(acquired)
    }

    pub async fn acquire(&self, pool: &PgPool, retry_delay: Duration) -> anyhow::Result<LockGuard> {
        loop {
            if self.try_acquire(pool).await? {
                tracing::trace!("Lock '{}' acquired by {}", self.name, self.owner);

                return Ok(LockGuard { lock: self.clone() });
            } else {
                tracing::trace!("Lock '{}' busy, retrying...", self.name);
                tokio::time::sleep(retry_delay).await;
            }
        }
    }

    pub async fn release(&self, pool: &PgPool) -> anyhow::Result<bool> {
        let mut conn = pool.acquire().await?;
        let released: bool = sqlx::query_scalar(
            r#"
            SELECT release_lock($1, $2)
            "#,
        )
        .bind(&self.name)
        .bind(&self.owner)
        .fetch_one(&mut *conn)
        .await?;
        Ok(released)
    }
}

impl LockGuard {
    pub async fn release(self, pool: &PgPool) -> anyhow::Result<bool> {
        self.lock.release(pool).await
    }
}
