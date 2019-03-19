/*
 * Copyright (C) 2018 Mathias Kraus <k.hias@gmx.de> - All Rights Reserved
 *
 * Unauthorized copying of this file, via any medium is strictly prohibited
 * Proprietary and confidential
 */

use crate::rpc::RPC;

use serde::{Serialize, de::DeserializeOwned};

use std::io::prelude::*;
use std::io;
use std::io::{Error, ErrorKind};
use std::net::{TcpListener, TcpStream, Ipv4Addr};
use std::{thread, time};
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
}

impl <Req: DeserializeOwned, Resp: Serialize, T: 'static + MessageProcessing<Request = Req, Response = Resp> + Send> Server<T> {

    pub fn new(port: u16) -> Self {
        Server {
            message_processing: Arc::new(Mutex::new(T::new())),
            port,
        }
    }

    pub fn run(&self) {
        println!("server::run");

        let ip = Ipv4Addr::UNSPECIFIED;

        // bind port
        let listener = self.bind(ip, self.port).unwrap();

        for stream in listener.incoming() {
            println!("server::connenction request");
            stream.and_then(|stream| self.handle_connection_request(stream))
                  .err().map(|err| println!("server::error::connection_handler::{:?}", err));
        }
    }

    fn bind(&self, ip: Ipv4Addr, port: u16) -> Result<TcpListener, io::Error> {
        let port_in_use_sleep_milliseconds = time::Duration::from_millis(100);
        match TcpListener::bind((ip, port)) {
            Ok(listener) => Ok(listener),
            Err(ref e) if e.raw_os_error() == Some(0x62) => {   // port already in use
                println!("server::bind port in use");
                thread::sleep(port_in_use_sleep_milliseconds);
                return self.bind(ip, port);
            },
            Err(err) => Err(err),
        }
    }

    fn handle_connection_request(&self, mut stream: TcpStream) -> io::Result<()> {
        // get listener
        let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        let local_port: u16 = listener.local_addr()?.port();
        let data = local_port.to_be_bytes();

        // set write timeout
        stream.set_write_timeout(Some(time::Duration::from_secs(1)))?;
        stream.write(&data)?;

        // start the server transmission handler
        let message_processing = self.message_processing.clone();
        thread::spawn(move || {Server::<T>::transmission_handler(message_processing, listener)});
        // TODO: store threads in Vec and join them on drop
        Ok(())
    }

    pub fn listener_accept_nonblocking(listener: TcpListener) -> io::Result<TcpStream> {
        listener.set_nonblocking(true)?;    //we can not interrupt the accept call, therefore we have to set the listener to nonblocking and poll every 100ms accept
        for _ in 1..20 {
            match listener.accept() {
                Ok((stream, socket_address)) => {println!("server::transmission_handler::connection to address {}", socket_address); return Ok(stream)},
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {println!("server::transmission_handler::waiting for connection"); thread::sleep(time::Duration::from_millis(100))},
                Err(err)   => {println!("server::error::transmission_handler at waiting for connection::{:?}", err); return Err(err)},
            }
        }

        let err = Err(Error::new(ErrorKind::TimedOut, "timeour while waiting for connection"));
        println!("server::error::{:?}", err);
        err
    }

    fn transmission_handler(message_processing: Arc<Mutex<Box<T>>>, listener: TcpListener) -> io::Result<()> {
        let local_port: u16 = listener.local_addr()?.port();
        let ref mut stream = Server::<T>::listener_accept_nonblocking(listener)?;
        stream.set_nodelay(true).err().map(|err| println!("server::error::failed to set nodelay: {:?}", err) );

        message_processing.lock().unwrap().setup("TODO: ip address:".to_string() + &local_port.to_string(), 13);

        let mut serde = bincode::config();
        let mut datalengthbuffer = [0u8; 4];

        let mut running = true;
        while running {
            // read data length
            stream.read_exact(&mut datalengthbuffer[..]).map(|_| {
                let bytes_to_read = u32::from_be_bytes(datalengthbuffer) as u64;

                let mut databuffer = Vec::<u8>::new();
                stream.take(bytes_to_read).read_to_end(&mut databuffer).map(|_| {
                    let request = serde.big_endian().deserialize::<RPC<Req>>(&databuffer).unwrap();

                    let response = message_processing.lock().unwrap().execute(/*s*/42, request.data);

                    let rpc = RPC { transmission_id: request.transmission_id, data: response };
                    let serialized = serde.big_endian().serialize(&rpc).unwrap();
                    let mut senddata = (serialized.len() as u32).to_be_bytes().to_vec();
                    senddata.extend(serialized);

                    stream.write(&senddata)
                })
            }).err().map(|err| { println!("server::transmission error: {:?}", err); running = false; });
        }

        message_processing.lock().unwrap().cleanup("TODO: ip address:".to_string() + &local_port.to_string(), 13);

        println!("server::end transmission_handler");
        Ok(())
    }
}
