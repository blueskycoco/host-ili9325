use tokio::io::Interest;
use tokio::net::TcpStream;
use std::io;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use colored::Colorize;
use crypto::digest::Digest;
use crypto::md5::Md5;

fn usize_to_u8_array(x: usize) -> [u8; 2] {
  let b1: u8 = ((x >> 8) & 0xff) as u8;
  let b2: u8 = (x & 0xff) as u8;

  [b1, b2]
}

#[tokio::main]
async fn main() {
    let param = std::env::args().nth(1).expect("no param given");
    let stream = TcpStream::connect("192.168.1.2:1234").await.unwrap();
    let path = Path::new(&param);
    let mut y :u16 = 0;
    let mut i :u8 = 0;

    loop {
        let s_path = path.join("a-".to_owned() + &i.to_string() + ".bmp");
        let display = s_path.display();
        println!("\r\n\r\ngoing to send: {}\r", display);
        let mut file = match File::open(&s_path) {
            Err(err) => panic!("can't open {}: {:?}", display, err),
            Ok(file) => file,
        };
        i = i + 1;
        let mut ctn = Vec::new();
        file.read_to_end(&mut ctn).unwrap();
        let mut sh = Md5::new();
        sh.input(&ctn);
        let mut digest: [u8; 16] = [0;16];
        sh.result(&mut digest);

        let file_len = usize_to_u8_array(ctn.len());
        let mut vec = Vec::new();
        vec.push(file_len[0]);
        vec.push(file_len[1]);
        vec.extend(digest);
        vec.push(0);
        vec.push(0);
        vec.push(((y >> 8) & 0xff) as u8);
        vec.push((y & 0xff) as u8);
        y = y + 16;
        vec.extend(ctn);

        println!("file len: {}, hash {:?}\r",
                 (file_len[0] as u16) << 8 | file_len[1] as u16, digest);
        let ready = stream.ready(Interest::WRITABLE).await.unwrap();
        if ready.is_writable() {
            match stream.try_write(vec.as_slice()) {
                Ok(n) => { println!("write {} bytes", n); },
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    continue;
                },
                Err(_e) => { return; },
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
                            println!("{} read {} bytes, result {}\r",
                                     "=======>".red(), n, result.bold().red());
                        } else {
                            println!("{} read {} bytes, result {}\r",
                                     "=======>".red(), n, result.green());
                        }
                        if i == 20 { return; }
                        if n != 1000 { break;}
                    },
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        continue;
                    },
                    Err(_e) => { return; },
                }
            } else {
                break;
            }
        }
    }
}
