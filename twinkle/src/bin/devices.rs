use std::{env, net::TcpStream, ops::Deref, thread, time::Duration};

use indi::client::ChangeError;

fn main() -> Result<(), ChangeError<()>> {
    let args: Vec<String> = env::args().collect();
    let addr = &args[1];

    let client = indi::client::new(
        TcpStream::connect(addr).expect(format!("Unable to connect to {}", addr).as_str()),
        None,
        None,
    )?;

    thread::sleep(Duration::from_secs(10));

    let binding = client.get_devices();
    let devices = binding.lock().unwrap();
    println!("{:#?}", devices.deref());
    Ok(())
}
