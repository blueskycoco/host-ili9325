use chrono::{DateTime, FixedOffset, Local, NaiveDateTime, Utc};
use colored::Colorize;
use crypto::digest::Digest;
use crypto::md5::Md5;
use serialport::{DataBits, StopBits};
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::path::Path;
use std::thread;
use std::time::Duration;
use walkdir::WalkDir;

use std::convert::TryFrom;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use tokio::io::split;
use tokio::io::{copy, stdout as tokio_stdout, AsyncWriteExt};
use tokio::io::{ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio_rustls::rustls::{self, ClientConfig, OwnedTrustAnchor, RootCertStore};
use tokio_rustls::TlsConnector;

use async_compression::tokio::write::GzipDecoder;
use hwclock::HwClockDev;
use tokio::io::AsyncWriteExt as _; // for `write_all` and `shutdown`

struct NoCertVerifier {}

impl rustls::client::ServerCertVerifier for NoCertVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _server_name: &rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}

pub async fn connect(
    dst_addr: &str,
    dst_port: u16,
    sni: &str,
    allow_insecure: bool,
) -> io::Result<(
    ReadHalf<tokio_rustls::client::TlsStream<tokio::net::TcpStream>>,
    WriteHalf<tokio_rustls::client::TlsStream<tokio::net::TcpStream>>,
)> {
    let addr = (dst_addr, dst_port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;

    let mut root_store = RootCertStore::empty();
    root_store.add_server_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.0.iter().map(|ta| {
        OwnedTrustAnchor::from_subject_spki_name_constraints(
            ta.subject,
            ta.spki,
            ta.name_constraints,
        )
    }));
    let mut config = ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    if allow_insecure {
        config
            .dangerous()
            .set_certificate_verifier(Arc::new(NoCertVerifier {}));
    }

    let connector = TlsConnector::from(Arc::new(config));
    let stream = TcpStream::connect(&addr).await?;

    let domain = rustls::ServerName::try_from(sni)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid dnsname"))?;

    let stream = connector.connect(domain, stream).await?;
    // stream.write_all(content.as_bytes()).await?;

    // let (mut reader, mut writer) = split(stream);

    Ok(split(stream))
}

async fn decompress(in_data: &[u8]) -> Result<Vec<u8>, &str> {
    let mut decoder = GzipDecoder::new(Vec::new());
    decoder.write_all(in_data).await.unwrap();
    decoder.shutdown().await.unwrap();
    Ok(decoder.into_inner())
}

fn usize_to_u8_array(x: usize) -> [u8; 3] {
    let b1: u8 = ((x >> 16) & 0xff) as u8;
    let b2: u8 = ((x >> 8) & 0xff) as u8;
    let b3: u8 = (x & 0xff) as u8;

    [b1, b2, b3]
}

#[tokio::main]
async fn main() {
    let param = std::env::args()
        .nth(1)
        .expect("no folder, eg: ./host-ili9325 /path/to/pic");
    let addr = std::env::args().nth(2).expect("no tty given, /dev/ttyACM0");

    let mut serial_buf: Vec<u8> = vec![0; 1024];

    let client = rsntp::AsyncSntpClient::new();
    let time_info = client.synchronize("pool.ntp.org").await.unwrap();
    let datetime_utc: DateTime<Utc> = time_info.datetime().try_into().unwrap();
    let local_time: DateTime<Local> = DateTime::from(datetime_utc);
    println!(
        "Local time: {}",
        local_time.with_timezone(&FixedOffset::east_opt(8 * 3600).unwrap())
    );
    let ct: NaiveDateTime = datetime_utc.naive_local();
    let rtc = HwClockDev::open("/dev/rtc0").expect("can't open rtc dev");
    rtc.set_time(&ct.into()).expect("can't set rtc time");
    let allow_insecure = false;
    // let sni = "www.baidu.com";
    // let dst_addr = "www.baidu.com";
    let sni = "devapi.qweather.com";
    let dst_addr = "devapi.qweather.com";
    let dst_port = 443;
    let content = format!("GET /airquality/v1/current/39.95/116.46 HTTP/1.1\r\nX-QW-Api-Key: c8cd8ac05fcb4808baf95c58c94c2fe8\r\nHost: {}\r\n\r\n", sni);

    let (mut reader, mut writer) = connect(dst_addr, dst_port, sni, allow_insecure)
        .await
        .unwrap();
    writer.write_all(content.as_bytes()).await.unwrap();
    let mut rsp: Vec<u8> = Vec::new();
    copy(&mut reader, &mut rsp).await.unwrap();
    //println!("{:?}", String::from_utf8(rsp.clone()).unwrap());
    match rsp.iter().position(|&b| b == 139) {
        Some(ofs) => {
            let rsp = rsp.split_off(ofs - 1);
            let body = decompress(&rsp).await.unwrap();
            println!("{:?}", String::from_utf8(body).unwrap());
        }
        None => println!("can't find gzip header"),
    }

    let builder = serialport::new(&addr, 2_000_000)
        .stop_bits(StopBits::One)
        .data_bits(DataBits::Eight);
    println!("{:?}", &builder);
    let mut port = builder.open().unwrap_or_else(|e| {
        eprintln!("Failed to open \"{}\". Error: {}", addr, e);
        ::std::process::exit(1);
    });
    port.set_timeout(Duration::from_millis(3000)).ok();

    loop {
        for entry in WalkDir::new(&param) {
            let entry = entry.unwrap();
            if entry.file_type().is_dir() && entry.depth() == 1 {
                println!("{} {}", entry.path().display(), entry.depth());
                let path = Path::new(&param);
                let path = path.join(entry.file_name());
                let mut y: u16 = 0;
                let mut i: u8 = 0;

                loop {
                    if i == 2 {
                        thread::sleep(Duration::from_millis(3000));
                        break;
                    }
                    let s_path = path.join("a-".to_owned() + &i.to_string() + ".bmp");
                    let display = s_path.display();
                    println!("\r\n\r\ngoing to send: {}\r", display);
                    let mut file = match File::open(&s_path) {
                        Err(err) => {
                            println!("can't open {}: {:?}", display, err);
                            break;
                        }
                        Ok(file) => file,
                    };
                    i += 1;
                    let mut ctn = Vec::new();
                    file.read_to_end(&mut ctn).unwrap();
                    let mut sh = Md5::new();
                    sh.input(&ctn);
                    let mut digest: [u8; 16] = [0; 16];
                    sh.result(&mut digest);

                    let file_len = usize_to_u8_array(ctn.len());
                    let mut vec = Vec::new();
                    vec.push(file_len[0]);
                    vec.push(file_len[1]);
                    vec.push(file_len[2]);
                    vec.extend(digest);
                    vec.push(0);
                    vec.push(0);
                    vec.push(((y >> 8) & 0xff) as u8);
                    vec.push((y & 0xff) as u8);
                    y += 160;
                    vec.extend(ctn);

                    println!(
                        "file len: {}, hash {:02x?}\r",
                        (file_len[0] as u16) << 8 | file_len[1] as u16,
                        digest
                    );
                    match port.write_all(&vec) {
                        Ok(_) => {}
                        Err(ref e) if e.kind() == io::ErrorKind::TimedOut => (),
                        Err(e) => eprintln!("{:?}", e),
                    }
                    match port.read_exact(serial_buf.as_mut_slice()) {
                        Ok(_t) => {
                            println!(
                                "recv: {}",
                                std::str::from_utf8(&serial_buf).unwrap().green()
                            );
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::TimedOut => (),
                        Err(e) => eprintln!("{:?}", e),
                    }
                }
            }
        }
    }
}
