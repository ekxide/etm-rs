/*
 * Copyright (C) 2018 Mathias Kraus <k.hias@gmx.de> - All Rights Reserved
 *
 * Unauthorized copying of this file, via any medium is strictly prohibited
 * Proprietary and confidential
 */

use crate::{ProtocolVersion, Service};
use crate::rpc::{RPC, ConnectionRequest, ConnectionResponse};
use crate::util;

use serde::{Serialize, de::DeserializeOwned};

use std::net::{TcpStream, Ipv4Addr, Shutdown};
use std::time;

pub struct Connection {
    id: u32,
    port: u16,
    stream: Option<TcpStream>,
    server_protocol_version: u32,
    server_service: Service,
}

impl Connection {
    pub fn new(ip: Ipv4Addr, connection_id : i32) -> Option<Box<Connection>> {
        let mut connection: Option<Box<Connection>> = None;
        let mut stream: Option<TcpStream> = None;
        //TODO the port should also be configurable from outside of ETM
        TcpStream::connect((ip, 0xC390)).map(|stream_port| {
            let read_timeout = Some(time::Duration::from_secs(2));
            stream_port.set_read_timeout(read_timeout).err().map(|err| println!("client::error::failed to set tcp timeout: {:?}", err));
            stream = Some(stream_port);
        }).err().map(|err| println!("client::error::failed to open tcp port: {:?}", err));

        let protocol_version = ProtocolVersion::entity().version();
        let mut serde = bincode::config();

        let rpc = ConnectionRequest { protocol_version, connection_id: std::u32::MAX, max_cmd_interval_ms: std::u32::MAX };

        let serialized = serde.big_endian().serialize(&rpc).unwrap();
        let payload = Self::send_receive(&mut stream, serialized);

        serde.big_endian().deserialize::<ConnectionResponse>(&payload).map(|response| {
            println!("client::assigned port::{}", response.port);
            TcpStream::connect((ip, response.port)).map(|stream| {
                stream.set_nodelay(true).err().map(|err| println!("client::error::failed to set tcp nodelay: {:?}", err));
                println!("connected to service: '{}'", response.service.id());
                connection = Some(Box::new(Connection{id: response.connection_id,
                                                      port: response.port,
                                                      stream: Some(stream),
                                                      server_protocol_version: response.protocol_version,
                                                      server_service: response.service,
                }));
            }).err().map(|err| println!("client::error::failed to open communication port: {:?}", err));
        }).err().map(|err| println!("client::error::deserialize response: {:?}", err));

        connection
    }

    pub fn compatibility_check(&self, service: Service) -> bool {
        let mut compatiblity = true;
        let protocol_version = ProtocolVersion::entity().version();

        if protocol_version != self.server_protocol_version {
            compatiblity = false;
            println!("incompatible ETM versions detected! client on v{} and server on v{}!", protocol_version, self.server_protocol_version);
        }

        if service.id() != self.server_service.id() {
            compatiblity = false;
            println!("incompatible Services detected! client expects '{}' and server supplies '{}'!", service.id(), self.server_service.id());
        }

        if service.protocol_version() != self.server_service.protocol_version() {
            compatiblity = false;
            println!("incompatible ETM versions detected! client on v{} and server on v{}!", service.protocol_version(), self.server_service.protocol_version());
        }

        compatiblity
    }

    pub fn transceive<Request: Serialize, Response: DeserializeOwned>(&mut self, request: Request) -> Option<Response> {
        let mut serde = bincode::config();

        let rpc = RPC { transmission_id: 42, data: request };
        let rpc = serde.big_endian().serialize(&rpc).unwrap();

        let rpc = Self::send_receive(&mut self.stream, rpc);

        let rpc = serde.big_endian().deserialize::<RPC<Response>>(&rpc);

        match rpc {
            Ok(rpc) => Some(rpc.data),
            Err(err) => {println!("error deserializing response: {:?}", err); None},
        }
        //TODO use map_or_else once feature is in stable rust
        // rpc.map_or_else(|err| { println!("error deserializing response: {:?}", err); None }, |rpc| { Some(rpc.data) } )
    }

    fn send_receive(stream: &mut Option<TcpStream>, serialized: Vec<u8>) -> Vec<u8> {
        let mut databuffer = Vec::<u8>::new();

        stream.as_mut().map(|stream| {
            util::send_rpc(stream, serialized).err().map(|err| println!("client::error::sending: {:?}", err));

            util::read_header(stream)
                .and_then(|payload_size| util::read_payload(stream, payload_size).map(|payload| databuffer = payload))
                .err().map(|err| println!("client::error::receiving: {:?}", err));
        });

        databuffer
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.stream.as_mut().map(|stream| {
            println!("client::shutdown stream");
            stream.shutdown(Shutdown::Both).err().map(|err| println!("client::error::shutdown stream: {:?}", err));;
        });
    }
}
