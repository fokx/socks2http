## socks2http: Convert SOCKS5 proxy to HTTP proxy with Rust

socks2http can convert an existing SOCKS5 proxy to an HTTP proxy written in Rust.
It works like `Privoxy`.

### Usage
```zsh
cargo build --release
socks2http <FROM_PORT> <TO_PORT>
# e.g. convert socks proxy at localhost:9050 to http proxy at localhost:10050
socks2http 9050 10050
```

### Implementation
socks5 connectivity provided by `async-socks5`.

handle `CONNECT` request with TcpStream bidirectional io copy

handle other requests such as `GET` with `reqwest`