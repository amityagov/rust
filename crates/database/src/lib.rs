//! Postgres connection pool setup on top of `sqlx`.

use serde::Deserialize;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

/// Pool size used when [`DatabaseConfig::max_connections`] is not set.
const DEFAULT_MAX_CONNECTIONS: u32 = 10;

/// Where and how to connect.
#[derive(Deserialize)]
pub struct DatabaseConfig {
    pub connection_string: String,
    /// Upper bound of the pool; defaults to 10.
    pub max_connections: Option<u32>,
}

/// Implemented by an application's own configuration so it can be passed to
/// [`create_database`] as is.
pub trait DatabaseConfigSource {
    fn get_config(&self) -> &DatabaseConfig;
}

/// A connected database.
pub struct Database {
    pub pool: PgPool,
}

/// Connects a pool using the configuration provided by `source`.
///
/// Fails if `max_connections` is zero.
pub async fn create_database<T>(source: &T) -> anyhow::Result<Database>
where
    T: DatabaseConfigSource,
{
    let config = source.get_config();
    let max_connections = config.max_connections.unwrap_or(DEFAULT_MAX_CONNECTIONS);
    // A pool of zero connections connects fine and then never hands one out.
    anyhow::ensure!(max_connections > 0, "max_connections must be at least 1");

    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(&config.connection_string)
        .await?;

    tracing::info!("connected to database");

    Ok(Database { pool })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Config(DatabaseConfig);

    impl DatabaseConfigSource for Config {
        fn get_config(&self) -> &DatabaseConfig {
            &self.0
        }
    }

    #[tokio::test]
    async fn rejects_an_empty_pool() {
        let config = Config(DatabaseConfig {
            connection_string: "postgres://localhost/unused".into(),
            max_connections: Some(0),
        });

        let error = create_database(&config).await.err().unwrap();

        assert!(error.to_string().contains("max_connections"));
    }
}
