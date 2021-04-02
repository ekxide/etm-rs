pub mod client;
pub mod server;
pub mod transport;

mod mgmt;
mod util;

use serde::{Deserialize, Serialize};

pub use util::listener_accept_nonblocking;

pub struct ProtocolVersion {
    version: u32,
}

impl ProtocolVersion {
    pub fn entity() -> Self {
        ProtocolVersion {
            version: 0,
        }
    }

    pub fn version(&self) -> u32 {
        self.version
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Service {
    id: String,
    protocol_version: u32,
}

impl Service {
    pub fn entity(id: String, protocol_version: u32) -> Self {
        Service {
            id,
            protocol_version,
        }
    }

    pub fn id(&self) -> String {
        self.id.clone()
    }

    pub fn protocol_version(&self) -> u32 {
        self.protocol_version
    }
}
