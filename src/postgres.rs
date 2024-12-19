use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::{Client, NoTls};

use crate::settings::Settings;

pub async fn postgres_client(settings: Settings) -> Result<Arc<Mutex<Client>>, Box<dyn Error>> {
    let pg_settings = settings.pg.clone();

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
