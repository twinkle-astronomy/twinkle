# indi

This crate provides support for the Instrument Neutral Distributed Interface (INDI) network protocol used to provide a network interface into controlling astronomical equipment.  See; https://www.indilib.org/index.html for more details on INDI.

## Quickstart
Add the crate to your cargo.toml
```bash
$ cargo add indi
```

An example program that creates an indi::Connection struct that represents a connect to a localhost indi server, and processes commands into a client object that keeps track of active devices and properties.
```rust
use indi;

fn main() {
    let mut connection = indi::Connection::new("localhost:7624").unwrap();
    connection
        .send(&indi::GetProperties {
            version: indi::INDI_PROTOCOL_VERSION.to_string(),
            device: None,
            name: None,
        })
        .unwrap();

    let mut client = indi::Client::new();

    for command in connection.command_iter().unwrap() {
        match command {
            Ok(command) => {
                if let Err(e) = client.update(command) {
                    println!("error: {:?}", e)
                }
            }
            Err(e) => match e {
                e => println!("error: {:?}", e),
            },
        }
    }
}
```

## Contributing
Contributions are welcome.  

In general, we follow the "fork-and-pull" Git workflow.

 1. **Fork** the repo on GitHub
 2. **Clone** the project to your own machine
 3. **Commit** changes to your own branch
 4. **Push** your work back up to your fork
 5. Submit a **Pull request** so that we can review your changes

NOTE: Be sure to merge the latest from "upstream" before making a pull request!