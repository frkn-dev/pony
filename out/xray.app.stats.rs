// This file is @generated by prost-build.
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct Config {}
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct ChannelConfig {
    #[prost(bool, tag = "1")]
    pub blocking: bool,
    #[prost(int32, tag = "2")]
    pub subscriber_limit: i32,
    #[prost(int32, tag = "3")]
    pub buffer_size: i32,
}
