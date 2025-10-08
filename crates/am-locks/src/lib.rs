use sqlx::PgPool;

pub struct LockGuard {}

pub struct PersistentLock {}

impl PersistentLock {
    pub async fn lock(pool: &PgPool) -> LockGuard {
        LockGuard {}
    }
}
