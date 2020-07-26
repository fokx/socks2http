## Convert SOCKS5 proxy to HTTP proxy with Rust
This is a program that can convert socks5 proxy to http proxy like `Privoxy`.

### implementation details
socks5 connectivity provided by `async-socks5`.

handle `CONNECT` request with TcpStream bidirectional io copy

handle other requests such as `GET` with `reqwest`