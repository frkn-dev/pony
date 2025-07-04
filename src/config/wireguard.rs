use crate::config::settings::WgConfig;
use crate::state::connection::wireguard::Keys as WgKeys;
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
        let (privkey, pubkey) = match (config.pubkey.clone(), config.privkey.clone()) {
            (Some(privkey), Some(pubkey)) => (privkey, pubkey),
            _ => {
                let keys = WgKeys::default();
                (keys.privkey, keys.pubkey)
            }
        };

        let network = if let Some(network) = &config.network {
            network.parse::<IpAddrMask>().unwrap()
        } else {
            panic!("WG network not defined");
        };

        let address = if let Some(address) = &config.address {
            address.parse::<Ipv4Addr>().unwrap()
        } else {
            panic!("WG interface address not defined");
        };

        let dns: Vec<Ipv4Addr> = if let Some(dns) = &config.dns {
            dns.iter()
                .map(|addr| addr.parse::<Ipv4Addr>().unwrap())
                .collect()
        } else {
            panic!("WG DNS servers are  not defined");
        };

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
