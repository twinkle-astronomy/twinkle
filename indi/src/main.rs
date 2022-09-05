// use indi;
// use chrono::prelude::*;
// use std::str::FromStr;

fn main() {
    let mut client = indi::Client::new("localhost:7624").unwrap();
    client.query_devices();
    for command in client.command_iter().unwrap() {
        match command {
            Ok(indi::Command::DefParameter(param)) => {
                println!("entry: {:?}", param);
            }
            Err(e) => match e {
                e => println!("error: {:?}", e),
            },
        }
    }
}
