/*
 * Copyright (C) 2018 Mathias Kraus <k.hias@gmx.de> - All Rights Reserved
 *
 * Unauthorized copying of this file, via any medium is strictly prohibited
 * Proprietary and confidential
 */

use std::io::prelude::*;
use std::io;
use std::io::{Error, ErrorKind};
use std::marker::PhantomData;
use std::net::{TcpListener, TcpStream, Ipv4Addr};
use std::{thread, time};

// TODO: use error_chain

pub trait MessageProcessing {
    fn setup(connection_info: String, connection_id: u32) -> () {
        // default implementation das nothing
        println!("default implementation for MessageProcessing::setup");
    }
    
    fn cmd_handler(connection_id: u32, cmd: Vec<u8>) -> Vec<u8>;
    
    fn cleanup(connection_info: String, connection_id: u32) -> () {
        // default implementation das nothing
        println!("default implementation for MessageProcessing::cleanup");
    }
}

pub struct Server<T: MessageProcessing> {
    message_processing: PhantomData<T>,
    port: u16,
}

unsafe impl <T: MessageProcessing> Sync for Server<T> {}

impl <T: MessageProcessing> Server<T> {

    pub fn new(port: u16) -> Server<T> {
        Server {
            message_processing: PhantomData,
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
            match stream {
                Ok(stream) => {
                    if let Err(err) = self.handle_connection_request(stream) {
                        println!("server::error::connection_handler::handleConnection::{:?}", err);
                    }
                },
                Err(err) => println!("server::error::connection_handler::incomming stream::{:?}", err),
            }
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
        let local_port = listener.local_addr()?.port();
        let data : [u8; 2] = [(local_port >> 8) as u8, local_port as u8];
        
        // set write timeout
        stream.set_write_timeout(Some(time::Duration::from_secs(1)))?;
        stream.write(&data)?;
        
        // start the server connection connection handler
        thread::spawn(move || {Server::<T>::transmission_handler(listener)});
        // TODO: store threads in Vec and join them on drop
        Ok(())
    }

    fn listener_accept_nonblocking(listener: TcpListener) -> io::Result<TcpStream> {
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

    fn transmission_handler(listener: TcpListener) -> io::Result<()> {
        let ref mut stream = Server::<T>::listener_accept_nonblocking(listener)?;
        stream.set_nodelay(true);

        // read the length
        let mut datalengthbuffer = [0u8; 4];
        let mut bytes_to_read : usize;
        
        T::setup("TODO: this is the connection info with ip address and port".to_string(), 13);
        
        loop {
            let mut databuffer: Vec<u8> = vec![];
            // read data length
            match stream.read_exact(&mut datalengthbuffer[..]) {
                Err(_)  => {println!("server::error::reading buffer"); break;},
                Ok(_) => {
//                     println!("{:?}", datalengthbuffer);
                    bytes_to_read = (datalengthbuffer[0] as usize) << 24 | (datalengthbuffer[1] as usize) << 16 | (datalengthbuffer[2] as usize) << 8 | (datalengthbuffer[3] as usize);
                    
                    match stream.take(bytes_to_read as u64).read_to_end(&mut databuffer) {
                        Ok(n) => assert_eq!(bytes_to_read as usize, n),
                        _ => panic!("server::didn't read enough"),
                    }
                    
//                     println!("server::bytes read: {}", bytes_to_read);
                    
                    let response = T::cmd_handler(/*connection_id*/42, databuffer);
                    let response_length = response.len();
                    let mut senddata: Vec<u8> = vec![0; 4];
                    senddata[0] = (response_length >> 24) as u8;
                    senddata[1] = (response_length >> 16) as u8;
                    senddata[2] = (response_length >> 8)  as u8;
                    senddata[3] =  response_length        as u8;
                    
                    senddata.extend(response);
                    
                    let _ = stream.write(&senddata);    //ignore write error
                    
                },
            };
            
        }
        
        T::cleanup("TODO: this is the connection info with ip address and port".to_string(), 13);

        println!("server::end transmission_handler");
        Ok(())
    }
}
