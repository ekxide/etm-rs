// Copyright 2018 Mathias Kraus <k.hias@gmx.de> - All rights reserved
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

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
    pub connection_id: u32, // assigned connection id
    pub port: u16,
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
