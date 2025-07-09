use crate::config::settings::WgConfig;
use crate::memory::connection::wireguard::Keys as WgKeys;
use defguard_wireguard_rs::net::IpAddrMask;
use serde::Deserialize;
use serde::Serialize;
use std::net::Ipv4Addr;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WireguardSettings {
    pub pubkey: String,
    pub privkey: String,
    pub interface: String,
    pub network: IpAddrMask,
    pub address: Ipv4Addr,
    pub port: u16,
    pub dns: Vec<Ipv4Addr>,
}

impl WireguardSettings {
    pub fn new(config: &WgConfig) -> Self {
        let (privkey, pubkey) = match (config.privkey.clone(), config.pubkey.clone()) {
            (Some(privkey), Some(pubkey)) => (privkey, pubkey),
            _ => {
                let keys = WgKeys::default();
                (keys.privkey, keys.pubkey)
            }
        };

        let network = config
            .network
            .as_ref()
            .expect("WG network not defined")
            .parse::<IpAddrMask>()
            .unwrap();

        let address = config
            .address
            .as_ref()
            .expect("WG interface address not defined")
            .parse::<Ipv4Addr>()
            .unwrap();

        let dns: Vec<Ipv4Addr> = config
            .dns
            .as_ref()
            .expect("WG DNS servers are  not defined")
            .iter()
            .map(|addr| addr.parse::<Ipv4Addr>().unwrap())
            .collect();

        Self {
            pubkey: pubkey,
            privkey: privkey,
            interface: config.interface.clone(),
            network: network,
            port: config.port,
            address: address,
            dns: dns,
        }
    }
}
