use std::{env, net::TcpStream, thread, time::Duration};

use indi::client::ChangeError;

fn main() -> Result<(), ChangeError<()>> {
    let args: Vec<String> = env::args().collect();
    let addr = &args[1];

    let client = indi::Client::new(
        TcpStream::connect(addr).expect(format!("Unable to connect to {}", addr).as_str()),
        None,
        None,
    )?;

    thread::sleep(Duration::from_secs(10));

    println!("{:#?}", client.devices);
    Ok(())
}
