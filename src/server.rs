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
use std::sync::Arc;
use std::thread;

// TODO: use error_chain

pub trait MessageProcessing : Send + Sync {
    type Rq;
    type Rsp;
    type E;

    fn new() -> Arc<Self>;

    fn setup(&self, connection_info: String, connection_id: u32) {
        // default implementation das nothing
        println!(
            "default implementation for MessageProcessing::setup: {} : {}",
            connection_info, connection_id
        );
    }

    fn execute(&self, connection_id: u32, rpc: Self::Rq) -> Result<Self::Rsp, Self::E>;

    fn cleanup(&self, connection_info: String, connection_id: u32) {
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


trait Executor {
    type Rq;
    type Rsp;
    type E;

    fn execute(&self, connection_id: u32, rpc: Self::Rq) -> Result<Self::Rsp, Self::E>;
}

impl<T: 'static + MessageProcessing> Executor for T {
    type Rq = <T as MessageProcessing>::Rq;
    type Rsp = <T as MessageProcessing>::Rsp;
    type E = <T as MessageProcessing>::E;

    fn execute(&self, connection_id: u32, rpc: Self::Rq) -> Result<Self::Rsp, Self::E> {
        self.execute(connection_id, rpc)
    }
}

pub struct Server<T: 'static + MessageProcessing> {
    message_processing: Arc<T>,
    port: u16,
    service: Service,
    //TODO store connection id in hash map with all assosiated thread join handles
}

impl<Req, Resp, Error, T> Server<T>
    where Req: DeserializeOwned
        , Resp: Serialize
        , Error: Serialize + std::fmt::Debug
        , T: 'static + MessageProcessing<Rq = Req, Rsp = Resp, E = Error>
{
    pub fn new(port: u16, service: Service) -> Self {
        Server {
            message_processing: T::new(),
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

        const DUMMY_CONNECTION_ID: u32 = 0;
        Self::handle_request(&mut stream, serde, self, DUMMY_CONNECTION_ID)
    }

    fn connection_request(&self, connection_id: u32) -> io::Result<u16> {
        let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        let local_port: u16 = listener.local_addr()?.port();

        // start the server transmission handler
        let message_processing = self.message_processing.clone();
        thread::spawn(move || {
            Server::<T>::transceiver(message_processing, listener, connection_id)
        }); // TODO: store threads in Vec and join them on drop

        Ok(local_port)
    }

    fn transceiver(
        message_processing: Arc<T>,
        listener: TcpListener,
        connection_id: u32,
    ) -> io::Result<()> {
        let local_port: u16 = listener.local_addr()?.port();
        let stream = &mut util::listener_accept_nonblocking(listener)?;
        //TODO: move to listener_accept_nonblocking???
        if let Some(err) = stream.set_nodelay(true).err() {
            println!("server::error::failed to set nodelay: {:?}", err);
        }

        message_processing.setup(
            "TODO: ip address:".to_string() + &local_port.to_string(),
            connection_id,
        );

        let mut serde = bincode::config();
        let serde = serde.big_endian();

        let mut running = true;
        while running {
            if let Err(e) = Self::handle_request(stream, serde, &*message_processing, connection_id)
            {
                println!("server::transmission error: {:?}", e);
                running = false;
            };
        }

        message_processing.cleanup(
            "TODO: ip address:".to_string() + &local_port.to_string(),
            connection_id,
        );

        println!("server::end transceiver");
        Ok(())
    }

    fn handle_request<Rq, Rsp, E, U>(stream: &mut TcpStream, serde: &mut bincode::Config, executor: &U, connection_id: u32) -> io::Result<()>
        where Rq: DeserializeOwned
            , Rsp: Serialize
            , E: Serialize + std::fmt::Debug
            , U: Executor<Rq = Rq, Rsp = Rsp, E = E>
    {

        let payload_size = util::wait_for_transmission(stream)?;
        let payload = util::read_transmission(stream, payload_size)?;

        let (tid, r#type) = payload.split_at(8 as usize);
        let transmission_id = u64::from_be_bytes(<[u8; 8]>::try_from(tid).unwrap());

        let request = serde
            .deserialize::<transport::Type<Rq>>(r#type)
            .unwrap();  //TODO error handling

        if let transport::Type::Request(cmd) = request {
            let response = executor.execute(connection_id, cmd);

            match response {
                Ok(response) => {
                    let response = transport::Transmission {
                        id: transmission_id,
                        r#type: transport::Type::Response(response),
                    };
                    let serialized = serde.serialize(&response).unwrap();
                    util::write_transmission(stream, serialized)?;
                },
                Err(err) => {
                    let response = transport::Transmission {
                        id: transmission_id,
                        r#type: transport::Type::Error(err),
                    };
                    let serialized = serde.serialize(&response).unwrap();
                    util::write_transmission(stream, serialized)?;
                },
            }
        } else {
            let response = transport::Transmission {
                id: transmission_id,
                r#type: transport::Type::Error("Not a request!".to_string()),
            };
            let serialized = serde.serialize(&response).unwrap();
            util::write_transmission(stream, serialized)?;
        }

        Ok(())
    }
}

impl<Req, Resp, Error, T> Executor for Server<T>
    where Req: DeserializeOwned
        , Resp: Serialize
        , Error: Serialize + std::fmt::Debug
        , T: 'static + MessageProcessing<Rq = Req, Rsp = Resp, E = Error>
{
    type Rq = mgmt::Request;
    type Rsp = mgmt::Response;
    type E = transport::Error;
    fn execute(&self, _connection_id: u32, rpc: Self::Rq) -> Result<Self::Rsp, Self::E> {
        match rpc {
            mgmt::Request::Identify{protocol_version} => {
                println!("server::identify request");
                let server_protocol_version = ProtocolVersion::entity().version();
                if server_protocol_version != protocol_version {
                    println!("server::identify -> incompatible protocol versions; server: {}, client: {}", server_protocol_version, protocol_version);
                }
                Ok(mgmt::Response::Identify(mgmt::Identity {
                    protocol_version: server_protocol_version,
                    service: self.service.clone(),
                }))
            },
            mgmt::Request::Connect(params) => {
                println!("server::connection request");
                let port = self.connection_request(params.connection_id).unwrap();
                Ok(mgmt::Response::Connect(mgmt::CommSettings {
                    connection_id: params.connection_id,
                    port,
                }))
            },
        }
    }
}
