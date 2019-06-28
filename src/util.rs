/*
 * Copyright (C) 2018 Mathias Kraus <k.hias@gmx.de> - All Rights Reserved
 *
 * Unauthorized copying of this file, via any medium is strictly prohibited
 * Proprietary and confidential
 */

use std::io;
use std::io::prelude::*;
use std::io::{Error, ErrorKind};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::{thread, time};

pub fn bind(ip: Ipv4Addr, port: u16) -> Result<TcpListener, io::Error> {
    let port_in_use_sleep_milliseconds = time::Duration::from_millis(100);
    match TcpListener::bind((ip, port)) {
        Ok(listener) => Ok(listener),
        Err(ref e) if e.raw_os_error() == Some(0x62) => {
            // port already in use
            println!("server::bind port in use");
            thread::sleep(port_in_use_sleep_milliseconds);
            bind(ip, port)
        }
        Err(err) => Err(err),
    }
}

pub fn listener_accept_nonblocking(listener: TcpListener) -> io::Result<TcpStream> {
    listener.set_nonblocking(true)?; //we can not interrupt the accept call, therefore we have to set the listener to nonblocking and poll every 100ms accept
    for _ in 1..20 {
        match listener.accept() {
            Ok((stream, socket_address)) => {
                println!(
                    "server::transmission_handler::connection to address {}",
                    socket_address
                );
                if let Some(err) = stream.set_nonblocking(false).err() {
                    println!("client::error::failed to set tcp blocking: {:?}", err);
                }
                return Ok(stream);
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                println!("server::transmission_handler::waiting for connection");
                thread::sleep(time::Duration::from_millis(100))
            }
            Err(err) => {
                println!(
                    "server::error::transmission_handler at waiting for connection::{:?}",
                    err
                );
                return Err(err);
            }
        }
    }

    let err = Err(Error::new(
        ErrorKind::TimedOut,
        "timeour while waiting for connection",
    ));
    println!("server::error::{:?}", err);
    err
}

pub fn wait_for_transmission(stream: &mut TcpStream) -> io::Result<u64> {
    let mut datalengthbuffer = [0u8; 8];
    stream.read_exact(&mut datalengthbuffer[..])?;
    Ok(u64::from_be_bytes(datalengthbuffer))
}

pub fn read_transmission(stream: &mut TcpStream, payload_size: u64) -> io::Result<Vec<u8>> {
    let mut databuffer = Vec::<u8>::new();
    stream.take(payload_size).read_to_end(&mut databuffer)?;
    Ok(databuffer)
}

pub fn write_transmission(stream: &mut TcpStream, serialized: Vec<u8>) -> io::Result<usize> {
    let mut senddata = (serialized.len() as u64).to_be_bytes().to_vec();
    senddata.extend(serialized);

    stream.write(&senddata)
}
