// use indi;
// use chrono::prelude::*;
// use std::str::FromStr;

fn main() {
    let mut client = indi::Client::new("localhost:7624").unwrap();
    client.query_devices();
    client.listen_for_updates();
}
