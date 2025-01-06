use chrono::NaiveDateTime;
use serde::Deserialize;
use serde::Serialize;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::{Client, NoTls};
use uuid::Uuid;

use crate::config::settings::PostgresConfig;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct UserRow {
    pub user_id: Uuid,
    pub trial: bool,
    pub password: String,
    pub cluster: String,
    pub created: NaiveDateTime,
}

pub async fn postgres_client(
    pg_settings: PostgresConfig,
) -> Result<Arc<Mutex<Client>>, Box<dyn Error>> {
    let connection_line = format!(
        "host={} user={} dbname={} password={} port={}",
        pg_settings.host,
        pg_settings.username,
        pg_settings.db,
        pg_settings.password,
        pg_settings.port
    );

    let (client, connection) = tokio_postgres::connect(&connection_line, NoTls).await?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Error in PostgreSQL connection: {}", e);
        }
    });

    Ok(Arc::new(Mutex::new(client)))
}
