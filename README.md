## socks2http: Convert SOCKS5 proxy to HTTP proxy with Rust

[socks2http](https://github.com/fokx/socks2http) can convert an existing SOCKS5 proxy to an HTTP proxy.

This is just what [Privoxy](https://www.privoxy.org/)'s `forward-socks5` does.

### Usage
```zsh
cargo build --release
socks2http --from <space separated SOCKS5 ports> --to <HTTP proxy listen port>
# e.g. 
# forward a socks5 server listening at localhost:9050 as an http proxy at localhost:10050
socks2http --from 9050 --to 10050
# forward multiple socks5 servers listening at localhost:2000, :2001, :2002, using random switch policy
socks2http --from 2000 2001 2002 --to 10050

```

### Implementation
socks5 connectivity provided by `async-socks5`.

handle `CONNECT` request with TcpStream bidirectional io copy

handle other requests such as `GET` with `reqwest`