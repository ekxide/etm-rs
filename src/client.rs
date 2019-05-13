/*
 * Copyright (C) 2018 Mathias Kraus <k.hias@gmx.de> - All Rights Reserved
 *
 * Unauthorized copying of this file, via any medium is strictly prohibited
 * Proprietary and confidential
 */

use crate::rpc::{ConnectionRequest, ConnectionResponse, RPCRequest, RPCResponse};
use crate::util;
use crate::{ProtocolVersion, Service};

use serde::{de::DeserializeOwned, Serialize};

use std::net::{Ipv4Addr, Shutdown, TcpStream};
use std::time;

pub struct Connection {
    id: u32,
    port: u16,
    stream: Option<TcpStream>,
    server_protocol_version: u32,
    server_service: Service,
}

impl Connection {
    pub fn new(
        ip: Ipv4Addr,
        connection_request_port: u16,
        connection_id: i32,
    ) -> Option<Box<Connection>> {
        let mut connection: Option<Box<Connection>> = None;
        let mut stream: Option<TcpStream> = None;

        if let Some(err) = TcpStream::connect((ip, connection_request_port))
            .map(|stream_port| {
                let read_timeout = Some(time::Duration::from_secs(2));

                if let Some(err) = stream_port.set_read_timeout(read_timeout).err() {
                    println!("client::error::failed to set tcp timeout: {:?}", err)
                }
                if let Some(err) = stream_port.set_nodelay(true).err() {
                    println!("client::error::failed to set tcp nodelay: {:?}", err);
                }
                if let Some(err) = stream_port.set_nonblocking(false).err() {
                    println!("client::error::failed to set tcp blocking: {:?}", err);
                }

                stream = Some(stream_port);
            })
            .err()
        {
            println!("client::error::failed to open tcp port: {:?}", err)
        }

        let protocol_version = ProtocolVersion::entity().version();
        let mut serde = bincode::config();

        let rpc = ConnectionRequest {
            protocol_version,
            connection_id: std::u32::MAX,
            rpc_interval_timeout_ms: std::u32::MAX,
        };

        let serialized = serde.big_endian().serialize(&rpc).unwrap();
        let payload = Self::send_receive(&mut stream, serialized);

        if let Some(err) = serde
            .big_endian()
            .deserialize::<ConnectionResponse>(&payload)
            .map(|response| {
                println!("client::assigned port::{}", response.port);
                if let Some(err) = TcpStream::connect((ip, response.port))
                    .map(|stream| {
                        if let Some(err) = stream.set_nodelay(true).err() {
                            println!("client::error::failed to set tcp nodelay: {:?}", err);
                        }
                        if let Some(err) = stream.set_nonblocking(false).err() {
                            println!("client::error::failed to set tcp blocking: {:?}", err);
                        }

                        println!("connected to service: '{}'", response.service.id());
                        connection = Some(Box::new(Connection {
                            id: response.connection_id,
                            port: response.port,
                            stream: Some(stream),
                            server_protocol_version: response.protocol_version,
                            server_service: response.service,
                        }));
                    })
                    .err()
                {
                    println!(
                        "client::error::failed to open communication port: {:?}",
                        err
                    )
                };
            })
            .err()
        {
            println!("client::error::deserialize response: {:?}", err);
        }

        connection
    }

    pub fn compatibility_check(&self, service: Service) -> bool {
        let mut compatiblity = true;
        let protocol_version = ProtocolVersion::entity().version();

        if protocol_version != self.server_protocol_version {
            compatiblity = false;
            println!(
                "incompatible ETM versions detected! client on v{} and server on v{}!",
                protocol_version, self.server_protocol_version
            );
        }

        if service.id() != self.server_service.id() {
            compatiblity = false;
            println!(
                "incompatible Services detected! client expects '{}' and server supplies '{}'!",
                service.id(),
                self.server_service.id()
            );
        }

        if service.protocol_version() != self.server_service.protocol_version() {
            compatiblity = false;
            println!(
                "incompatible ETM versions detected! client on v{} and server on v{}!",
                service.protocol_version(),
                self.server_service.protocol_version()
            );
        }

        compatiblity
    }

    pub fn transceive<
        Req: Serialize,
        Resp: DeserializeOwned,
        E: DeserializeOwned + std::fmt::Debug,
    >(
        &mut self,
        request: Req,
    ) -> Option<Resp> {
        let mut serde = bincode::config();

        let request = RPCRequest {
            transmission_id: 42,
            data: request,
        };
        let request = serde.big_endian().serialize(&request).unwrap();

        let response = Self::send_receive(&mut self.stream, request);

        let response = serde
            .big_endian()
            .deserialize::<RPCResponse<Resp, E>>(&response);

        match response {
            Ok(response) => match response.data {
                Ok(response) => Some(response),
                Err(err) => {
                    println!("error response: {:?}", err);
                    None
                }
            },
            Err(err) => {
                println!("error deserializing response: {:?}", err);
                None
            }
        }
        //TODO use map_or_else once feature is in stable rust
        // rpc.map_or_else(|err| { println!("error deserializing response: {:?}", err); None }, |rpc| { Some(rpc.data) } )
    }

    fn send_receive(stream: &mut Option<TcpStream>, serialized: Vec<u8>) -> Vec<u8> {
        let mut databuffer = Vec::<u8>::new();

        if let Some(stream) = stream.as_mut() {
            if let Some(err) = util::send_rpc(stream, serialized).err() {
                println!("client::error::sending: {:?}", err);
            }

            if let Some(err) = util::read_header(stream)
                .and_then(|payload_size| {
                    util::read_payload(stream, payload_size).map(|payload| databuffer = payload)
                })
                .err()
            {
                println!("client::error::receiving: {:?}", err);
            }
        }

        databuffer
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        if let Some(stream) = self.stream.as_mut() {
            println!("client::shutdown stream");
            if let Some(err) = stream.shutdown(Shutdown::Both).err() {
                println!("client::error::shutdown stream: {:?}", err);
            }
        }
    }
}
