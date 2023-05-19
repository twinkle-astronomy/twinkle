pub mod serialization;

use std::net::TcpStream;
use std::fs::File;

use serde_json::{Deserializer, StreamDeserializer, de::IoRead};
use crate::serialization::Event;


pub trait Connection {
    type Read: std::io::Read + Send;

    fn iter(&self) -> StreamDeserializer<'_, IoRead<<Self as Connection>::Read>, Event>{
        let deser = Deserializer::from_reader(self.clone_reader().unwrap());
        deser.into_iter::<Event>()
    }

    fn clone_reader(&self) -> Result<Self::Read, std::io::Error>;
}

impl Connection for TcpStream {
    type Read = TcpStream;

    fn clone_reader(&self) -> Result<Self::Read, std::io::Error> {
        self.try_clone()
    }
}

impl Connection for File {
    type Read = File;

    fn clone_reader(&self) -> Result<Self::Read, std::io::Error> {
        self.try_clone()
    }
}

#[cfg(test)]
mod tests {



    use super::*;

    #[test]
    fn test_read_session() {

        let file = File::open("./src/test_data/session.log").unwrap();

        for _event in file.iter() {
            dbg!(_event);
        }

    }
}
