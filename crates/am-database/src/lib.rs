use serde::Deserialize;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

#[derive(Deserialize)]
pub struct DatabaseConfig {
    connection_string: String,
    max_connections: Option<u32>,
}

pub trait DatabaseConfigSource {
    fn get_config(&self) -> &DatabaseConfig;
}

pub struct Database {
    pub pool: PgPool,
}

pub async fn create_database<T>(config: &T) -> anyhow::Result<Database>
where
    T: DatabaseConfigSource,
{
    let config = config.get_config();
    let connection_string = &config.connection_string;

    let pool = PgPoolOptions::new()
        .max_connections(config.max_connections.unwrap_or(10))
        .connect(connection_string)
        .await?;

    tracing::info!("connected to database");

    Ok(Database { pool })
}
