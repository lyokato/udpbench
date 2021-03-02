use std::convert::TryInto;
use std::net::{SocketAddr, UdpSocket};
use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Barrier};
use std::thread::{self, JoinHandle};

use crossbeam_channel::{bounded, select, unbounded, Sender};
use nix::sched::{sched_setaffinity, CpuSet};
use nix::unistd::gettid;
use socket2::{Domain, Protocol, Socket, Type};

use crate::result::{ErrorKind, Result};

extern "C" {
    fn attach_reuseport_cbpf(fd: libc::c_int, group_size: libc::c_ushort) -> libc::c_int;
    fn attach_reuseport_ebpf(fd: libc::c_int, group_size: libc::c_ushort) -> libc::c_int;
}

#[derive(Eq, PartialEq)]
enum ClusterState {
    Idle,
    Started,
    Closed,
}

pub struct UdpSocketCluster {
    cpu_set: CpuSet,
    num_node: usize,
    r_handles: Vec<Option<JoinHandle<()>>>,
    r_closers: Vec<Sender<()>>,
    c_closer: Option<Sender<()>>,
    c_handle: Option<JoinHandle<()>>,
    barrier: Arc<Barrier>,
    state: ClusterState,
    use_cbpf: bool,
    use_pinning: bool,
}

impl Default for UdpSocketCluster {
    fn default() -> Self {
        Self::new(num_cpus::get(), true, true)
    }
}

impl UdpSocketCluster {
    fn build_socket(addr: &str) -> Result<UdpSocket> {
        let addr = addr
            .parse::<SocketAddr>()
            .map_err(|_| ErrorKind::BadAddress)?;

        let domain = if addr.is_ipv4() {
            Domain::ipv4()
        } else {
            Domain::ipv6()
        };

        let sock = Socket::new(domain, Type::dgram(), Some(Protocol::udp()))
            .map_err(ErrorKind::SocketBuildFailure)?;

        sock.set_reuse_address(true)
            .map_err(ErrorKind::CantSetOptionReuseAddress)?;

        sock.set_reuse_port(true)
            .map_err(ErrorKind::CantSetOptionReusePort)?;

        sock.set_nonblocking(true)
            .map_err(ErrorKind::CantSetOptionNonBlocking)?;

        println!("bind addr: {}", addr);
        sock.bind(&addr.into())
            .map_err(ErrorKind::SocketBindFailure)?;

        let std_sock = sock.into_udp_socket();

        Ok(std_sock)
    }

    pub fn new(num_node: usize, use_cbpf: bool, use_pinning: bool) -> Self {
        Self {
            cpu_set: CpuSet::new(),
            num_node,
            r_handles: Vec::with_capacity(num_node),
            r_closers: Vec::with_capacity(num_node),
            c_closer: None,
            c_handle: None,
            barrier: Arc::new(Barrier::new(num_node)),
            state: ClusterState::Idle,
            use_cbpf,
            use_pinning,
        }
    }

    pub fn is_started(&self) -> bool {
        self.state == ClusterState::Started
    }

    pub fn start(&mut self, addr: &str) -> Result<()> {
        if self.state != ClusterState::Idle {
            return Err(ErrorKind::BadClusterState);
        }
        self.state = ClusterState::Started;

        let num_node = self.num_node;

        println!("SETUP FOR {} CPUs", num_node);

        let mut sockets: Vec<Option<UdpSocket>> = Vec::with_capacity(num_node);

        let mut first_fd: i32 = 0;
        for n in 0..num_node {
            let sock = UdpSocketCluster::build_socket(&addr)?;
            if n == 0 {
                first_fd = sock.as_raw_fd();
            }
            sockets.push(Some(sock));
        }

        if self.use_cbpf {
            println!("try to attach CBPF");
            unsafe {
                let ret = attach_reuseport_cbpf(first_fd, num_node.try_into().unwrap());
                //let ret = attach_reuseport_ebpf(first_fd, num_node.try_into().unwrap());
                if ret != 0 {
                    println!(
                        "failed CBPF setting: {} - {:?}",
                        ret,
                        std::io::Error::last_os_error()
                    );
                    return Err(ErrorKind::CantSetOptionAttachReusePortCbpf);
                }
            }
        }

        println!("start counter");

        let count_tx = self.start_counter_thread();

        println!("start receivers");

        for (n, sock) in sockets.iter_mut().enumerate() {
            let r_sock = sock.take().unwrap();
            self.start_receiver_thread(n, r_sock, count_tx.clone());
        }

        Ok(())
    }

    pub fn stop(&mut self) {
        if !self.is_started() {
            return;
        }
        self.state = ClusterState::Closed;
        for r_closer in self.r_closers.iter() {
            let _ = r_closer.send(());
        }

        for n in 0..self.r_handles.len() {
            if let Some(handle) = self.r_handles[n].take() {
                let _ = handle.join().unwrap();
            }
        }

        if let Some(closer) = self.c_closer.take() {
            let _ = closer.send(());
        }
        if let Some(handle) = self.c_handle.take() {
            let _ = handle.join().unwrap();
        }
    }

    fn start_counter_thread(&mut self) -> Sender<u64> {
        let (closer_tx, closer_rx) = bounded::<()>(1);
        let (count_tx, count_rx) = unbounded::<u64>();
        let handle = thread::spawn(move || {
            let mut cnt = 0;
            let mut last_ts = std::time::Instant::now();
            let mut last_cnt = 0;
            loop {
                select! {
                    recv(count_rx) -> msg => if let Ok(num) = msg {
                        cnt += num;
                        if last_ts.elapsed().as_millis() > 1000 {
                            let cnt_in_sec = cnt - last_cnt;
                            last_cnt = cnt;
                            last_ts = std::time::Instant::now();
                            println!("{} packets/S", cnt_in_sec);
                        }
                    },
                    recv(closer_rx) -> _ => {
                        break;
                    },
                }
            }
        });

        self.c_handle = Some(handle);
        self.c_closer = Some(closer_tx);

        count_tx
    }

    fn start_receiver_thread(&mut self, nth: usize, sock: UdpSocket, count_tx: Sender<u64>) {
        let mut cpu_set = self.cpu_set;

        let (closer_tx, closer_rx) = bounded::<()>(1);
        self.r_closers.push(closer_tx);

        let barrier = self.barrier.clone();

        let use_pinning = self.use_pinning;

        let handle = thread::spawn(move || {
            println!("start {}th receiver thread", nth);

            if use_pinning {
                cpu_set.set(nth).unwrap();
                sched_setaffinity(gettid(), &cpu_set).unwrap();
            }

            let mut buf = [0u8; 65535];

            barrier.wait();

            let mut cnt = 0;

            loop {
                select! {
                    recv(closer_rx) -> _ => {
                        break;
                    },
                    default => {
                        match sock.recv_from(&mut buf) {
                            Ok((len, peer)) => {
                                let packet = &buf[0..len];
                                'send: loop {
                                    match sock.send_to(&packet, peer) {
                                        Ok(_) => {
                                            cnt += 1;
                                            if cnt % 10000 == 0 {
                                                let _ = count_tx.send(cnt);
                                                cnt = 0;
                                            }
                                            break 'send;
                                        },
                                        Err(e) => {
                                            match e.kind() {
                                                std::io::ErrorKind::WouldBlock => {
                                                    continue 'send;
                                                }
                                                _ => {
                                                    error!("sender IO error: {:?}", e);
                                                    break 'send
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                            Err(e) => {
                                match e.kind() {
                                    std::io::ErrorKind::WouldBlock => {
                                        continue;
                                    },
                                    _ => {
                                        error!("receiver IO error: {:?}", e);
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            debug!("finish {}th receiver thread", nth);
        });

        self.r_handles.push(Some(handle));
    }
}
