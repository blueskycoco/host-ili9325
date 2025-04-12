use chrono::{DateTime, FixedOffset, Local, Utc};
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

fn usize_to_u8_array(x: usize) -> [u8; 2] {
    let b1: u8 = ((x >> 8) & 0xff) as u8;
    let b2: u8 = (x & 0xff) as u8;

    [b1, b2]
}

#[tokio::main]
async fn main() {
    let param = std::env::args()
        .nth(1)
        .expect("no folder, eg: ./host-ili9325 /path/to/pic");
    let addr = std::env::args().nth(2).expect("no tty given, /dev/ttyACM0");

    let mut serial_buf: Vec<u8> = vec![0; 7];
    let client = rsntp::AsyncSntpClient::new();
    let time_info = client.synchronize("pool.ntp.org").await.unwrap();
    let datetime_utc: DateTime<Utc> = time_info.datetime().try_into().unwrap();
    let local_time: DateTime<Local> = DateTime::from(datetime_utc);
    println!(
        "Local time: {}",
        local_time.with_timezone(&FixedOffset::east_opt(8 * 3600).unwrap())
    );

    let builder = serialport::new(&addr, 921_600)
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
                    if i == 20 {
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
                    vec.extend(digest);
                    vec.push(0);
                    vec.push(0);
                    vec.push(((y >> 8) & 0xff) as u8);
                    vec.push((y & 0xff) as u8);
                    y = y + 16;
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
