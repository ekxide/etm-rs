/*
 * Copyright (C) 2018 Mathias Kraus <k.hias@gmx.de> - All Rights Reserved
 *
 * Unauthorized copying of this file, via any medium is strictly prohibited
 * Proprietary and confidential
 */

use crate::mgmt;
use crate::transport;
use crate::util;
use crate::{ProtocolVersion, Service};

use serde::{de::DeserializeOwned, Serialize};

use std::convert::TryFrom;
use std::io;
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

// TODO: use error_chain

pub trait MessageProcessing {
    type Rq;
    type Rsp;
    type E;

    fn new() -> Box<Self>;

    fn setup(&mut self, connection_info: String, connection_id: u32) {
        // default implementation das nothing
        println!(
            "default implementation for MessageProcessing::setup: {} : {}",
            connection_info, connection_id
        );
    }

    fn execute(&mut self, connection_id: u32, rpc: Self::Rq) -> Result<Self::Rsp, Self::E>;

    fn cleanup(&mut self, connection_info: String, connection_id: u32) {
        // default implementation das nothing
        println!(
            "default implementation for MessageProcessing::cleanup: {} : {}",
            connection_info, connection_id
        );
    }

    // TODO
    // - add post_transmission (to be able to perform an action after the message is sent over TCP)
    // - add connection change handler (to be able to perform special actions on first open/last close, e.g. blink LED)
}

pub struct Server<T: 'static + MessageProcessing + Send> {
    message_processing: Arc<Mutex<Box<T>>>,
    port: u16,
    service: Service,
    //TODO store connection id in hash map with all assosiated thread join handles
}

impl<
        Req: DeserializeOwned,
        Resp: Serialize,
        Error: Serialize + std::fmt::Debug,
        T: 'static + MessageProcessing<Rq = Req, Rsp = Resp, E = Error> + Send,
    > Server<T>
{
    pub fn new(port: u16, service: Service) -> Self {
        Server {
            message_processing: Arc::new(Mutex::new(T::new())),
            port,
            service,
        }
    }

    pub fn run(&self) -> io::Result<()> {
        println!("server::run");

        let ip = Ipv4Addr::UNSPECIFIED;

        // bind port
        let listener = util::bind(ip, self.port)?;

        let mut serde = bincode::config();

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    if let Err(e) = self.handle_mgmt_request(stream, serde.big_endian()) {
                        println!("server::run -> mgmt request error: {:?}", e);
                    }
                },
                Err(e) => println!("server::run -> error: {:?}", e),
            }
        }
        println!("server::run -> stop");
        Ok(())
    }

    fn handle_mgmt_request(&self, mut stream: TcpStream, serde: &mut bincode::Config) -> io::Result<()> {
        stream.set_nonblocking(false)?;

        let server_protocol_version = ProtocolVersion::entity().version();

        let payload_size = util::read_header(&mut stream)?;
        let payload = util::read_payload(&mut stream, payload_size)?;

        let request = serde
            .deserialize::<transport::Transmission<mgmt::Request>>(&payload)
            .unwrap();  //TODO error handling

        if let transport::Type::Request(request) = request.r#type {
            match request {
                mgmt::Request::Identify{protocol_version} => {
                    println!("server::identify request");
                    if server_protocol_version != protocol_version {
                        println!("server::identify -> incompatible protocol versions; server: {}, client: {}", server_protocol_version, protocol_version);
                    }
                    let identity = mgmt::Response::Identify(mgmt::Identity {
                        protocol_version: server_protocol_version,
                        service: self.service.clone(),
                    });
                    let response = transport::Transmission {
                        id: 0,
                        r#type: transport::Type::Response(identity),
                    };
                    let serialized = serde.serialize(&response).unwrap();
                    util::send_rpc(&mut stream, serialized)?;
                },
                mgmt::Request::Connect(params) => {
                    println!("server::connection request");
                    let port = self.connection_request(params.connection_id)?;
                    let comm_settings = mgmt::Response::Connect(mgmt::CommSettings {
                        connection_id: params.connection_id,
                        port,
                    });
                    let response = transport::Transmission {
                        id: 0,
                        r#type: transport::Type::Response(comm_settings),
                    };
                    let serialized = serde.serialize(&response).unwrap();
                    util::send_rpc(&mut stream, serialized)?;
                },
            }
        }

        Ok(())
    }

    fn connection_request(&self, connection_id: u32) -> io::Result<u16> {
        let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        let local_port: u16 = listener.local_addr()?.port();

        // start the server transmission handler
        let message_processing = self.message_processing.clone();
        thread::spawn(move || {
            Server::<T>::transmission_handler(message_processing, listener, connection_id)
        }); // TODO: store threads in Vec and join them on drop

        Ok(local_port)
    }

    fn transmission_handler(
        message_processing: Arc<Mutex<Box<T>>>,
        listener: TcpListener,
        connection_id: u32,
    ) -> io::Result<()> {
        let local_port: u16 = listener.local_addr()?.port();
        let stream = &mut util::listener_accept_nonblocking(listener)?;
        if let Some(err) = stream.set_nodelay(true).err() {
            println!("server::error::failed to set nodelay: {:?}", err);
        }

        message_processing.lock().unwrap().setup(
            "TODO: ip address:".to_string() + &local_port.to_string(),
            connection_id,
        );

        let mut serde = bincode::config();
        let serde = serde.big_endian();

        let mut running = true;
        while running {
            if let Some(err) = util::read_header(stream)
                .and_then(|payload_size| util::read_payload(stream, payload_size))
                .and_then(|payload| {

                    let (tid, r#type) = payload.split_at(8 as usize);
                    let transmission_id = u64::from_be_bytes(<[u8; 8]>::try_from(tid).unwrap());

                    let request = serde
                        .deserialize::<transport::Type<Req>>(r#type)
                        .unwrap();  //TODO error handling

                    if let transport::Type::Request(cmd) = request {
                        let response = message_processing
                            .lock()
                            .unwrap()
                            .execute(connection_id, cmd);

                        match response {
                            Ok(response) => {
                                let response = transport::Transmission {
                                    id: transmission_id,
                                    r#type: transport::Type::Response(response),
                                };
                                let serialized = serde.serialize(&response).unwrap();
                                util::send_rpc(stream, serialized)
                            },
                            Err(err) => {
                                let response = transport::Transmission {
                                    id: transmission_id,
                                    r#type: transport::Type::Error(err),
                                };
                                let serialized = serde.serialize(&response).unwrap();
                                util::send_rpc(stream, serialized)
                            },
                        }
                    } else {
                        let response = transport::Transmission {
                            id: transmission_id,
                            r#type: transport::Type::Error("fubar".to_string()),
                        };
                        let serialized = serde.serialize(&response).unwrap();
                        util::send_rpc(stream, serialized)
                    }
                })
                .err()
            {
                println!("server::transmission error: {:?}", err);
                running = false;
            };
        }

        message_processing.lock().unwrap().cleanup(
            "TODO: ip address:".to_string() + &local_port.to_string(),
            connection_id,
        );

        println!("server::end transmission_handler");
        Ok(())
    }
}
