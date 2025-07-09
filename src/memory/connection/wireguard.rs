use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use defguard_wireguard_rs::net::IpAddrMask;
use rand::rngs::OsRng;
use serde::Deserialize;
use serde::Serialize;
use x25519_dalek::{PublicKey, StaticSecret};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Param {
    pub keys: Keys,
    pub address: IpAddrMask,
}

impl Param {
    pub fn new(address: IpAddrMask) -> Self {
        let keys = Keys::default();
        Self { keys, address }
    }
}
