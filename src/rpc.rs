/*
 * Copyright (C) 2019 Mathias Kraus <k.hias@gmx.de> - All Rights Reserved
 *
 * Unauthorized copying of this file, via any medium is strictly prohibited
 * Proprietary and confidential
 */

use crate::Service;

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct ConnectionRequest {
    pub protocol_version: u32,
    pub connection_id: u32,         // -1 dynamic
    pub max_cmd_interval_ms: u32,   // -1 infinite
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct ConnectionResponse {
    pub protocol_version: u32,
    pub port: u16,
    pub connection_id: u32,         // requested or assigned connection id
    pub service: Service,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct RPC<T> {
    pub transmission_id: u32,
    pub data: T,
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
