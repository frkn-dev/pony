use hmac::{Hmac, Mac};
use lettre::AsyncTransport;
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};

use chrono::{DateTime, Utc};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;

use super::config::SmtpConfig;

type HmacSha256 = Hmac<Sha256>;

use lettre::{
    transport::smtp::authentication::Credentials, AsyncSmtpTransport, Message, Tokio1Executor,
};

#[derive(Clone)]
pub struct EmailStore {
    pub store: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
    file: String,
    smtp: SmtpConfig,
    secret: Vec<u8>,
    pub web_host: String,

    mailer: Arc<AsyncSmtpTransport<Tokio1Executor>>,
}

impl EmailStore {
    pub fn new(file: String, smtp: SmtpConfig, secret: Vec<u8>, web_host: String) -> Self {
        let mailer = AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp.server)
            .expect("invalid smtp server")
            .credentials(Credentials::new(
                smtp.username.clone(),
                smtp.password.clone(),
            ))
            .build();

        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
            file,
            smtp,
            secret,
            web_host,
            mailer: Arc::new(mailer),
        }
    }

    fn hmac_email(&self, email: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(&self.secret).unwrap();
        mac.update(email.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    pub async fn check_email_hmac(&self, email: &str) -> bool {
        let email_hmac = self.hmac_email(email);
        let store = self.store.read().await;
        store.contains_key(&email_hmac)
    }

    pub async fn save_trial_hmac(
        &self,
        email: &str,
        sub_id: &uuid::Uuid,
        time: &DateTime<Utc>,
        ref_by: &str,
    ) -> std::io::Result<()> {
        let email_hmac = self.hmac_email(email);

        {
            let mut store = self.store.write().await;
            store.insert(email_hmac.clone(), *time);
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file)
            .await;

        let line = format!(
            "{},{},{},{}\n",
            time.to_rfc3339(),
            email_hmac,
            sub_id,
            ref_by
        );

        file?.write_all(line.as_bytes()).await?;

        Ok(())
    }

    pub async fn load_trials(&self) -> std::io::Result<()> {
        let file = match File::open(&self.file).await {
            Ok(f) => f,
            Err(_) => return Ok(()),
        };

        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        let mut map = HashMap::new();

        while let Some(line) = lines.next_line().await? {
            let parts: Vec<&str> = line.split(',').collect();

            if parts.len() >= 2 {
                if let Ok(ts) = parts[0].parse::<DateTime<Utc>>() {
                    map.insert(parts[1].to_string(), ts);
                }
            }
        }

        let mut store = self.store.write().await;
        *store = map;

        Ok(())
    }

    pub async fn send_email_background(&self, to: String, sub_id: uuid::Uuid) {
        let mailer = self.mailer.clone();
        let web_host = self.web_host.clone();
        let from = self.smtp.from.clone();
        tokio::spawn(async move {
            let html_body = format!(
                r#"
                <html>
                <body>
                    <h2>Твой Тест-Драйв активирован</h2>
                    <p>Ссылка:</p>
                    <a href="{web_host}/subscription?id={sub_id}">
                        Открыть подписку
                    </a>
                </body>
                </html>
                "#,
                web_host = web_host,
                sub_id = sub_id,
            );

            let msg = match Message::builder()
                .from(from.parse().unwrap())
                .to(to.parse().unwrap())
                .subject("Тест-Драйв")
                .header(lettre::message::header::ContentType::TEXT_HTML)
                .body(html_body)
            {
                Ok(m) => m,
                Err(e) => {
                    tracing::error!("Email build error: {}", e);
                    return;
                }
            };

            if let Err(e) = mailer.send(msg).await {
                tracing::error!("Email send error: {}", e);
            }
        });
    }
}
