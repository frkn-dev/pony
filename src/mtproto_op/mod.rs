use crate::config::xray::Inbound;
use reqwest::Url;
use std::net::Ipv4Addr;

use crate::PonyError;
use crate::Result;

pub fn mtproto_conn(address: Ipv4Addr, inbound: &Inbound, label: &str) -> Result<String> {
    let port = inbound.port;

    let secret = inbound
        .mtproto_secret
        .as_ref()
        .ok_or(PonyError::Custom("MTProto: secret missing".to_string()))?;

    let mut url = Url::parse(&format!(
        "https://t.me/proxy?server={address}&port={port}&secret={secret}"
    ))?;

    url.set_fragment(Some(label));

    Ok(url.to_string())
}
