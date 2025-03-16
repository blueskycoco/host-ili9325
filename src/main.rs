use colored::Colorize;
use crypto::digest::Digest;
use crypto::md5::Md5;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::path::Path;
use tokio::io::Interest;
use tokio::net::TcpStream;
use walkdir::WalkDir;
use chrono::{DateTime, FixedOffset, Local, Utc};

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
    let addr = std::env::args()
        .nth(2)
        .expect("no addr given, 192.168.1.3:1234");
    
    let client = rsntp::AsyncSntpClient::new();
    let time_info = client.synchronize("pool.ntp.org").await.unwrap();
    let datetime_utc: DateTime<Utc> = time_info.datetime().try_into().unwrap();
    let local_time: DateTime<Local> = DateTime::from(datetime_utc);
    println!("Local time: {}", local_time.with_timezone(&FixedOffset::east_opt(8*3600).unwrap()));

    println!("to connect usr232-wifi-t");
    let stream = TcpStream::connect(addr).await.unwrap();
    println!("usr232-wifi-t connected");
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
                    i = i + 1;
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
                    y = y + 160;
                    vec.extend(ctn);

                    println!(
                        "file len: {}, hash {:?}\r",
                        (file_len[0] as u16) << 8 | file_len[1] as u16,
                        digest
                    );
                    loop {
                        let ready = stream.ready(Interest::WRITABLE).await.unwrap();
                        if ready.is_writable() {
                            match stream.try_write(vec.as_slice()) {
                                Ok(n) => {
                                    println!("write {} of {} bytes", n, vec.len());
                                    if n == vec.len() {
                                        println!("send {} done\r", vec.len());
                                        break;
                                    } else {
                                        vec = vec.split_off(n);
                                    }
                                }
                                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                    continue;
                                }
                                Err(_e) => {
                                    return;
                                }
                            }
                        }
                    }

                    loop {
                        let ready = stream.ready(Interest::READABLE).await.unwrap();
                        if ready.is_readable() {
                            let mut data = vec![0; 1024];
                            match stream.try_read(&mut data) {
                                Ok(n) => {
                                    let result = String::from(core::str::from_utf8(&data).unwrap());
                                    if !result.contains("send ok") {
                                        y = y - 16;
                                        i = i - 1;
                                        println!(
                                            "{} read {} bytes, result {}\r",
                                            "=======>".red(),
                                            n,
                                            result.bold().red()
                                        );
                                    } else {
                                        println!(
                                            "{} read {} bytes, result {}\r",
                                            "=======>".red(),
                                            n,
                                            result.green()
                                        );
                                    }
                                    break;
                                }
                                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                    println!("WouldBlock ...");
                                    continue;
                                }
                                Err(_e) => {
                                    return;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
