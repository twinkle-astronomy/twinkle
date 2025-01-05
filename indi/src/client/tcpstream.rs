use quick_xml::{events::Event, NsReader};
use tokio::{
    io::{AsyncRead, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
};

use crate::Command;
use tokio::io::BufReader;

use super::{AsyncClientConnection, AsyncReadConnection, AsyncWriteConnection};

impl AsyncClientConnection for TcpStream {
    type Reader = AsyncIndiReader<OwnedReadHalf>;
    type Writer = AsyncIndiWriter;

    fn to_indi(self) -> (Self::Writer, Self::Reader) {
        let (reader, writer) = self.into_split();
        let reader = NsReader::from_reader(BufReader::new(reader));

        (AsyncIndiWriter { writer }, AsyncIndiReader::new(reader))
    }
}

pub struct AsyncIndiReader<T> {
    reader: NsReader<BufReader<T>>,
}

impl<T: AsyncRead + Unpin> AsyncIndiReader<T> {
    fn new(reader: quick_xml::reader::NsReader<BufReader<T>>) -> AsyncIndiReader<T> {
        AsyncIndiReader { reader }
    }

    async fn read_xml_documents(&mut self) -> Option<Result<String, crate::DeError>> {
        let mut buffer = Vec::new();
        let mut document = Vec::new();
        let mut depth = 0;
        loop {
            let event = match self.reader.read_event_into_async(&mut buffer).await {
                Ok(e) => e,
                Err(e) => return Some(Err(e.into())),
            };
            match event {
                Event::Start(e) => {
                    depth += 1;
                    document.extend_from_slice(b"<");
                    document.extend_from_slice(e.name().as_ref());
                    for attr in e.attributes() {
                        let attr = match attr {
                            Ok(d) => d,
                            Err(e) => return Some(Err(e.into())),
                        };
                        document.extend_from_slice(b" ");
                        document.extend_from_slice(attr.key.as_ref());
                        document.extend_from_slice(b"=\"");
                        document.extend_from_slice(&attr.value);
                        document.extend_from_slice(b"\"");
                    }
                    document.extend_from_slice(b">");
                }
                Event::End(e) => {
                    depth -= 1;
                    document.extend_from_slice(b"</");
                    document.extend_from_slice(e.name().as_ref());
                    document.extend_from_slice(b">");
                    if depth == 0 {
                        let doc = match String::from_utf8(document) {
                            Ok(d) => d,
                            Err(e) => return Some(Err(e.into())),
                        };
                        return Some(Ok(doc));
                    }
                }
                Event::Text(e) => {
                    document.extend_from_slice(&e.into_inner());
                }
                Event::Eof => return None,
                _ => {
                    // Handle other event types if needed
                }
            }
            buffer.clear();
        }
    }
}

impl<T: AsyncRead + Unpin + Send> AsyncReadConnection for AsyncIndiReader<T> {
    async fn read(&mut self) -> Option<Result<crate::Command, crate::DeError>> {
        let doc = match self.read_xml_documents().await? {
            Ok(doc) => doc,
            Err(e) => return Some(Err(e.into())),
        };
        let cmd = quick_xml::de::from_str::<crate::Command>(&doc).map_err(|x| x.into());

        return Some(cmd);
    }
}

pub struct AsyncIndiWriter {
    writer: OwnedWriteHalf,
}

impl AsyncWriteConnection for AsyncIndiWriter {
    async fn write(&mut self, cmd: Command) -> Result<(), crate::DeError> {
        let buffer = quick_xml::se::to_string(&cmd)?;
        self.writer.write(buffer.as_bytes()).await?;

        self.writer.write(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), crate::DeError> {
        Ok(self.writer.shutdown().await?)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::client::new;

    #[tokio::test]
    async fn test_threads_stop_on_shutdown() {
        let connection = TcpStream::connect("indi:7624")
            .await
            .expect("connecting to indi");
        let mut client = new(connection, None, None);
        client.shutdown();
        if let Some((reader, writer)) = client._workers.take() {
            let _ = tokio::join!(reader, writer);
        }
    }
}
