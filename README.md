## UDP Throughput Benchmarking Tools

works only on Linux server

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
./udpclient --count 10000000 --num 10 --server "127.0.0.1:8080"
```

- sockets; how many sockets (and threads) you wanto build. (on different ephemeral port)
- count: how many packet you want to sent for each sockets
- server: server address
