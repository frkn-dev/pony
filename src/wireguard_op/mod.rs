use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use std::collections::HashSet;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::sync::Arc;

use defguard_wireguard_rs::host::Peer;
use defguard_wireguard_rs::key::Key;
use defguard_wireguard_rs::net::IpAddrMask;
use defguard_wireguard_rs::WGApi;
use defguard_wireguard_rs::WireguardInterfaceApi;

#[cfg(target_os = "linux")]
use defguard_wireguard_rs::Kernel;

#[cfg(target_os = "macos")]
use defguard_wireguard_rs::Userspace;

use crate::http::requests::InboundResponse;
use crate::PonyError;
use crate::Result;

const KEY_LEN: usize = 32;

#[derive(Clone)]
pub struct WgApi {
    pub client: Arc<InnerWgApi>,
}

#[cfg(target_os = "linux")]
type InnerWgApi = WGApi<Kernel>;

#[cfg(target_os = "macos")]
type InnerWgApi = WGApi<Userspace>;

impl WgApi {
    pub fn new(iface: &str) -> Result<Self> {
        InnerWgApi::new(iface.to_string())
            .map(|wg_api| Self {
                client: Arc::new(wg_api),
            })
            .map_err(|e| e.into())
    }

    pub fn validate(&self) -> Result<()> {
        self.client.as_ref().read_host()?;
        Ok(())
    }

    fn decode_pubkey(pubkey: &str) -> Result<Key> {
        let bytes = STANDARD
            .decode(pubkey)
            .map_err(|e| PonyError::Custom(format!("Failed to decode pubkey: {}", e)))?;

        if bytes.len() != KEY_LEN {
            return Err(PonyError::Custom(format!(
                "Invalid WireGuard public key length: {}",
                bytes.len()
            )));
        }

        Ok(Key::new(bytes.as_slice().try_into().unwrap()))
    }

    pub fn create(&self, pubkey: &str, ip: IpAddrMask) -> Result<()> {
        let key = Self::decode_pubkey(pubkey)?;
        let mut peer = Peer::new(key);
        peer.set_allowed_ips(vec![ip]);
        self.client.configure_peer(&peer)?;
        Ok(())
    }

    pub fn last_ip(&self) -> Result<Ipv4Addr> {
        let data = self.client.read_interface_data()?;

        let mut ips: Vec<Ipv4Addr> = data
            .peers
            .values()
            .flat_map(|peer| &peer.allowed_ips)
            .filter_map(|ip_mask| match ip_mask {
                IpAddrMask {
                    ip: IpAddr::V4(addr),
                    cidr: 32,
                } => Some(*addr),
                _ => None,
            })
            .collect();

        ips.sort();

        let last_ip = ips
            .last()
            .cloned()
            .ok_or_else(|| PonyError::Custom("No /32 IPs found in peers".to_string()))?;

        Ok(last_ip)
    }

    pub fn next_available_ip_mask(
        &self,
        network: &IpAddrMask,
        address: &Ipv4Addr,
    ) -> Result<IpAddrMask> {
        let IpAddrMask {
            ip: IpAddr::V4(base_ip),
            cidr,
        } = network
        else {
            return Err(PonyError::Custom("Only IPv4 networks are supported".into()));
        };

        let interface_ip = u32::from_be_bytes(address.octets());
        let base_ip_u32 = u32::from_be_bytes(base_ip.octets());
        let netmask = !((1u32 << (32 - cidr)) - 1);
        let network_addr = base_ip_u32 & netmask;
        let broadcast_addr = network_addr | !netmask;

        let data = self.client.read_interface_data()?;
        let mut used: HashSet<u32> = data
            .peers
            .values()
            .flat_map(|peer| &peer.allowed_ips)
            .filter_map(|ip_mask| match ip_mask {
                IpAddrMask {
                    ip: IpAddr::V4(addr),
                    cidr: 32,
                } => Some(u32::from_be_bytes(addr.octets())),
                _ => None,
            })
            .collect();

        used.insert(interface_ip);

        for ip_u32 in (network_addr + 1)..(broadcast_addr - 1) {
            if !used.contains(&ip_u32) {
                let next_ip = Ipv4Addr::from(ip_u32);
                return Ok(IpAddrMask {
                    ip: IpAddr::V4(next_ip),
                    cidr: 32,
                });
            }
        }

        Err(PonyError::Custom("No available IPs in the subnet".into()))
    }

    pub fn peer_stats(&self, pubkey: &str) -> Result<(i64, i64)> {
        let data = self.client.read_interface_data()?;
        let key = Self::decode_pubkey(pubkey)?;
        let peer = data
            .peers
            .get(&key)
            .ok_or_else(|| PonyError::Custom(format!("Peer with pubkey {} not found", pubkey)))?;
        Ok((peer.rx_bytes as i64, peer.tx_bytes as i64))
    }

    pub fn is_exist(&self, pubkey: String) -> bool {
        Self::decode_pubkey(&pubkey)
            .ok()
            .and_then(|key| {
                self.client
                    .read_interface_data()
                    .ok()
                    .map(|d| d.peers.contains_key(&key))
            })
            .unwrap_or(false)
    }

    pub fn delete(&self, pubkey: &str) -> Result<()> {
        let key = Self::decode_pubkey(pubkey)?;
        self.client.remove_peer(&key)?;
        Ok(())
    }
}

pub fn wireguard_conn(
    conn_id: &uuid::Uuid,
    ipv4: &Ipv4Addr,
    inbound: InboundResponse,
    label: &str,
    private_key: &str,
    client_ip: &IpAddrMask,
) -> Result<String> {
    if let Some(wg) = inbound.wg {
        let server_pubkey = wg.pubkey;
        let host = ipv4;
        let port = wg.port;
        let dns: Vec<_> = wg.dns.iter().map(|d| d.to_string()).collect();
        let dns = dns.join(",");

        let config = format!(
            r#"
[Interface]
PrivateKey = {private_key}
Address    = {client_ip}
DNS        = {dns}

[Peer]
PublicKey           = {server_pubkey}
Endpoint            = {host}:{port}
AllowedIPs          = 0.0.0.0/0, ::/0
PersistentKeepalive = 25
         
# {label} — conn_id: {conn_id}"#
        );

        Ok(config)
    } else {
        Err(PonyError::Custom("WG is not configured".into()))
    }
}
