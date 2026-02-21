use crate::PonyError;
use crate::Result;
use serde::Deserialize;
use serde::Serialize;
use std::fs::File;
use std::io::Read;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HysteriaServerConfig {
    pub listen: Option<String>,
    pub acme: Option<AcmeConfig>,
    pub auth: Option<AuthConfig>,
    pub obfs: Option<HysteriaObfs>,
    pub masquerade: Option<Masquerade>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct H2AuthInfo {
    pub auth_type: String,
    pub has_password: bool,
    pub has_url: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcmeConfig {
    pub domains: Option<Vec<String>>,
    pub email: Option<String>,

    #[serde(rename = "type")]
    pub r#type: Option<String>,
    pub dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub r#type: Option<String>,
    pub password: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HysteriaObfs {
    pub r#type: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Masquerade {
    pub r#type: String,
}

impl HysteriaServerConfig {
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let config: HysteriaServerConfig = serde_yaml::from_str(&contents)?;
        Ok(config)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct H2Settings {
    pub host: String,
    pub port: u16,
    pub sni: Option<String>,
    pub insecure: bool,
    pub obfs: Option<H2Obfs>,
    pub alpn: Option<Vec<String>>,
    pub up_mbps: Option<u32>,
    pub down_mbps: Option<u32>,
    pub auth_info: Option<H2AuthInfo>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct H2Obfs {
    pub r#type: String,
    pub password: String,
}

impl HysteriaServerConfig {
    pub fn validate(&self) -> Result<()> {
        if self.listen.is_none() {
            return Err(PonyError::Custom("Hysteria2: listen is required".into()));
        }

        let auth = self
            .auth
            .as_ref()
            .ok_or_else(|| PonyError::Custom("Hysteria2: auth section is required".into()))?;

        if auth.password.clone().unwrap_or("".to_string()).is_empty() {
            return Err(PonyError::Custom(
                "Hysteria2: auth.password is required".into(),
            ));
        }

        Ok(())
    }
}

impl TryFrom<HysteriaServerConfig> for H2Settings {
    type Error = PonyError;

    fn try_from(server: HysteriaServerConfig) -> std::result::Result<H2Settings, PonyError> {
        let listen = server
            .listen
            .ok_or_else(|| PonyError::Custom("Hysteria2: listen missing".into()))?;

        let port = listen
            .split(':')
            .last()
            .unwrap_or("443")
            .parse::<u16>()
            .map_err(|_| PonyError::Custom("Hysteria2: invalid port".into()))?;

        let host = server
            .acme
            .as_ref()
            .and_then(|a| a.domains.as_ref())
            .and_then(|d| d.first())
            .cloned()
            .ok_or_else(|| PonyError::Custom("Hysteria2: acme.domains missing".into()))?;

        let auth_info = server.auth.map(|a| H2AuthInfo {
            auth_type: a.r#type.unwrap_or_else(|| "unknown".into()),
            has_password: a.password.is_some(),
            has_url: a.url.is_some(),
        });

        let obfs = server.obfs.map(|o| H2Obfs {
            r#type: o.r#type.unwrap_or_default(),
            password: o.password.unwrap_or_default(),
        });

        Ok(H2Settings {
            host: host.clone(),
            port,
            sni: Some(host),
            insecure: false,
            alpn: Some(vec!["h2".into(), "http/1.1".into()]),
            obfs,
            up_mbps: None,
            down_mbps: None,
            auth_info,
        })
    }
}
