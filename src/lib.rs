/*
 * Copyright (C) 2018 Mathias Kraus <k.hias@gmx.de> - All Rights Reserved
 *
 * Unauthorized copying of this file, via any medium is strictly prohibited
 * Proprietary and confidential
 */

pub mod client;
pub mod server;

mod rpc;
mod util;

use serde::{Deserialize, Serialize};

pub use util::listener_accept_nonblocking;

pub struct ProtocolVersion {
    version: u32,
}

impl ProtocolVersion {
    pub fn entity() -> Self {
        ProtocolVersion {
            version: env!("CARGO_PKG_VERSION_MAJOR")
                .parse::<u32>()
                .unwrap_or(std::u32::MAX),
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

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
