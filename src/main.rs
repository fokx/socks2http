use std::net::{Ipv4Addr, SocketAddrV4, SocketAddr, ToSocketAddrs};
use std::io::{Read, BufRead, Write};
use std::str::FromStr;
use log::{info, warn};
use std::ops::Deref;
use tokio::net::{TcpStream, TcpListener};
use tokio::io::BufReader;
use tokio::prelude::*;
use reqwest::Method;
/*
TODO: IPv6 support
 parsing [2606:4700:4700::1111]:443
x6.x87.org

 */
static UPSTREAM_HTTP_PROXY_URL: &str = "http://127.0.0.1:8118";
static UPSTREAM_SOCKS5_PROXY_URL_FULL: &str = "socks5://127.0.0.1:20001";
static UPSTREAM_SOCKS5_PROXY_URL: &str = "127.0.0.1:20001";
static BIND_URL: &str = "127.0.0.1:8121";

fn print_type_of<T>(_: &T) {
    println!("{}", std::any::type_name::<T>())
}

#[tokio::main]
async fn main() {//-> Result<(), Box<dyn std::error::Error>> {
    env_logger::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let mut listener = TcpListener::bind(BIND_URL).await.unwrap();
    warn!("Listening on http://{}", BIND_URL);

    while let Ok((mut inbound, addr)) = listener.accept().await {
        info!("NEW CLIENT: {}", addr);
        tokio::spawn(async move {
            let mut buf = [0; 1024 * 8];
            /* stream.read() will clear buf before writing,
             also, after reading, stream will become blank

            do not use String as buffer, which will add a lot of head to the program,
             leading to seemingly spawn does not work!
            // let mut buffer = String::new();
            // stream.read_to_string(&mut buffer).await.unwrap();
            // println!("CLIENT SAYS:\n{}\nEND OF CLIENT MESSAGE", buffer);
             */
            let downstream_read_bytes_size = match inbound.read(&mut buf).await {
                // socket closed
                Ok(0) => return,
                Ok(n) => n,
                Err(e) => {
                    println!("failed to read from socket; err = {:?}", e);
                    return;
                }
            };
            let bytes_from_downstream = &buf[0..downstream_read_bytes_size];

            let mut headers = [httparse::EMPTY_HEADER; 16];
            let mut req = httparse::Request::new(&mut headers);
            let parse_result = req.parse(bytes_from_downstream);
            if let Err(e) = parse_result {
                info!("invalid req: ");
                // convert buf to utf8 string may fail:
                let invalid_buf = String::from_utf8(buf[0..downstream_read_bytes_size].to_vec());
                if let Err(e) = invalid_buf {
                    info!("request not valid UTF sequence from some point, {}", e);
                }
                // } else {
                //     info!("{}", invalid_buf.unwrap());
                // }
                return;
            } else {
                // convert buf to utf8 string may fail:
                let invalid_buf = String::from_utf8(buf[0..downstream_read_bytes_size].to_vec());
                if let Err(e) = invalid_buf {
                    info!("request not valid UTF sequence from some point, {}", e);
                    return;
                }


                if parse_result.unwrap().is_complete() {
                    if let Some(valid_req_path) = req.path {
                        info!("parsing {}", valid_req_path);
                        let (req_host, req_port) = if !valid_req_path.contains("/") {
                            let mut vec: Vec<&str> = valid_req_path.split(":").collect();
                            let port = vec.last().unwrap().parse::<u16>();
                            if let Err(e) = port {
                                warn!("ignore invalid req port, {}", e);
                                return;
                            }
                            let port = port.unwrap();
                            let slice = &vec[..];
                            let host: String = slice[0..slice.len() - 1].join(":"); // String
                            (host, port)
                        } else {
                            let req_url_parser =
                                reqwest::Url::parse(valid_req_path);
                            if let Err(e) = req_url_parser {
                                warn!("ignore invalid req, {}", e);
                                return;
                            }
                            let req_url_parser = req_url_parser.unwrap();

                            let req_host = req_url_parser.host().unwrap().to_string();
                            let req_port = if let Some(port) = req_url_parser.port() {
                                port
                            } else {
                                80
                            };
                            (req_host, req_port)
                        };
                        let pass_outbound = format!("{}:{}", req_host, req_port);
                        info!("PASS {} TO UPSTREAM", pass_outbound);

                        // let mut upstream = socks::Socks5Stream::connect(
                        //     UPSTREAM_SOCKS5_PROXY_URL, &*pass_outbound).unwrap();

                        let outbound =
                            TcpStream::connect(UPSTREAM_SOCKS5_PROXY_URL).await.unwrap();
                        let mut outbound =
                            tokio::io::BufStream::new(outbound);
                        async_socks5::connect(&mut outbound, ((req_host, req_port)), None)
                            .await.unwrap();

                        if req.method.unwrap() == Method::CONNECT {
                            inbound.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").await.unwrap();
                            let (mut ri, mut wi) = inbound.split();
                            let (mut ro, mut wo) = outbound.get_mut().split();

                            let client_to_server = async {
                                io::copy(&mut ri, &mut wo)
                                    .await.expect("Transport endpoint is not connected");
                                wo.shutdown().await
                            };

                            let server_to_client = async {
                                io::copy(&mut ro, &mut wi)
                                    .await.expect("Transport endpoint is not connected");
                                wi.shutdown().await
                            };

                            futures::future::try_join(client_to_server, server_to_client)
                                .await.unwrap();
                        } else {
                            let socks5_url = reqwest::Url::parse(UPSTREAM_SOCKS5_PROXY_URL_FULL).unwrap();
                            let client = reqwest::Client::builder()
                                .proxy(reqwest::Proxy::all(socks5_url).unwrap())
                                .build()
                                .unwrap();
                            info!("connect via SOCKS5 to {}", valid_req_path);
                            info!("{}", invalid_buf.unwrap());
                            let response = client.get(valid_req_path).send().await.unwrap();
                            // let headers = response.headers();
                            // let body_text =response.text().await.unwrap();
                            let response_bytes = response.bytes().await.unwrap();
                            let write_result = inbound.write(&response_bytes).await;
                            inbound.flush().await.unwrap();
                            // Ok(hyper::Response::new(hyper::Body::from(body_text)))


                            // // Method = GET ...
                            // let upstream_write_bytes_size =
                            //     outbound.write(bytes_from_downstream).await.unwrap();
                            // assert_eq!(upstream_write_bytes_size, downstream_read_bytes_size);
                            //
                            // let (mut ri, mut wi) = inbound.split();
                            // let (mut ro, mut wo) = outbound.get_mut().split();
                            //
                            // io::copy(&mut ro, &mut wi)
                            //     .await.expect("Transport endpoint is not connected");
                            // wi.shutdown().await;
                            // wo.shutdown().await;
                        }
                    } else {
                        warn!("GOT INVALID REQ PATH: {:?}", req.path)
                    };
                } else {
                    warn!("GOT INCOMPLETE REQ: {:?}", req.path)
                }
            }
        });
    };
}



