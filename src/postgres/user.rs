use chrono::NaiveDateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::Client as PgClient;
use uuid::Uuid;

use crate::zmq::message::Action;
use crate::zmq::message::Message as UserRequest;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct UserRow {
    pub user_id: Uuid,
    pub username: Option<String>,
    pub trial: bool,
    pub limit: i64,
    pub password: String,
    pub cluster: String,
    pub created: NaiveDateTime,
}

impl UserRow {
    pub fn new(username: &str, user_id: Uuid) -> Self {
        Self {
            user_id: user_id,
            username: Some(username.to_string()),
            trial: true,
            limit: 1000,
            password: "random123".to_string(),
            cluster: "dev".to_string(),
            created: Utc::now().naive_utc(),
        }
    }

    pub fn as_create_user_request(&self) -> UserRequest {
        UserRequest {
            user_id: self.user_id,
            action: Action::Create,
            env: self.cluster.clone(),
            trial: self.trial,
            limit: self.limit,
            password: Some(self.password.clone()),
        }
    }
}

pub struct PgUserRequest {
    pub client: Arc<Mutex<PgClient>>,
}

impl PgUserRequest {
    pub fn new(client: Arc<Mutex<PgClient>>) -> Self {
        Self { client }
    }

    pub async fn get_all_users(&self) -> Result<Vec<UserRow>, Box<dyn Error + Send + Sync>> {
        let client = self.client.lock().await;

        let query = "
        SELECT id, trial, data_limit_mb, password, cluster, created 
        FROM users
    ";

        let rows = client.query(query, &[]).await?;
        let users = self.map_rows_to_users(rows);
        Ok(users)
    }

    pub async fn get_users_by_cluster(
        &self,
        cluster: &str,
    ) -> Result<Vec<UserRow>, Box<dyn Error + Send + Sync>> {
        let client = self.client.lock().await;

        let query = "
        SELECT id, trial, data_limit_mb, password, cluster, created 
        FROM users 
        WHERE cluster = $1
    ";

        let rows = client.query(query, &[&cluster]).await?;

        let users = self.map_rows_to_users(rows);
        Ok(users)
    }

    fn map_rows_to_users(&self, rows: Vec<tokio_postgres::Row>) -> Vec<UserRow> {
        rows.into_iter()
            .map(|row| {
                let user_id: Uuid = row.get(0);
                let trial: bool = row.get(1);
                let limit: i64 = row.get(2);
                let password: String = row.get(3);
                let cluster: String = row.get(4);
                let created: NaiveDateTime = row.get(5);

                UserRow {
                    user_id,
                    username: None,
                    trial,
                    limit,
                    password,
                    cluster,
                    created,
                }
            })
            .collect()
    }

    pub async fn user_exist(&self, username: String) -> Option<Uuid> {
        let client = self.client.lock().await;

        let query = "
        SELECT id 
        FROM users 
        WHERE username = $1
    ";

        match client.query(query, &[&username]).await {
            Ok(rows) => rows.first().map(|row| row.get(0)),
            Err(err) => {
                log::error!("Database query failed: {}", err);
                None
            }
        }
    }

    pub async fn insert_user(&self, user: UserRow) -> Result<(), Box<dyn Error + Send + Sync>> {
        let client = self.client.lock().await;

        let query = "
        INSERT INTO users (id, username, trial, data_limit_mb, password, cluster, created)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
    ";

        client
            .execute(
                query,
                &[
                    &user.user_id,
                    &user.username,
                    &user.trial,
                    &user.limit,
                    &user.password,
                    &user.cluster,
                    &user.created,
                ],
            )
            .await?;

        Ok(())
    }

    pub async fn insert_tg_user(
        &self,
        username: String,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let client = self.client.lock().await;
        let user_id = uuid::Uuid::new_v4();
        let now = Utc::now();

        let query = "
        INSERT INTO tg_users (id, username, created)
        VALUES ($1, $2, $3)
    ";

        client.execute(query, &[&user_id, &username, &now]).await?;

        Ok(())
    }
}
