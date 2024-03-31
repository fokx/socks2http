use clap::Parser;
use log::{info, warn};
use rand::prelude::SliceRandom;
use reqwest::Method;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::io;
use tokio::net::{TcpListener, TcpStream};

// fn print_type_of<T>(_: &T) {
//     println!("{}", std::any::type_name::<T>())
// }

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(short, long, num_args = 1..)]
    from: Option<Vec<usize>>,
    #[arg(short, long)]
    to: Option<usize>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Cli::parse();
    let upstreams: &'static [usize] = args.from.unwrap().leak();

    let bind_port = args.to.expect("invalid `to` port");
    // warn!("convert proxy from socks5 :{:?} to http :{:?}", &upstreams_clone.clone(), bind_port);

    let  listener = TcpListener::bind(format!("127.0.0.1:{:?}", bind_port)).await?;
    loop {
        let (mut inbound, addr) = listener.accept().await?;
        info!("NEW CLIENT: {}", addr);
        tokio::spawn(async move {
            let upstream_port = upstreams.choose(&mut rand::thread_rng()).unwrap();
            let mut buf = [0; 1024 * 8];
            let Ok(downstream_read_bytes_size) = inbound.read(&mut buf).await else { return; };
            let bytes_from_downstream = &buf[0..downstream_read_bytes_size];

            let mut headers = [httparse::EMPTY_HEADER; 16];
            let mut req = httparse::Request::new(&mut headers);
            let Ok(parse_result) = req.parse(bytes_from_downstream) else { return; };
            // let Ok(invalid_buf) = String::from_utf8(buf[0..downstream_read_bytes_size].to_vec()) else {return};

            if parse_result.is_complete() {
                if let Some(valid_req_path) = req.path {
                    info!("parsing {}", valid_req_path);
                    let (req_host, req_port) = if !valid_req_path.contains("/") {
                        let vec: Vec<&str> = valid_req_path.split(":").collect();
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

                    warn!("forwarded to socks5 proxy at port {}!", upstream_port);
                    let outbound = TcpStream::connect(format!("127.0.0.1:{:?}", upstream_port)).await.unwrap();
                    let mut outbound = io::BufStream::new(outbound);
                    async_socks5::connect(&mut outbound, (req_host, req_port), None).await.unwrap();

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
                            let _ = io::copy(&mut ro, &mut wi).await;
                            wi.shutdown().await
                        };

                        let _ = futures::future::try_join(client_to_server, server_to_client).await;
                    } else {
                        let socks5_url = reqwest::Url::parse(&*format!("socks5://127.0.0.1:{:?}", upstream_port).to_string()).unwrap();
                        let client = reqwest::Client::builder()
                                .proxy(reqwest::Proxy::all(socks5_url).unwrap())
                                .build()
                                .unwrap();
                        info!("connect via SOCKS5 to {}", valid_req_path);
                        let response = client.get(valid_req_path).send().await.unwrap();
                        // let headers = response.headers();
                        // let body_text =response.text().await.unwrap();
                        let response_bytes = response.bytes().await.unwrap();
                        let _ = inbound.write(&response_bytes).await;
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
            }
        });
    };
}



