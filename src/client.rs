/*
 * Copyright (C) 2018 Mathias Kraus <k.hias@gmx.de> - All Rights Reserved
 *
 * Unauthorized copying of this file, via any medium is strictly prohibited
 * Proprietary and confidential
 */

use std::io::prelude::*;
use std::net::{TcpStream, Ipv4Addr, Shutdown};
use std::time;

pub struct Connection {
    id: i32,
    port: u16,
    stream: Option<TcpStream>
}

impl Connection {
    pub fn new(ip: Ipv4Addr, connection_id : i32) -> Option<Box<Connection>> {
        // request communication port
        match TcpStream::connect((ip, 0xC390)) {
            Ok(mut stream) => {
                // set read timeout
                let read_timeout = Some(time::Duration::from_secs(2));
                stream.set_read_timeout(read_timeout);

                // read the communication port
                let mut buffer = [0u8; 2];
                match stream.read_exact(&mut buffer[..]) {
                    Err(err)  => {println!("client::error::timeout while waiting for communication port::{:?}", err); None},
                    Ok(_)     => {
                        // open communication port
                        let port = u16::from_be_bytes(buffer);
                        println!("client::assigned port::{}", port);

                        match TcpStream::connect((ip, port)) {
                            Ok(stream) => {
                                println!("client::connection established");
                                stream.set_nodelay(true);
                                Some(Box::new(Connection{id: connection_id, port: port, stream: Some(stream)}))},
                            Err(err)   => {println!("client::error::could not connect to assigned communication port {} at {}::{:?}", port, ip, err); None},
                        }
                    },
                }
            },
            Err(err) => {println!("client::error::could not open port for communication request::{:?}", err); None},
        }
    }


    pub fn send_receive(&mut self, data_send: Vec<u8>) -> Vec<u8> {
        if let Some(stream) = self.stream.as_mut() {
//             println!("client::write data");

            let mut senddata = (data_send.len() as u32).to_be_bytes().to_vec();

            senddata.extend(data_send);

            stream.write(&senddata);
        } else {
            println!("client::the stream hasn't been initialized yet");
        }

        let mut datalengthbuffer = [0u8; 4];
        let mut databuffer = vec![];

        if let Some(stream) = self.stream.as_mut() {
            // clear databuffer
            //databuffer.drain(..);
            // read data length
            match stream.read_exact(&mut datalengthbuffer[..]) {
                Err(_)  => {println!("client::error::reading buffer");},
                Ok(_) => {
                    let bytes_to_read = u32::from_be_bytes(datalengthbuffer) as u64;
                    let mut data = stream.take(bytes_to_read);
                    match data.read_to_end(&mut databuffer) {
                        Ok(n) => assert_eq!(bytes_to_read as usize, n),
                        _ => panic!("client::didn't read enough"),
                    }
                },
            };
        } else {
            println!("client::the stream hasn't been initialized yet");
        }

//         println!("client::data received::{:?}", databuffer);
        databuffer
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.stream.as_mut().map(|stream| {
            println!("client::shutdown stream");
            stream.shutdown(Shutdown::Both);
        });
    }
}
