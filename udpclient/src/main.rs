extern crate clap;
use clap::{App, Arg};

use std::io::ErrorKind;
use std::net::UdpSocket;
use std::thread::{self, JoinHandle};

use crossbeam_channel::{bounded, select, unbounded};

pub struct Socket {
    server: String,
    count: u32,
}

impl Socket {
    pub fn new(server: &str, count: u32) -> Self {
        Self {
            server: server.to_string(),
            count,
        }
    }

    pub fn run(&mut self, num: u32) {
        let mut handles = Vec::with_capacity(num as usize);
        for n in 0..num {
            let handle = self.start(n);
            handles.push(Some(handle));
        }

        for n in 0..handles.len() {
            let cnt = handles[n].take().unwrap().join().unwrap();
            println!("{}th Thread COUNTED: {}", n, cnt);
        }
    }

    pub fn start(&mut self, n: u32) -> JoinHandle<u32> {
        let count = self.count;

        let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
        socket.connect(&self.server).unwrap();
        socket.set_nonblocking(true).unwrap();
        let socket2 = socket.try_clone().unwrap();

        let (receiver_packet_tx, receiver_packet_rx) = unbounded::<Vec<u8>>();
        let (receiver_stopper_tx, receiver_stopper_rx) = bounded::<()>(1);

        let _ = thread::spawn(move || {
            let mut buf = [0u8; 1500];
            loop {
                select! {
                    recv(receiver_stopper_rx) -> _ => {
                        break;
                    },
                    default => {
                        match socket.recv(&mut buf) {
                            Ok(len) => {
                                let packet = buf[..len].to_vec();
                                // TODO count error
                                let _ = receiver_packet_tx.send(packet);
                                continue;
                            },
                            Err(e) => {
                                match e.kind() {
                                    ErrorKind::WouldBlock => {
                                        continue;
                                    },
                                    kind => {
                                        println!("encountered IO error: {:?}", kind);
                                    },
                                }
                            },
                        }
                    },
                }
            }
        });

        let (sender_packet_tx, sender_packet_rx) = unbounded::<Vec<u8>>();
        let (sender_stopper_tx, sender_stopper_rx) = bounded::<()>(1);

        let _ = thread::spawn(move || {
            let mut cnt: u64 = 0;
            loop {
                select! {
                    recv(sender_stopper_rx) -> _ => {
                        break;
                    },
                    recv(sender_packet_rx) -> msg => {
                        match msg {
                            Ok(packet) => {
                                'send: loop {
                                    match socket2.send(&packet) {
                                        Ok(_) => {
                                            cnt += 1;
                                            if cnt % 1000 == 0 {
                                                println!("SENT COUNT: {}", cnt);
                                            }
                                            break 'send;
                                        },
                                        Err(e) => {
                                            match e.kind() {
                                                ErrorKind::WouldBlock => {
                                                    continue 'send;
                                                },
                                                kind => {
                                                    println!("encountered IO error {:?}", kind);
                                                    break 'send;
                                                },
                                            }
                                        },
                                    }
                                }
                            },
                            Err(e) => {
                                println!("SENDER: channel error, {:?}", e);
                                break;
                            }
                        }
                    },
                }
            }
        });

        let (g_stopper_tx, g_stopper_rx) = bounded::<()>(1);

        let _ = thread::spawn(move || {
            let mut cnt: u32 = 0;
            let msg = &"0123456789".repeat(10);
            loop {
                select! {
                    recv(g_stopper_rx) -> _ => {
                        break;
                    },
                    default => {
                        if cnt < count {
                            let _ = sender_packet_tx.send(msg.as_bytes().to_vec());
                            cnt += 1;
                        }
                    }
                }
            }
            sender_stopper_tx.send(()).unwrap();
        });

        let counter: thread::JoinHandle<u32> = thread::spawn(move || {
            let mut cnt: u32 = 0;
            loop {
                select! {
                    recv(receiver_packet_rx) -> _ => {
                        cnt += 1;
                        match cnt % 100 {
                            0 => {
                                println!("CLIENT:{}:COUNTED: {}", n, cnt);
                            },
                            _ => {
                                // do nothing
                            },
                        }
                        if cnt == count {
                            println!("counter: reached max count, closing...");
                            break;
                        }
                    },
                }
            }
            let _ = g_stopper_tx.send(());
            let _ = receiver_stopper_tx.send(());
            cnt
        });

        return counter;
    }
}

fn main() {
    let app = App::new("UDP Client")
        .version("1.0")
        .author("lyokato")
        .about("UDP Client")
        .arg(
            Arg::with_name("server")
                .short("s")
                .long("server")
                .value_name("HOST:PORT")
                .help("server's address")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("count")
                .short("c")
                .long("count")
                .help("number of messages this client sends to server")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("sockets")
                .short("s")
                .long("sockets")
                .help("number of sockets")
                .required(true)
                .takes_value(true),
        );

    let matches = app.get_matches();
    let server = matches.value_of("server").unwrap();
    let count: u32 = matches.value_of("count").unwrap().parse().unwrap();
    let num: u32 = matches.value_of("sockets").unwrap().parse().unwrap();

    let now = ::std::time::Instant::now();

    let mut socket = Socket::new(server, count);
    socket.run(num);

    let elapsed = now.elapsed();
    println!(
        "{:7.3} sec",
        elapsed.as_secs() as f64 + elapsed.subsec_nanos() as f64 / 1e9
    );
}
