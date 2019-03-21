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
    pub connection_id: u32,             // -1 dynamic
    pub rpc_interval_timeout_ms: u32,   // -1 infinite
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct ConnectionResponse {
    pub protocol_version: u32,
    pub port: u16,
    pub connection_id: u32,         // assigned connection id
    pub service: Service,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Request<T> {
    pub transmission_id: u32,
    pub data: T,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Response<T, E> {
    pub transmission_id: u32,
    pub data: Result<T, E>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
