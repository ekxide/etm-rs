use crate::mgmt;
use crate::transport;
use crate::util;
use crate::{ProtocolVersion, Service};

use bincode::Options;
use serde::{de::DeserializeOwned, Serialize};

use std::io;
use std::marker::PhantomData;
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpStream};
use std::time;

#[derive(Debug)]
pub struct Connection<Req, Resp, Error>
where
    Req: Serialize,
    Resp: DeserializeOwned + std::fmt::Debug,
    Error: DeserializeOwned + std::fmt::Debug,
{
    id: u32,
    port: u16,
    stream: TcpStream,
    server_protocol_version: u32,
    server_service: Service,
    _req: PhantomData<Req>,
    _resp: PhantomData<Resp>,
    _error: PhantomData<Error>,
}

impl<Req, Resp, Error> Connection<Req, Resp, Error>
where
    Req: Serialize,
    Resp: DeserializeOwned + std::fmt::Debug,
    Error: DeserializeOwned + std::fmt::Debug,
{
    pub fn new(
        ip: Ipv4Addr,
        service_management_port: u16,
        connection_id: i32,
    ) -> Option<Box<Connection<Req, Resp, Error>>> {
        let addr = SocketAddr::from((ip, service_management_port));

        let protocol_version = ProtocolVersion::entity().version();
        let identify = mgmt::Request::Identify { protocol_version };

        let response = Self::mgmt_transceive(&addr, identify)?;
        let identity = if let mgmt::Response::Identify(identity) = response {
            Some(identity)
        } else {
            log::error!("wrong response to Identify");
            None
        }?;

        let comm_params = mgmt::Request::Connect(mgmt::CommParams {
            protocol_version,
            connection_id: std::u32::MAX,
            rpc_interval_timeout_ms: std::u32::MAX,
        });
        let response = Self::mgmt_transceive(&addr, comm_params)?;
        let comm_settings = if let mgmt::Response::Connect(comm_settings) = response {
            Some(comm_settings)
        } else {
            log::error!("wrong response to Connect");
            None
        }?;

        log::info!("assigned port: {}", comm_settings.port);
        let addr = SocketAddr::from((ip, comm_settings.port));
        let stream = TcpStream::connect_timeout(&addr, time::Duration::from_secs(2))
            .map_err(|err| {
                log::error!("failed to open communication port: {:?}", err);
                err
            })
            .ok()?;

        util::adjust_stream(&stream, None).ok()?;

        log::info!("connected to service: '{}'", identity.service.id());
        Some(Box::new(Connection::<Req, Resp, Error> {
            id: comm_settings.connection_id,
            port: comm_settings.port,
            stream: stream,
            server_protocol_version: identity.protocol_version,
            server_service: identity.service,
            _req: PhantomData,
            _resp: PhantomData,
            _error: PhantomData,
        }))
    }

    pub(crate) fn mgmt_transceive(addr: &SocketAddr, req: mgmt::Request) -> Option<mgmt::Response> {
        let mut stream = TcpStream::connect_timeout(addr, time::Duration::from_secs(2))
            .map_err(|err| {
                log::error!("failed to open tcp port: {:?}", err);
                err
            })
            .ok()?;
        let read_timeout = Some(time::Duration::from_secs(2));
        util::adjust_stream(&stream, read_timeout).ok()?;

        Self::transceive_generic::<mgmt::Request, mgmt::Response, transport::Error>(
            &mut stream,
            req,
        )
    }

    pub fn compatibility_check(&self, service: Service) -> bool {
        let mut compatiblity = true;
        let protocol_version = ProtocolVersion::entity().version();

        if protocol_version != self.server_protocol_version {
            compatiblity = false;
            log::error!(
                "incompatible ETM versions detected! client on v{} and server on v{}!",
                protocol_version,
                self.server_protocol_version
            );
        }

        if service.id() != self.server_service.id() {
            compatiblity = false;
            log::error!(
                "incompatible Services detected! client expects '{}' and server supplies '{}'!",
                service.id(),
                self.server_service.id()
            );
        }

        if service.protocol_version() != self.server_service.protocol_version() {
            compatiblity = false;
            log::error!(
                "incompatible ETM versions detected! client on v{} and server on v{}!",
                service.protocol_version(),
                self.server_service.protocol_version()
            );
        }

        compatiblity
    }

    pub fn transceive(&mut self, request: Req) -> Option<Resp> {
        Self::transceive_generic::<Req, Resp, Error>(&mut self.stream, request)
    }

    fn transceive_generic<Rq, Rsp, E>(stream: &mut TcpStream, request: Rq) -> Option<Rsp>
    where
        Rq: Serialize,
        Rsp: DeserializeOwned + std::fmt::Debug,
        E: DeserializeOwned + std::fmt::Debug,
    {
        let serde = bincode::DefaultOptions::new()
            .with_big_endian()
            .with_fixint_encoding();

        let transmission = transport::Transmission {
            id: 42,
            r#type: transport::Type::Request(request),
        };

        let transmission = serde
            .serialize(&transmission)
            .map_err(|err| {
                log::error!("serializing request: {:?}", err);
                err
            })
            .ok()?;

        let response = Self::send_receive(stream, transmission).ok()?;

        let response = serde
            .deserialize::<transport::Transmission<Rsp>>(&response)
            .map_err(|err| {
                log::error!("deserializing response: {:?}", err);
                err
            })
            .ok()?;

        match response.r#type {
            transport::Type::Response(response) => Some(response),
            transport::Type::Error(err) => {
                log::error!("response: {:?}", err);
                None
            }
            unexpected @ _ => {
                log::error!("unexpected response: {:?}", unexpected);
                None
            }
        }
        //TODO use map_or_else once feature is in stable rust
        // response.map_or_else(|err| { log::error!("deserializing response: {:?}", err); None }, |response| { Some(response.r#type) } )
    }

    fn send_receive(stream: &mut TcpStream, serialized: Vec<u8>) -> io::Result<Vec<u8>> {
        util::write_transmission(stream, serialized)?;
        let payload_size = util::wait_for_transmission(stream)?;
        util::read_transmission(stream, payload_size)
    }
}

impl<Req, Resp, Error> Drop for Connection<Req, Resp, Error>
where
    Req: Serialize,
    Resp: DeserializeOwned + std::fmt::Debug,
    Error: DeserializeOwned + std::fmt::Debug,
{
    fn drop(&mut self) {
        log::info!("shutdown stream");

        let serde = bincode::DefaultOptions::new()
            .with_big_endian()
            .with_fixint_encoding();

        let transmission = transport::Transmission::<()> {
            id: 42,
            r#type: transport::Type::End,
        };

        let transmission = serde
            .serialize(&transmission)
            .map_err(|err| {
                log::error!("serializing request: {:?}", err);
                err
            })
            .unwrap();

        if let Err(err) = util::write_transmission(&mut self.stream, transmission) {
            log::error!("sending transmission end: {:?}", err);
        }

        if let Err(err) = self.stream.shutdown(Shutdown::Both) {
            log::error!("shutdown stream: {:?}", err);
        }
    }
}
