use tokio::io::Interest;
use tokio::net::TcpStream;
use std::io;

#[tokio::main]
async fn main() {
    let param = std::env::args().nth(1).expect("no param given");
    let stream = TcpStream::connect("192.168.1.2:1234").await.unwrap();

//    loop {
        // 注册可读和可写事件，并等待事件的发生
        let ready = stream.ready(Interest::READABLE | Interest::WRITABLE).await.unwrap();

        // 如果注册的事件中，发生了可读事件，则执行如下代码
        if ready.is_readable() {
            let mut data = vec![0; 1024];
            match stream.try_read(&mut data) {
                Ok(n) => {
                    println!("read {} bytes", n);
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    return;
                }
                Err(e) => {
                    return;
                }
            }
        }

        // 如果注册的事件中，发生了可写事件，则执行如下代码
        if ready.is_writable() {
            match stream.try_write(param.as_bytes()) {
                Ok(n) => {
                    println!("write {} bytes", n);
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    return;
                }
                Err(e) => {
                    return;
                }
            }
        }
  //  }
}
