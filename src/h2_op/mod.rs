use crate::http::requests::InboundResponse;
use crate::{PonyError, Result};
use url::Url;

pub fn hysteria2_conn(
    inbound: &InboundResponse,
    label: &str,
    token: &Option<uuid::Uuid>,
) -> Result<String> {
    let port = inbound.port;
    let h2 = inbound.h2.as_ref().ok_or(PonyError::Custom(
        "Hysteria2: H2Settings missing".to_string(),
    ))?;

    if let Some(inb) = &inbound.h2 {
        let hostname = inb.host.clone();

        let obfs_type = h2
            .obfs
            .as_ref()
            .map(|o| o.r#type.clone())
            .unwrap_or_default();
        let obfs_pass = h2
            .obfs
            .as_ref()
            .map(|o| o.password.clone())
            .unwrap_or_default();

        let alpn = h2.alpn.as_ref().map(|v| v.join(",")).unwrap_or_default();

        if let Some(token) = token {
            let mut url = Url::parse(&format!("hysteria://{token}@{hostname}:{port}"))?;
            url.query_pairs_mut()
                .append_pair("host", &h2.host)
                .append_pair("sni", h2.sni.as_deref().unwrap_or(""))
                .append_pair("insecure", &h2.insecure.to_string())
                .append_pair("obfs", &obfs_type)
                .append_pair("obfs-pass", &obfs_pass)
                .append_pair("alpn", &alpn)
                .append_pair("up-mbps", &h2.up_mbps.unwrap_or(0).to_string())
                .append_pair("down-mbps", &h2.down_mbps.unwrap_or(0).to_string());

            url.set_fragment(Some(label));

            return Ok(url.to_string());
        } else {
            Err(PonyError::Custom("Token is not valid".to_string()).into())
        }
    } else {
        Err(PonyError::Custom("H2 Inbound is not valid".to_string()).into())
    }
}
