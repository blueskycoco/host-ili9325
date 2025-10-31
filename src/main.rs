use colored::Colorize;
use serialport::{DataBits, StopBits};
use std::fs::File;
use std::io::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let param = std::env::args()
        .nth(1)
        .expect("no fw assigned, eg: ./serial-ota /path/to/fw");
    let addr = std::env::args().nth(2).expect("no tty given, /dev/ttyACM0");
    let pkt_len = std::env::args().nth(3).expect("no pkt len, 2048 or 131072, depends on flash layout");
    let pkt_len = pkt_len.parse::<usize>().unwrap();

    let mut serial_buf: Vec<u8> = vec![0; 7];

    let builder = serialport::new(&addr, 2_000_000)
        .stop_bits(StopBits::One)
        .data_bits(DataBits::Eight);
    println!("{:?}", &builder);
    let mut port = builder.open().unwrap_or_else(|e| {
        eprintln!("Failed to open \"{}\". Error: {}", addr, e);
        ::std::process::exit(1);
    });
    port.set_timeout(Duration::from_millis(30000)).ok();

    let mut file = match File::open(&param) {
        Err(err) => {
            println!("can't open {}: {:?}", param, err);
            return;
        }
        Ok(file) => file,
    };
    let mut ofs = 0;
    let mut exit_flag = false;
    loop {
        let mut ctn: Vec<u8> = vec![0; pkt_len];
        let mut vec = Vec::new();
        match port.read_exact(serial_buf.as_mut_slice()) {
            Ok(_t) => {
                println!(
                    "recv: {}",
                    std::str::from_utf8(&serial_buf).unwrap().green()
                );
            }
            Err(e) => {
                eprintln!("rx {:?}", e);
                return;
            }
        }
        let r = std::str::from_utf8(&serial_buf).unwrap();
        if !r.contains("send ot") {
            continue;
        }
        println!("going to send: {} {ofs}", param);
        match file.read_exact(&mut ctn) {
            Ok(_t) => {
                vec.push(0);
                ofs += pkt_len;
            }
            Err(_) => {
                vec.push(1);
                exit_flag = true
            }
        }
        vec.extend(ctn);
        match port.write_all(&vec) {
            Ok(_) => {}
            Err(e) => eprintln!("tx {:?}", e),
        }
        if exit_flag {
            println!("ota finished");
            return;
        }
    }
}
