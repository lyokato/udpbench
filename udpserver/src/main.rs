#[macro_use]
extern crate log;
extern crate clap;
extern crate num_cpus;

mod cluster;
mod result;

use std::convert::TryInto;

use cluster::UdpSocketCluster;

use clap::{App, Arg};
use crossbeam_channel::bounded;

fn main() {
    let app = App::new("UDP Server")
        .version("1.0")
        .author("lyokato")
        .about("UDP Server")
        .arg(
            Arg::with_name("sockets")
                .short("s")
                .long("sockets")
                .value_name("SOCKETS")
                .help("number of sockets")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("cbpf")
                .long("cbpf")
                .value_name("CBPF")
                .help("use SO_ATTACH_REUSEPORT_CBPF flag")
                .required(false)
                .takes_value(false),
        )
        .arg(
            Arg::with_name("pinning")
                .long("pinning")
                .value_name("PINNING")
                .help("use CPU pinning")
                .required(false)
                .takes_value(false),
        )
        .arg(
            Arg::with_name("port")
                .short("p")
                .long("port")
                .value_name("PORT")
                .help("server's binding port")
                .required(true)
                .takes_value(true),
        );

    let matches = app.get_matches();
    let port: u32 = matches.value_of("port").unwrap().parse().unwrap();
    let num: i32 = matches.value_of("sockets").unwrap().parse().unwrap();
    let use_cbpf = matches.is_present("cbpf");
    let use_pinning = matches.is_present("pinning");

    let mut cluster: UdpSocketCluster =
        UdpSocketCluster::new(num.try_into().unwrap(), use_cbpf, use_pinning);
    let address = format!("0.0.0.0:{}", port);
    println!("try to bind {}", address);
    if let Err(err) = cluster.start(&address) {
        println!("failed to start: {:?}", err);
        return;
    }

    let (quit_tx, quit_rx) = bounded::<()>(1);
    ctrlc::set_handler(move || {
        cluster.stop();
        quit_tx.send(()).unwrap();
    })
    .expect("handles Ctrl-C");

    let _ = quit_rx.recv();
}
