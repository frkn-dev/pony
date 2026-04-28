use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use lettre::{
    transport::smtp::authentication::Credentials, AsyncSmtpTransport, AsyncTransport, Message,
    Tokio1Executor,
};
use sha2::Sha256;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::RwLock;

use super::config::SmtpConfig;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
pub struct EmailStore {
    pub store: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
    file: String,
    smtp: SmtpConfig,
    secret: Vec<u8>,
    pub web_host: String,
}

impl EmailStore {
    pub fn new(file: String, smtp: SmtpConfig, secret: Vec<u8>, web_host: String) -> Self {
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
            file,
            smtp,
            secret,
            web_host,
        }
    }

    fn hmac_email(&self, email: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(&self.secret).unwrap();
        mac.update(email.as_bytes());
        hex::encode(mac.finalize().into_bytes())
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

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file)?;

        writeln!(
            file,
            "{},{},{},{}",
            time.to_rfc3339(),
            email_hmac,
            sub_id,
            ref_by
        )?;

        Ok(())
    }

    pub async fn check_email_hmac(&self, email: &str) -> bool {
        let email_hmac = self.hmac_email(email);

        let store = self.store.read().await;
        store.contains_key(&email_hmac)
    }

    pub async fn send_email(
        &self,
        to: &str,
        sub_id: &uuid::Uuid,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let html_body = format!(
            r#"
    <!DOCTYPE html>
    <html>
    <head>
    <meta charset="UTF-8">
    <title>FRKN Рилзопровод</title>
    <style>
        body {{
            font-family: Arial, sans-serif;
            background-color: #f4f4f4;
            margin: 0;
            padding: 0;
        }}
        .container {{
            width: 100%;
            max-width: 600px;
            margin: 0 auto;
            background-color: #ffffff;
            padding: 20px;
            border-radius: 12px;
            box-shadow: 0 4px 12px rgba(0,0,0,0.1);
        }}
        .header {{
            text-align: center;
            margin-bottom: 20px;
        }}
        .logo {{
            max-width: 150px;
        }}
        h1 {{
            color: #1d4ed8;
            font-size: 24px;
        }}
        p {{
            color: #374151;
            font-size: 16px;
            line-height: 1.5;
        }}
        .button {{
            display: inline-block;
            padding: 12px 24px;
            background-color: #1d4ed8;
            color: #ffffff;
            text-decoration: none;
            border-radius: 8px;
            margin: 20px 0;
            font-weight: bold;
        }}
        .footer {{
            font-size: 12px;
            color: #9ca3af;
            text-align: center;
            margin-top: 20px;
        }}
    </style>
    </head>
    <body>
    <div class="container">
        <div class="header">
            <h1>Твоя подписка FRKN активирована!</h1>
        </div>
        <p>Привет!</p>
        <p>Твоя подписка для <strong>FRKN</strong> успешно активирована 🎉</p>

        <a href="{web_host}/subscription?id={sub_id}"

         style="
           display: inline-block;
           padding: 12px 24px;
           background-color: #1d4ed8;
           color: #ffffff !important;
           text-decoration: none;
           border-radius: 8px;
           font-weight: bold;">Перейти к подписке</a>

        <br><br>
        <p>
            Твой <strong>ID:</strong> {sub_id}<br/>
            <div style="font-size: small;">ID это твой уникальный идентификатор подписки, рекомендуем его записать или запомнить</div>
            <div style="font-size: small;">Мы не храним твой email</div>

        </p>

        <p>Подписывайся на наш Telegram: <a href="https://t.me/frkn_org">@frkn_org</a></p>
        <br>
        <div class="footer">
        <a href="https://t.me/frkn_support">Поддержка</a></p> • Vive la résistance!<br/>
           {web_host} • © 2026 FRKN Privacy Company
        </div>
    </div>
    </body>
    </html>
    "#,
            sub_id = sub_id,
            web_host = self.web_host,
        );

        let msg = Message::builder()
            .from(format!("FRKN <{}>", self.smtp.from).parse()?)
            .to(to.parse()?)
            .subject("FRKN Рилзопровод")
            .header(lettre::message::header::ContentType::TEXT_HTML)
            .body(html_body)?;

        let mailer = AsyncSmtpTransport::<Tokio1Executor>::relay(&self.smtp.server)?
            .credentials(Credentials::new(
                self.smtp.username.clone(),
                self.smtp.password.clone(),
            ))
            .build();

        mailer.send(msg).await?;
        Ok(())
    }
}
