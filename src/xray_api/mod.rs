pub mod xray {
    pub mod common {
        pub mod log {
            tonic::include_proto!("xray.common.log");
        }
        pub mod net {
            tonic::include_proto!("xray.common.net");
        }
        pub mod protocol {
            tonic::include_proto!("xray.common.protocol");
        }
        pub mod serial {
            tonic::include_proto!("xray.common.serial");
        }
    }
    pub mod transport {
        pub mod internet {
            tonic::include_proto!("xray.transport.internet");
        }
    }
    pub mod app {
        pub mod proxyman {
            tonic::include_proto!("xray.app.proxyman");
            pub mod command {
                tonic::include_proto!("xray.app.proxyman.command");
            }
        }
        pub mod log {
            tonic::include_proto!("xray.app.log");
            pub mod command {
                tonic::include_proto!("xray.app.log.command");
            }
        }
        pub mod stats {
            tonic::include_proto!("xray.app.stats");
            pub mod command {
                tonic::include_proto!("xray.app.stats.command");
            }
        }
    }

    pub mod core {
        tonic::include_proto!("xray.core");
    }
}
