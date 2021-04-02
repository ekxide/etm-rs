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
use std::time::Duration;

pub trait MessageProcessing: Send + Sync {
    type Rq;
    type Rsp;
    type E;

    fn new() -> Arc<Self>;

    fn setup(&self, connection_info: String, connection_id: u32) {
        // default implementation das nothing
        log::trace!(
            "default implementation for MessageProcessing::setup: {} : {}",
            connection_info,
            connection_id
        );
    }

    fn execute(&self, connection_id: u32, rpc: Self::Rq) -> Result<Self::Rsp, Self::E>;

    fn cleanup(&self, connection_info: String, connection_id: u32) {
        // default implementation das nothing
        log::trace!(
            "default implementation for MessageProcessing::cleanup: {} : {}",
            connection_info,
            connection_id
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

#[derive(PartialEq)]
enum TransceiveLoopAction {
    Stop,
    Continue,
}

impl<Req, Resp, Error, T> Server<T>
where
    Req: DeserializeOwned,
    Resp: Serialize,
    Error: Serialize + std::fmt::Debug,
    T: 'static + MessageProcessing<Rq = Req, Rsp = Resp, E = Error>,
{
    pub fn new(port: u16, service: Service) -> Self {
        Server {
            message_processing: T::new(),
            port,
            service,
        }
    }

    pub fn run(&self) -> io::Result<()> {
        log::info!("run");

        let ip = Ipv4Addr::UNSPECIFIED;

        // bind port
        let listener = util::bind(ip, self.port)?;

        let mut serde = bincode::config();

        for stream in listener.incoming() {
            let _ =
                || -> io::Result<()> { self.handle_mgmt_request(stream?, serde.big_endian()) }()
                    .map_err(|err| log::error!("mgmt request: {:?}", err));
        }
        log::info!("run -> stop");
        Ok(())
    }

    fn handle_mgmt_request(
        &self,
        mut stream: TcpStream,
        serde: &mut bincode::Config,
    ) -> io::Result<()> {
        util::adjust_stream(&stream, None)?;

        const DUMMY_CONNECTION_ID: u32 = 0;
        Self::handle_request(&mut stream, serde, self, DUMMY_CONNECTION_ID).and_then(|_| Ok(()))
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
        let stream = &mut util::listener_accept_nonblocking(listener, Duration::from_secs(2))?;
        util::adjust_stream(stream, None)?;

        message_processing.setup(
            "TODO: ip address:".to_string() + &local_port.to_string(),
            connection_id,
        );

        let mut serde = bincode::config();
        let serde = serde.big_endian();

        let mut running = TransceiveLoopAction::Continue;
        while running == TransceiveLoopAction::Continue {
            running = Self::handle_request(stream, serde, &*message_processing, connection_id)
                .map_err(|err| log::error!("transmission error: {:?}", err))
                .unwrap_or(TransceiveLoopAction::Stop);
        }

        message_processing.cleanup(
            "TODO: ip address:".to_string() + &local_port.to_string(),
            connection_id,
        );

        log::debug!("end message processing transceiver");
        Ok(())
    }

    fn handle_request<Rq, Rsp, E, U>(
        stream: &mut TcpStream,
        serde: &mut bincode::Config,
        executor: &U,
        connection_id: u32,
    ) -> io::Result<TransceiveLoopAction>
    where
        Rq: DeserializeOwned,
        Rsp: Serialize,
        E: Serialize + std::fmt::Debug,
        U: Executor<Rq = Rq, Rsp = Rsp, E = E>,
    {
        let payload_size = util::wait_for_transmission(stream)?;
        let payload = util::read_transmission(stream, payload_size)?;

        let (tid, r#type) = payload.split_at(8 as usize);
        let transmission_id =
            u64::from_be_bytes(<[u8; 8]>::try_from(tid).expect("transmission id"));

        let request = serde
            .deserialize::<transport::Type<Rq>>(r#type)
            .expect("deserializing request"); //TODO error handling

        match request {
            transport::Type::Request(cmd) => {
                let response = executor.execute(connection_id, cmd);

                match response {
                    Ok(response) => {
                        let response = transport::Transmission {
                            id: transmission_id,
                            r#type: transport::Type::Response(response),
                        };
                        let serialized = serde.serialize(&response).unwrap();
                        util::write_transmission(stream, serialized)?;
                    }
                    Err(err) => {
                        let response = transport::Transmission {
                            id: transmission_id,
                            r#type: transport::Type::Error(err),
                        };
                        let serialized = serde.serialize(&response).unwrap();
                        util::write_transmission(stream, serialized)?;
                    }
                }
            }
            transport::Type::End => {
                log::trace!("end request");
                return Ok(TransceiveLoopAction::Stop);
            }
            _ => {
                let response = transport::Transmission {
                    id: transmission_id,
                    r#type: transport::Type::Error("Not a request!".to_string()),
                };
                let serialized = serde.serialize(&response).unwrap();
                util::write_transmission(stream, serialized)?;
            }
        }

        Ok(TransceiveLoopAction::Continue)
    }
}

impl<Req, Resp, Error, T> Executor for Server<T>
where
    Req: DeserializeOwned,
    Resp: Serialize,
    Error: Serialize + std::fmt::Debug,
    T: 'static + MessageProcessing<Rq = Req, Rsp = Resp, E = Error>,
{
    type Rq = mgmt::Request;
    type Rsp = mgmt::Response;
    type E = transport::Error;
    fn execute(&self, _connection_id: u32, rpc: Self::Rq) -> Result<Self::Rsp, Self::E> {
        match rpc {
            mgmt::Request::Identify { protocol_version } => {
                log::debug!("server::identify request");
                let server_protocol_version = ProtocolVersion::entity().version();
                if server_protocol_version != protocol_version {
                    log::warn!("server::identify -> incompatible protocol versions; server: {}, client: {}", server_protocol_version, protocol_version);
                }
                Ok(mgmt::Response::Identify(mgmt::Identity {
                    protocol_version: server_protocol_version,
                    service: self.service.clone(),
                }))
            }
            mgmt::Request::Connect(params) => {
                log::debug!("server::connection request");
                let port = self.connection_request(params.connection_id).unwrap();
                Ok(mgmt::Response::Connect(mgmt::CommSettings {
                    connection_id: params.connection_id,
                    port,
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde::{Deserialize, Serialize};
    use std::net::SocketAddr;
    use std::sync::atomic::{AtomicU16, Ordering};

    #[derive(Serialize, Deserialize, Debug)]
    struct Dummy {}

    #[derive(Serialize, Deserialize, Debug)]
    enum DummyRequest {
        Ping,
    }

    #[derive(Serialize, Deserialize, Debug)]
    enum DummyResponse {
        Pong,
    }

    impl MessageProcessing for Dummy {
        type Rq = DummyRequest;
        type Rsp = DummyResponse;
        type E = String;

        fn new() -> Arc<Self> {
            Arc::new(Dummy {})
        }

        fn execute(&self, _connection_id: u32, _rpc: Self::Rq) -> Result<Self::Rsp, Self::E> {
            Ok(DummyResponse::Pong)
        }
    }

    static TEST_PORT_BASE: AtomicU16 = AtomicU16::new(6000);

    #[test]
    fn identify_request() -> io::Result<()> {
        let ip = Ipv4Addr::UNSPECIFIED;
        let port = TEST_PORT_BASE.fetch_add(1, Ordering::Relaxed);
        let listener = util::bind(ip, port)?;

        let th = thread::spawn(move || {
            let mut serde = bincode::config();
            let serde = serde.big_endian();

            const EXPECTED_ETM_PROTOCOL_VERSION: u32 = 0;
            let identify = transport::Transmission {
                id: 0,
                r#type: transport::Type::Request(mgmt::Request::Identify {
                    protocol_version: EXPECTED_ETM_PROTOCOL_VERSION,
                }),
            };
            let identify = serde.serialize(&identify).unwrap();

            let addr = SocketAddr::from((ip, port));
            if let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(100)) {
                util::adjust_stream(&mut stream, Some(Duration::from_millis(100)))?;
                util::write_transmission(&mut stream, identify)?;
                let payload_length = util::wait_for_transmission(&mut stream)?;
                let response = util::read_transmission(&mut stream, payload_length)?;
                let identity = serde
                    .deserialize::<transport::Transmission<mgmt::Response>>(&response)
                    .unwrap();
                match identity.r#type {
                    transport::Type::Response(mgmt::Response::Identify(identity)) => {
                        assert!(identity.protocol_version == 0)
                    }
                    _ => assert!(false),
                }
            }

            Ok::<(), io::Error>(())
        });

        let stream = util::listener_accept_nonblocking(listener, Duration::from_millis(100))?;

        let service = Service::entity("TestService".to_string(), 1);
        let server = Server::<Dummy>::new(port, service);

        let mut serde = bincode::config();
        let serde = serde.big_endian();
        server.handle_mgmt_request(stream, serde)?;

        assert!(th.join().is_ok());
        Ok(())
    }
}
