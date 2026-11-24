use std::error::Error;
use std::future::pending;
use zbus::{connection, interface};

struct ScaraDaemon {
    volume: u8,
}

#[interface(name = "org.marchscara.Daemon1")]
impl ScaraDaemon {
    fn status(&self) -> String {
        format!("MARCH SCARA Daemon Active | Volume: {}%", self.volume)
    }

    fn set_volume(&mut self, level: u8) -> u8 {
        self.volume = level.min(100);
        self.volume
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let daemon = ScaraDaemon { volume: 65 };
    let _conn = connection::Builder::session()
        .name("org.marchscara.Daemon")
        .serve_at("/org/marchscara/Daemon", daemon)
        .build()
        .await?;

    println!("scara-daemon berjalan di D-Bus: org.marchscara.Daemon");
    pending::<()>().await;
    Ok(())
}