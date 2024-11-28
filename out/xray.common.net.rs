// This file is @generated by prost-build.
/// Address of a network host. It may be either an IP address or a domain
/// address.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IpOrDomain {
    #[prost(oneof = "ip_or_domain::Address", tags = "1, 2")]
    pub address: ::core::option::Option<ip_or_domain::Address>,
}
/// Nested message and enum types in `IPOrDomain`.
pub mod ip_or_domain {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Address {
        /// IP address. Must by either 4 or 16 bytes.
        #[prost(bytes, tag = "1")]
        Ip(::prost::alloc::vec::Vec<u8>),
        /// Domain address.
        #[prost(string, tag = "2")]
        Domain(::prost::alloc::string::String),
    }
}
/// PortRange represents a range of ports.
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct PortRange {
    /// The port that this range starts from.
    #[prost(uint32, tag = "1")]
    pub from: u32,
    /// The port that this range ends with (inclusive).
    #[prost(uint32, tag = "2")]
    pub to: u32,
}
/// PortList is a list of ports.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PortList {
    #[prost(message, repeated, tag = "1")]
    pub range: ::prost::alloc::vec::Vec<PortRange>,
}
