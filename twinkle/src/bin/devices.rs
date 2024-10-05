use std::{env, ops::Deref, time::Duration};

use indi::client::ChangeError;
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> Result<(), ChangeError<()>> {
    let args: Vec<String> = env::args().collect();
    let addr = &args[1];

    let client = indi::client::new(
        TcpStream::connect(addr).await.expect(format!("Unable to connect to {}", addr).as_str()),
        None,
        None,
    )?;

    tokio::time::sleep(Duration::from_secs(10)).await;

    let binding = client.get_devices();
    let devices = binding.lock().await;
    println!("{:#?}", devices.deref());
    Ok(())
}
