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

use std::io;
use std::net::{TcpListener, Ipv4Addr};
use std::thread;
use std::sync::{Arc, Mutex};

// TODO: use error_chain

pub trait MessageProcessing {
    type Request;
    type Response;

    fn new() -> Box<Self>;

    fn setup(&mut self, connection_info: String, connection_id: u32) -> () {
        // default implementation das nothing
        println!("default implementation for MessageProcessing::setup: {} : {}", connection_info, connection_id);
    }

    fn execute(&mut self, connection_id: u32, rpc: Self::Request) -> Self::Response;

    fn cleanup(&mut self, connection_info: String, connection_id: u32) -> () {
        // default implementation das nothing
        println!("default implementation for MessageProcessing::cleanup: {} : {}", connection_info, connection_id);
    }
}

pub struct Server<T: 'static + MessageProcessing + Send> {
    message_processing: Arc<Mutex<Box<T>>>,
    port: u16,
    service: Service,
    //TODO store connection id in hash map with all assosiated thread join handles
}

impl <Req: DeserializeOwned, Resp: Serialize, T: 'static + MessageProcessing<Request = Req, Response = Resp> + Send> Server<T> {

    pub fn new(port: u16, service: Service) -> Self {
        Server {
            message_processing: Arc::new(Mutex::new(T::new())),
            port,
            service,
        }
    }

    pub fn run(&self) {
        println!("server::run");

        let protocol_version = ProtocolVersion::entity().version();
        let mut serde = bincode::config();

        let ip = Ipv4Addr::UNSPECIFIED;

        // bind port
        let listener = util::bind(ip, self.port).unwrap();

        for mut stream in listener.incoming() {
            if let Ok(stream) = stream.as_mut() {
                println!("server::connection request");
                util::read_header(stream).and_then(|payload_size| util::read_payload(stream, payload_size)).and_then(|payload| {
                    //TODO check the etm protocol version once we have a version bump
                    let request = serde.big_endian().deserialize::<ConnectionRequest>(&payload).unwrap();
                    self.connection_request(request.connection_id).and_then(|port| {
                        let rpc = ConnectionResponse { protocol_version, port, connection_id: request.connection_id, service: self.service.clone() };
                        let serialized = serde.big_endian().serialize(&rpc).unwrap();
                        util::send_rpc(stream, serialized)
                    })
                }).err().map(|err| println!("server::error::connection request::{:?}", err));
            }
        }
        println!("server::run -> stop");
    }

    fn connection_request(&self, connection_id: u32) -> io::Result<u16> {
        let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        let local_port: u16 = listener.local_addr()?.port();

        // start the server transmission handler
        let message_processing = self.message_processing.clone();
        thread::spawn(move || {Server::<T>::transmission_handler(message_processing, listener, connection_id)}); // TODO: store threads in Vec and join them on drop

        Ok(local_port)
    }

    fn transmission_handler(message_processing: Arc<Mutex<Box<T>>>, listener: TcpListener, connection_id: u32) -> io::Result<()> {
        let local_port: u16 = listener.local_addr()?.port();
        let ref mut stream = util::listener_accept_nonblocking(listener)?;
        stream.set_nodelay(true).err().map(|err| println!("server::error::failed to set nodelay: {:?}", err) );

        message_processing.lock().unwrap().setup("TODO: ip address:".to_string() + &local_port.to_string(), connection_id);

        let mut serde = bincode::config();

        let mut running = true;
        while running {
            util::read_header(stream).and_then(|payload_size| util::read_payload(stream, payload_size)).and_then(|payload| {
                let request = serde.big_endian().deserialize::<RPC<Req>>(&payload).unwrap();

                let response = message_processing.lock().unwrap().execute(request.transmission_id, request.data);

                let rpc = RPC { transmission_id: request.transmission_id, data: response };
                let serialized = serde.big_endian().serialize(&rpc).unwrap();
                util::send_rpc(stream, serialized)
            }).err().map(|err| { println!("server::transmission error: {:?}", err); running = false; });
        }

        message_processing.lock().unwrap().cleanup("TODO: ip address:".to_string() + &local_port.to_string(), connection_id);

        println!("server::end transmission_handler");
        Ok(())
    }
}
