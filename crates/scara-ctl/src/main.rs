use std::env;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let cmd = if args.is_empty() {
        "ping".to_string()
    } else {
        args.join(" ")
    };

    let runtime_dir = env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let sock_path = format!("{}/scara.sock", runtime_dir);

    match UnixStream::connect(&sock_path) {
        Ok(mut stream) => {
            stream.write_all(cmd.as_bytes()).expect("Gagal kirim perintah");
            stream.shutdown(std::net::Shutdown::Write).ok();

            let mut reply = String::new();
            stream.read_to_string(&mut reply).ok();
            println!("{}", reply);
        }
        Err(e) => {
            eprintln!("Tidak bisa terhubung ke {} ({})", sock_path, e);
            eprintln!("Pastikan scara-wm sedang berjalan.");
            std::process::exit(1);
        }
    }
}