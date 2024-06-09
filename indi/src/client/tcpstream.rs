use twinkle_client;

use std::{
    io::{BufReader, BufWriter, Write},
    net::{Shutdown, TcpStream},
};

use quick_xml::Writer;

use crate::{serialization::CommandIter, Command, DeError, XmlSerialization};
pub use twinkle_client::notify::{self, wait_fn, Notify};

use super::{ClientConnection, CommandWriter};

pub struct TcpCommandWriter {
    connection: TcpStream,
}

impl CommandWriter for TcpCommandWriter {
    fn write<X: XmlSerialization>(&self, command: &X) -> Result<(), DeError> {
        let mut xml_writer =
            Writer::new_with_indent(BufWriter::new(self.connection.try_clone()?), b' ', 2);

        command.write(&mut xml_writer)?;
        xml_writer.into_inner().flush()?;
        Ok(())
    }
}

impl ClientConnection for TcpStream {
    type Read = TcpStream;
    type Write = TcpStream;

    fn shutdown(&self) -> Result<(), std::io::Error> {
        self.shutdown(Shutdown::Both)
    }

    fn writer(&self) -> Result<impl CommandWriter + Send + 'static, DeError> {
        Ok(TcpCommandWriter {
            connection: self.try_clone()?,
        })
    }

    fn reader(
        &self,
    ) -> Result<impl Iterator<Item = Result<Command, DeError>> + Send + 'static, std::io::Error>
    {
        Ok(CommandIter::new(BufReader::new(self.try_clone()?)))
    }
}

#[cfg(test)]
mod test {
    use std::thread;
    use std::time::{Duration, Instant};

    use crate::client::new;

    use super::*;

    fn wait_finished_timeout<T>(
        join_handle: &tokio::task::JoinHandle<T>,
        timeout: Duration,
    ) -> Result<(), ()> {
        let start = Instant::now();

        loop {
            if join_handle.is_finished() {
                return Ok(());
            }
            if start.elapsed() > timeout {
                return Err(());
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[tokio::test]
    async fn test_threads_stop_on_shutdown() {
        let connection = TcpStream::connect("indi:7624").expect("connecting to indi");
        let client = new(connection, None, None).expect("Making client");
        assert!(wait_finished_timeout(&client._writer_thread, Duration::from_millis(100)).is_err());
        client.shutdown().expect("Shuting down connection");
        assert!(wait_finished_timeout(&client._writer_thread, Duration::from_millis(100)).is_ok());
    }
}
