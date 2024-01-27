use tokio::io::Interest;
use tokio::net::TcpStream;
use std::io;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use colored::Colorize;

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
    let display = path.display();

    let mut file = match File::open(&path) {
        Err(err) => panic!("can't open {}: {:?}", display, err),
        Ok(file) => file,
    };

    let mut ctn = String::new();
    file.read_to_string(&mut ctn).unwrap();
    //println!("write timeout {}, read timeout {}\r",
    //         stream.write_timeout().unwrap(),
    //         stream.read_timeout().unwrap());

//    loop {
        // 注册可读和可写事件，并等待事件的发生
        let ready = stream.ready(Interest::WRITABLE).await.unwrap();
        //stream.try_write(&usize_to_u8_array(ctn.len())).unwrap();
        // 如果注册的事件中，发生了可写事件，则执行如下代码
        if ready.is_writable() {
            let file_len = usize_to_u8_array(ctn.len());
            let mut vec = Vec::new();
            vec.push(file_len[0]);
            vec.push(file_len[1]);
            vec.extend(ctn.as_bytes());
            //stream.try_write(&file_len).unwrap();
            println!("file len: {}\r", (file_len[0] as u16) << 8 | file_len[1] as u16);
            match stream.try_write(vec.as_slice()) {
                Ok(n) => {
                    println!("write {} bytes", n);
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    return;
                }
                Err(_e) => {
                    return;
                }
            }
        }
       
        loop {
        let ready = stream.ready(Interest::READABLE).await.unwrap();

        // 如果注册的事件中，发生了可读事件，则执行如下代码
        if ready.is_readable() {
            let mut data = vec![0; 1024];
            match stream.try_read(&mut data) {
                Ok(n) => {
                    println!("\r\n\r\n{} read {} bytes, data:\n{}\r", "=======>".red(), n, core::str::from_utf8(&data).unwrap().green());
                    if n != 1000 {
                        return;
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(_e) => {
                    return;
                }
            }
        } else {
            break;
        }
        }
  //  }
}
