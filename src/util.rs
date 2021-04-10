use std::io;
use std::io::prelude::*;
use std::io::{Error, ErrorKind};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use std::{thread, time};

struct InterruptableSleep {
    predicate: Mutex<bool>,
    cond_var: Condvar,
}

struct Sleeper {
    resource: Arc<InterruptableSleep>,
}
struct Interrupter {
    resource: Arc<InterruptableSleep>,
}

impl InterruptableSleep {
    fn new() -> (Sleeper, Interrupter) {
        let resource = Arc::new(InterruptableSleep {
            predicate: Mutex::new(false),
            cond_var: Condvar::new(),
        });
        (
            Sleeper {
                resource: resource.clone(),
            },
            Interrupter { resource },
        )
    }
}

impl Sleeper {
    // returns true if sleep was finished, false if interrupted
    fn sleep(self, timeout: Duration) -> bool {
        let mut predicate = self.resource.predicate.lock().expect("getting lock");
        while *predicate == false {
            let (pred, result) = self
                .resource
                .cond_var
                .wait_timeout(predicate, timeout)
                .expect("getting lock");
            predicate = pred;
            if result.timed_out() {
                return !*predicate;
            }
        }
        *predicate = true;
        false
    }
}

impl Interrupter {
    // returns true if interrupt was successful, false if sleep already finished
    fn interrupt(self) -> bool {
        {
            let mut predicate = self.resource.predicate.lock().expect("getting lock");
            if *predicate {
                return false;
            }
            *predicate = true;
        }
        self.resource.cond_var.notify_one();
        true
    }
}

pub fn bind(ip: Ipv4Addr, port: u16) -> Result<TcpListener, io::Error> {
    let port_in_use_sleep_milliseconds = time::Duration::from_millis(100);
    match TcpListener::bind((ip, port)) {
        Ok(listener) => Ok(listener),
        Err(ref e) if e.raw_os_error() == Some(0x62) => {
            // port already in use
            log::warn!("port in use");
            thread::sleep(port_in_use_sleep_milliseconds);
            bind(ip, port)
        }
        Err(err) => Err(err),
    }
}

pub fn listener_accept_nonblocking(
    listener: TcpListener,
    timeout: Duration,
) -> io::Result<TcpStream> {
    listener.set_nonblocking(false)?;
    let addr = listener.local_addr()?;
    let (sleeper, interrupter) = InterruptableSleep::new();
    // the thread returns None if the sleeper was interupted, the local address if not interrupted
    let th = thread::spawn(move || {
        if sleeper.sleep(timeout) {
            match TcpStream::connect_timeout(&addr, time::Duration::from_millis(1)) {
                Ok(stream) => stream.local_addr().ok(),
                _ => None,
            }
        } else {
            None
        }
    });

    let (stream, address) = listener.accept()?;
    interrupter.interrupt();

    match th.join() {
        Ok(None) => Ok(stream),                          // sleep was interrupted
        Ok(Some(addr)) if addr != address => Ok(stream), // sleep was not interrupted but also didn't interrupt the listener; this should actually never happen
        _ => {
            let err = Err(Error::new(
                ErrorKind::TimedOut,
                "timeour while waiting for connection",
            ));
            log::error!("{:?}", err);
            err
        }
    }
}

pub fn adjust_stream(stream: &TcpStream, read_timeout: Option<time::Duration>) -> io::Result<()> {
    stream.set_read_timeout(read_timeout).map_err(|err| {
        log::error!("failed to set read timeout on tcp stream: {:?}", err);
        err
    })?;
    stream.set_nodelay(true).map_err(|err| {
        log::error!("failed to set nodelay on tcp stream: {:?}", err);
        err
    })?;
    stream.set_nonblocking(false).map_err(|err| {
        log::error!("failed to set blocking read on tcp stream: {:?}", err);
        err
    })?;
    Ok(())
}

pub fn wait_for_transmission(stream: &mut TcpStream) -> io::Result<u64> {
    let mut datalengthbuffer = [0u8; 8];
    stream
        .read_exact(&mut datalengthbuffer[..])
        .map_err(|err| {
            log::error!("waiting for transmission: {:?}", err);
            err
        })?;
    Ok(u64::from_be_bytes(datalengthbuffer))
}

pub fn read_transmission(stream: &mut TcpStream, payload_size: u64) -> io::Result<Vec<u8>> {
    let mut databuffer = vec![0u8; payload_size as usize];
    stream.read_exact(&mut databuffer[..]).map_err(|err| {
        log::error!("waiting for transmission: {:?}", err);
        err
    })?;
    Ok(databuffer)
}

pub fn write_transmission(stream: &mut TcpStream, serialized: Vec<u8>) -> io::Result<usize> {
    let mut senddata = (serialized.len() as u64).to_be_bytes().to_vec();
    senddata.extend(serialized);

    stream.write(&senddata).map_err(|err| {
        log::error!("writing transmission: {:?}", err);
        err
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_common::TEST_PORT_BASE;

    use std::net::SocketAddr;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;
    use std::time::{Duration, SystemTime};

    #[test]
    fn interruptable_sleep_finish() {
        let (sleeper, _) = InterruptableSleep::new();
        let start = SystemTime::now();
        let th = thread::spawn(move || {
            assert!(sleeper.sleep(Duration::from_millis(50)));
        });
        assert!(th.join().is_ok());
        match start.elapsed() {
            Ok(elapsed) => assert!(elapsed < Duration::from_millis(100)),
            _ => assert!(false),
        }
    }

    #[test]
    fn interruptable_sleep_interrupted() {
        let (sleeper, interrupter) = InterruptableSleep::new();
        let start = SystemTime::now();
        let th = thread::spawn(move || {
            assert!(!sleeper.sleep(Duration::from_millis(2000)));
        });
        thread::sleep(Duration::from_millis(10));
        interrupter.interrupt();
        assert!(th.join().is_ok());
        match start.elapsed() {
            Ok(elapsed) => assert!(elapsed < Duration::from_millis(100)),
            _ => assert!(false),
        }
    }

    #[test]
    fn bind_port() {
        let ip = Ipv4Addr::UNSPECIFIED;
        let port = TEST_PORT_BASE.fetch_add(1, Ordering::Relaxed);
        let start = SystemTime::now();
        assert!(bind(ip, port).is_ok());
        match start.elapsed() {
            Ok(elapsed) => assert!(elapsed < Duration::from_millis(20)),
            _ => assert!(false),
        }
    }

    #[test]
    fn bind_port_in_use() {
        let ip = Ipv4Addr::UNSPECIFIED;
        let port = TEST_PORT_BASE.fetch_add(1, Ordering::Relaxed);
        let listener = bind(ip, port);
        assert!(listener.is_ok());

        let th = std::thread::spawn(move || {
            thread::sleep(Duration::from_millis(110));
            drop(listener);
        });

        let start = SystemTime::now();
        assert!(bind(ip, port).is_ok());
        match start.elapsed() {
            Ok(elapsed) => assert!(elapsed > Duration::from_millis(90)),
            _ => assert!(false),
        }

        assert!(th.join().is_ok());
    }

    #[test]
    fn listener_accept_success() {
        let ip = Ipv4Addr::UNSPECIFIED;
        let port = TEST_PORT_BASE.fetch_add(1, Ordering::Relaxed);
        let listener = bind(ip, port);
        assert!(listener.is_ok());

        if let Ok(listener) = listener {
            let th = std::thread::spawn(move || {
                let addr = SocketAddr::from((ip, port));
                let _ = TcpStream::connect_timeout(&addr, time::Duration::from_millis(100));
            });

            assert!(listener_accept_nonblocking(listener, Duration::from_millis(100)).is_ok());
            assert!(th.join().is_ok());
        }
    }

    #[test]
    fn listener_accept_failure() {
        let ip = Ipv4Addr::UNSPECIFIED;
        let port = TEST_PORT_BASE.fetch_add(1, Ordering::Relaxed);
        let listener = bind(ip, port);
        assert!(listener.is_ok());

        if let Ok(listener) = listener {
            assert!(listener_accept_nonblocking(listener, Duration::from_millis(100)).is_err());
        }
    }

    #[test]
    fn adjust_stream_for_blocking_read() {
        let ip = Ipv4Addr::UNSPECIFIED;
        let port = TEST_PORT_BASE.fetch_add(1, Ordering::Relaxed);
        let listener = bind(ip, port);
        assert!(listener.is_ok());

        if let Ok(listener) = listener {
            let ready = Arc::new(AtomicBool::new(false));
            let ready_to_send = ready.clone();
            let th = std::thread::spawn(move || {
                let addr = SocketAddr::from((ip, port));
                let tcp_stream =
                    TcpStream::connect_timeout(&addr, time::Duration::from_millis(200));
                while !ready_to_send.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(1));
                }
                thread::sleep(Duration::from_millis(100));
                if let Ok(mut tcp_stream) = tcp_stream {
                    let _ = tcp_stream.write(&[0x42]);
                }
            });

            let tcp_stream = listener_accept_nonblocking(listener, Duration::from_millis(100));
            assert!(tcp_stream.is_ok());

            if let Ok(mut tcp_stream) = tcp_stream {
                assert!(adjust_stream(&tcp_stream, Some(Duration::from_millis(200))).is_ok());
                match tcp_stream.nodelay() {
                    Ok(true) => (),
                    _ => assert!(false),
                }
                let mut buffer = [0u8; 1];
                ready.store(true, Ordering::Relaxed);
                assert!(tcp_stream.read_exact(&mut buffer[..]).is_ok());
            }

            assert!(th.join().is_ok());
        }
    }

    #[test]
    fn wait_for_transmission_success() {
        let ip = Ipv4Addr::UNSPECIFIED;
        let port = TEST_PORT_BASE.fetch_add(1, Ordering::Relaxed);
        let listener = bind(ip, port);
        assert!(listener.is_ok());

        const DATA_LENGTH: u64 = 42;
        let send_data = u64::to_be_bytes(DATA_LENGTH);

        if let Ok(listener) = listener {
            let th = thread::spawn(move || {
                let addr = SocketAddr::from((ip, port));
                assert!(
                    TcpStream::connect_timeout(&addr, Duration::from_millis(100))
                        .map(|mut writer| writer.write(&send_data))
                        .is_ok()
                );
            });

            assert!(
                listener_accept_nonblocking(listener, Duration::from_millis(100))
                    .and_then(|mut reader| adjust_stream(
                        &mut reader,
                        Some(Duration::from_millis(100))
                    )
                    .and_then(|_| Ok(reader)))
                    .and_then(|mut reader| wait_for_transmission(&mut reader))
                    .map(|data_length| {
                        assert_eq!(data_length, DATA_LENGTH);
                        Result::<(), ()>::Ok(())
                    })
                    .is_ok()
            );

            assert!(th.join().is_ok());
        }
    }

    #[test]
    fn wait_for_transmission_failure() {
        let ip = Ipv4Addr::UNSPECIFIED;
        let port = TEST_PORT_BASE.fetch_add(1, Ordering::Relaxed);
        let listener = bind(ip, port);
        assert!(listener.is_ok());

        const DATA_LENGTH: u32 = 42;
        let send_data = u32::to_be_bytes(DATA_LENGTH);

        if let Ok(listener) = listener {
            let th = thread::spawn(move || {
                let addr = SocketAddr::from((ip, port));
                assert!(
                    TcpStream::connect_timeout(&addr, Duration::from_millis(100))
                        .map(|mut writer| writer.write(&send_data))
                        .is_ok()
                );
            });

            assert!(
                listener_accept_nonblocking(listener, Duration::from_millis(100))
                    .and_then(|mut reader| adjust_stream(
                        &mut reader,
                        Some(Duration::from_millis(100))
                    )
                    .and_then(|_| Ok(reader)))
                    .and_then(|mut reader| wait_for_transmission(&mut reader))
                    .is_err()
            );

            assert!(th.join().is_ok());
        }
    }

    #[test]
    fn read_transmission_success() {
        let ip = Ipv4Addr::UNSPECIFIED;
        let port = TEST_PORT_BASE.fetch_add(1, Ordering::Relaxed);
        let listener = bind(ip, port);
        assert!(listener.is_ok());

        const DATA_LENGTH: u64 = 8;
        const SEND_DATA: u64 = 73;
        let send_data = u64::to_be_bytes(SEND_DATA);

        if let Ok(listener) = listener {
            let th = thread::spawn(move || {
                let addr = SocketAddr::from((ip, port));
                assert!(
                    TcpStream::connect_timeout(&addr, Duration::from_millis(100))
                        .map(|mut writer| writer.write(&send_data))
                        .is_ok()
                );
            });

            assert!(
                listener_accept_nonblocking(listener, Duration::from_millis(100))
                    .and_then(|mut reader| adjust_stream(
                        &mut reader,
                        Some(Duration::from_millis(100))
                    )
                    .and_then(|_| Ok(reader)))
                    .and_then(|mut reader| read_transmission(&mut reader, DATA_LENGTH))
                    .map(|payload| {
                        assert_eq!(payload.len(), DATA_LENGTH as usize);
                        assert_eq!(payload, send_data);
                        Result::<(), ()>::Ok(())
                    })
                    .is_ok()
            );

            assert!(th.join().is_ok());
        }
    }

    #[test]
    fn read_transmission_failure() {
        let ip = Ipv4Addr::UNSPECIFIED;
        let port = TEST_PORT_BASE.fetch_add(1, Ordering::Relaxed);
        let listener = bind(ip, port);
        assert!(listener.is_ok());

        const DATA_LENGTH: u64 = 13;
        const SEND_DATA: u64 = 73;
        let send_data = u64::to_be_bytes(SEND_DATA);

        if let Ok(listener) = listener {
            let th = thread::spawn(move || {
                let addr = SocketAddr::from((ip, port));
                assert!(
                    TcpStream::connect_timeout(&addr, Duration::from_millis(100))
                        .map(|mut writer| writer.write(&send_data))
                        .is_ok()
                );
            });

            assert!(
                listener_accept_nonblocking(listener, Duration::from_millis(100))
                    .and_then(|mut reader| adjust_stream(
                        &mut reader,
                        Some(Duration::from_millis(1000))
                    )
                    .and_then(|_| Ok(reader)))
                    .and_then(|mut reader| { read_transmission(&mut reader, DATA_LENGTH) })
                    .is_err()
            );

            assert!(th.join().is_ok());
        }
    }

    #[test]
    fn write_transmission_success() {
        let ip = Ipv4Addr::UNSPECIFIED;
        let port = TEST_PORT_BASE.fetch_add(1, Ordering::Relaxed);
        let listener = bind(ip, port);
        assert!(listener.is_ok());

        const DATA_LENGTH: u64 = 8;
        const SEND_DATA: u64 = 73;
        let payload = u64::to_be_bytes(SEND_DATA);

        let mut expected_data = Vec::<u8>::new();
        expected_data.extend(u64::to_be_bytes(DATA_LENGTH).to_vec());
        expected_data.extend(payload.to_vec());

        if let Ok(listener) = listener {
            let th = thread::spawn(move || {
                let addr = SocketAddr::from((ip, port));
                assert!(
                    TcpStream::connect_timeout(&addr, Duration::from_millis(100))
                        .and_then(|mut writer| adjust_stream(
                            &mut writer,
                            Some(Duration::from_millis(100))
                        )
                        .and_then(|_| Ok(writer)))
                        .map(|mut writer| write_transmission(&mut writer, payload.to_vec()))
                        .is_ok()
                );
            });

            assert!(
                listener_accept_nonblocking(listener, Duration::from_millis(100))
                    .and_then(|mut reader| adjust_stream(
                        &mut reader,
                        Some(Duration::from_millis(100))
                    )
                    .and_then(|_| Ok(reader)))
                    .and_then(|mut reader| {
                        let mut databuffer = vec![0u8; expected_data.len()];
                        reader
                            .read_exact(&mut databuffer)
                            .map(|_| assert_eq!(databuffer, expected_data))
                    })
                    .is_ok()
            );

            assert!(th.join().is_ok());
        }
    }
}
