/*
 * Copyright (C) 2018 Mathias Kraus <k.hias@gmx.de> - All Rights Reserved
 *
 * Unauthorized copying of this file, via any medium is strictly prohibited
 * Proprietary and confidential
 */

use crate::rpc::RPC;

use serde::{Serialize, de::DeserializeOwned};

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
        let connection = TcpStream::connect((ip, 0xC390)).and_then(|mut stream| {
            let read_timeout = Some(time::Duration::from_secs(2));
            stream.set_read_timeout(read_timeout).err().map(|err| println!("client::error::failed to set tcp timeout: {:?}", err));

            let mut buffer = [0u8; 2];
            stream.read_exact(&mut buffer[..]).and_then(|_| {
                let port = u16::from_be_bytes(buffer);
                println!("client::assigned port::{}", port);
                TcpStream::connect((ip, port)).and_then(|stream| {
                    stream.set_nodelay(true).err().map(|err| println!("client::error::failed to set tcp nodelay: {:?}", err));
                    Ok(Box::new(Connection{id: connection_id, port, stream: Some(stream)}))
                })
            })
        });

        match connection {
            Ok(connection) => {println!("client::connection established"); Some(connection)},
            Err(err) => {println!("client::error::connection: {:?}", err); None},
        }
        //TODO use map_or_else once feature is in stable rust
        // connection.map_or_else(|err| { println!(""client::error::connection: {:?}", err); None }, |connection| { Some(connection) } )
    }

    pub fn transceive<Request: Serialize, Response: DeserializeOwned>(&mut self, request: Request) -> Option<Response> {
        let mut serde = bincode::config();

        let rpc = RPC { transmission_id: 42, data: request };
        let rpc = serde.big_endian().serialize(&rpc).unwrap();

        let rpc = self.send_receive(rpc);

        let rpc = serde.big_endian().deserialize::<RPC<Response>>(&rpc);

        match rpc {
            Ok(rpc) => Some(rpc.data),
            Err(err) => {println!("error deserializing response: {:?}", err); None},
        }
        //TODO use map_or_else once feature is in stable rust
        // rpc.map_or_else(|err| { println!("error deserializing response: {:?}", err); None }, |rpc| { Some(rpc.data) } )
    }

    fn send_receive(&mut self, serialized: Vec<u8>) -> Vec<u8> {
        let mut datalengthbuffer = [0u8; 4];
        let mut databuffer = Vec::<u8>::new();

        self.stream.as_mut().map(|stream| {
            let mut senddata = (serialized.len() as u32).to_be_bytes().to_vec();
            senddata.extend(serialized);

            stream.write(&senddata).map(|_| stream.read_exact(&mut datalengthbuffer[..])).and_then(|_| {
                let bytes_to_read = u32::from_be_bytes(datalengthbuffer) as u64;
                stream.take(bytes_to_read).read_to_end(&mut databuffer)
            }).err().map(|err| println!("client::error::transmission: {:?}", err));
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
