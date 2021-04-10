use super::*;
use crate::test_common::TEST_PORT_BASE;

use serde::{Deserialize, Serialize};

use std::io;
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};

struct DummyServer {
    shutdown_request: Arc<AtomicBool>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
enum DummyRequest {
    Ping,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
enum DummyResponse {
    Pong,
}

impl server::MessageProcessing for DummyServer {
    type Rq = DummyRequest;
    type Rsp = DummyResponse;
    type E = String;

    fn new() -> Arc<Self> {
        Arc::new(DummyServer {
            shutdown_request: Arc::new(AtomicBool::new(false)),
        })
    }

    fn execute(&self, _connection_id: u32, _rpc: Self::Rq) -> Result<Self::Rsp, Self::E> {
        Ok(DummyResponse::Pong)
    }

    fn shutdown(&self) -> bool {
        self.shutdown_request.load(Ordering::Relaxed)
    }
}

type Connection = client::Connection<DummyRequest, DummyResponse, String>;

#[test]
fn simple_request() -> io::Result<()> {
    let ip = Ipv4Addr::UNSPECIFIED;
    let port = TEST_PORT_BASE.fetch_add(1, Ordering::Relaxed);

    let service = Service::entity("TestService".to_string(), 1);

    let server = server::Server::<DummyServer>::new(port, service);

    let th = {
        let shutdown_request = server.message_processing.shutdown_request.clone();
        thread::spawn(move || {
            const EXIT_FAILURE: i32 = 1;

            let mut retries = 100;
            let mut connection = loop {
                if let Some(connection) = Connection::new(ip, port, 1) {
                    break connection;
                } else if retries > 0 {
                    retries -= 1;
                    thread::sleep(Duration::from_millis(10));
                } else {
                    std::process::exit({
                        eprintln!("could not connect to server");
                        EXIT_FAILURE
                    });
                }
            };

            assert_eq!(
                connection.transceive(DummyRequest::Ping),
                Some(DummyResponse::Pong)
            );

            // shutdown server
            shutdown_request.store(true, Ordering::Relaxed);

            let addr = SocketAddr::from((ip, port));
            if Connection::mgmt_transceive(&addr, mgmt::Request::CheckRunState)
                != Some(mgmt::Response::CheckRunState)
            {
                std::process::exit({
                    eprintln!("requesting to check server run state failed");
                    EXIT_FAILURE
                });
            }

            Ok::<(), io::Error>(())
        })
    };

    server.run()?;

    assert!(th.join().is_ok());

    Ok(())
}
