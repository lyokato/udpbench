## UDP echo server/client for throughput benchmarking

This code works on Linux server.

### Preparation

You have to set some kernel options for enough beffer size.
Proper numbers for each option are depends on your test's scale.

- net.core.rmem_default
- net.core.rmem_max
- net.core.wmem_default
- net.core.wmem_max
- net.core.netdev_max_backlog
- net.ipv4.udp_mem
- net.ipv4.udp_rmem_min
- net.ipv4.udp_wmem_min

### udpserver

UDP Echo Server

```
cd udpserver
cargo build --release
cd target/release
./udpserver --port 8080 --sockets 16
```

- port: port number for UDP socket to bind
- sockets: how many sockets (and threads) you want to build on same port (with SO_REUSEPORT)
- cbpf: SO_ATTACH_REUSEPORT_CBPF option
- pinning: CPU pinning for each threads


### udpclient

```
cd udpclient
cargo build --release
cd target/release
./udpclient --sockets 32 --count 10000000 --server "127.0.0.1:8080"
```

- sockets; how many sockets (and threads) you wanto build. (on different ephemeral port for each socket)
- count: how many packet you want to sent for each socket
- server: server address
