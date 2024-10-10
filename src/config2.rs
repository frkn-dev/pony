use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CarbonConfig {
    pub address: String,
}

#[derive(Debug, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AppConfig {
    pub env: String,
    pub hostname: String,
    pub iface: String,
    pub xray_vmess_port: u16,
    pub xray_vless_port: u16,
    pub xray_ss_port: u16,
    pub metrics_delay: u64,
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub carbon: CarbonConfig,
    pub logging: LoggingConfig,
    pub app: AppConfig,
}
