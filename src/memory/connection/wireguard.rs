use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use defguard_wireguard_rs::net::IpAddrMask;
use rand::rngs::OsRng;
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::Deserialize;
use serde::Serialize;
use std::fmt;
use x25519_dalek::{PublicKey, StaticSecret};

#[derive(
    Archive, Clone, Debug, Serialize, Deserialize, PartialEq, RkyvDeserialize, RkyvSerialize,
)]
#[archive(check_bytes)]
pub struct Keys {
    pub privkey: String,
    pub pubkey: String,
}

impl Default for Keys {
    fn default() -> Self {
        let secret = StaticSecret::random_from_rng(OsRng);
        let public = PublicKey::from(&secret);

        let privkey_bytes = secret.as_bytes();
        let pubkey_bytes = public.as_bytes();

        Self {
            privkey: STANDARD.encode(privkey_bytes),
            pubkey: STANDARD.encode(pubkey_bytes),
        }
    }
}

#[derive(
    Archive, Serialize, Deserialize, RkyvSerialize, RkyvDeserialize, Clone, Debug, PartialEq,
)]
#[archive(check_bytes)]
pub struct IpAddrMaskSerializable {
    pub addr: String,
    pub cidr: u8,
}

impl From<IpAddrMask> for IpAddrMaskSerializable {
    fn from(ip_mask: IpAddrMask) -> Self {
        IpAddrMaskSerializable {
            addr: ip_mask.ip.to_string(),
            cidr: ip_mask.cidr,
        }
    }
}

impl From<IpAddrMaskSerializable> for defguard_wireguard_rs::net::IpAddrMask {
    fn from(val: IpAddrMaskSerializable) -> Self {
        let addr = val.addr.parse().expect("Invalid IP address string");

        defguard_wireguard_rs::net::IpAddrMask::new(addr, val.cidr)
    }
}

impl fmt::Display for IpAddrMaskSerializable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.addr, self.cidr)
    }
}

#[derive(
    Archive, Clone, Debug, Serialize, Deserialize, RkyvDeserialize, RkyvSerialize, PartialEq,
)]
#[archive(check_bytes)]
pub struct Param {
    pub keys: Keys,
    pub address: IpAddrMaskSerializable,
}

impl Param {
    pub fn new(address: IpAddrMask) -> Self {
        let keys = Keys::default();
        Self {
            keys,
            address: address.into(),
        }
    }
}

impl fmt::Display for Param {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "privkey: {} pubkey: {} address: {}",
            self.keys.privkey, self.keys.pubkey, self.address
        )
    }
}
