/*
 * Copyright (C) 2019 Mathias Kraus <k.hias@gmx.de> - All Rights Reserved
 *
 * Unauthorized copying of this file, via any medium is strictly prohibited
 * Proprietary and confidential
 */

use crate::Service;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Identity {
    pub protocol_version: u32,
    pub service: Service,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct CommParams {
    pub protocol_version: u32,
    pub connection_id: u32,           // -1 dynamic
    pub rpc_interval_timeout_ms: u32, // -1 infinite
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct CommSettings {
    pub protocol_version: u32,
    pub connection_id: u32, // assigned connection id
    pub port: u16,
    pub service: Service,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum Request {
    Identify{protocol_version: u32},
    Connect(CommParams),
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum Response {
    Identify(Identity),
    Connect(CommSettings),
}
