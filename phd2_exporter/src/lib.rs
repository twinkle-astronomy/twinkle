pub mod serialization;
pub mod metrics;
use serde_json::{de::IoRead, Deserializer, StreamDeserializer};
use serialization::ServerEvent;

pub trait WithMiddleware<T: std::io::Read> {
    fn middleware<F>(self, func: F) -> ReadMiddleware<T, F>
    where
        F: Fn(&[u8]);
}

impl<T: std::io::Read> WithMiddleware<T> for T {
    fn middleware<F>(self, func: F) -> ReadMiddleware<T, F>
    where
        F: Fn(&[u8]),
    {
        ReadMiddleware { read: self, func }
    }
}

pub struct ReadMiddleware<T, F>
where
    T: std::io::Read,
    F: Fn(&[u8]),
{
    read: T,
    func: F,
}

impl<T, F> ReadMiddleware<T, F>
where
    T: std::io::Read,
    F: Fn(&[u8]),
{
    pub fn new(read: T, func: F) -> Self {
        ReadMiddleware { read, func }
    }
}

impl<T, F> std::io::Read for ReadMiddleware<T, F>
where
    T: std::io::Read,
    F: Fn(&[u8]),
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let len = self.read.read(buf)?;
        (self.func)(&buf[0..len]);
        Ok(len)
    }
}

pub trait Connection {
    fn iter(self) -> StreamDeserializer<'static, IoRead<Self>, ServerEvent>
    where
        Self: Sized + std::io::Read,
    {
        Deserializer::from_reader(self).into_iter::<ServerEvent>()
    }
}

impl<T: std::io::Read> Connection for T {}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs::File;

    #[test]
    fn test_read_session() {
        let file = File::open("./src/test_data/session.log").unwrap();

        for _event in file.iter() {
            dbg!(_event.unwrap());
        }
    }
}
