pub mod stream;

// use axum::extract::ws::{Message, WebSocket};
// use futures::{stream::{SplitSink, SplitStream}, StreamExt};
// use indi::{client::tokio_tcpstream::{AsyncClientConnection, AsyncWriteConnection}, serialization::{Command, DeError}};
// use tokio::sync::mpsc::{self};


// struct IndiWebSocket{
//     websocket: WebSocket,
// }

// impl AsyncClientConnection for IndiWebSocket {
//     type Writer = WebSocketCommandWriter;
//     type Reader = WebSocketCommandReader;

//     fn to_indi(self) -> (Self::Writer, Self::Reader) {
//         let (reader, writer) = self.websocket.split();
//         // let reader = NsReader::from_reader(BufReader::new(reader));
        
//         // (WebSocketCommandWriter { writer }, AsyncIndiReader::new(reader))
//         todo!()
//     }
// }

// struct WebSocketCommandWriter {
//     writer: SplitSink<WebSocket, Message>
// }

// impl CommandWriter for WebSocketCommandWriter {
//     fn write<X: Serialize>(&mut self, command: X) -> Result<(), DeError> {
//         todo!()
//     }
    
//     fn shutdown(&self) {
//         todo!()
//     }
// }

// impl AsyncWriteConnection for WebSocketCommandWriter {
//     async fn write(&mut self, cmd: Command) -> Result<(), crate::DeError> {
//         let msg = quick_xml::se::to_string(&cmd)?;
//         self.writer.send(Message::Text(msg))?;
//         Ok(())
//     }
// }

// impl ClientConnection for IndiWebSocket {
//     fn writer(self) -> Result<impl CommandWriter + Send + 'static, DeError> {
//         Ok(WebSocketCommandWriter { })
//     }
    
//     fn reader(
//         self,
//     ) -> Result<impl Iterator<Item = Result<Command, DeError>>, std::io::Error> {
//         todo!()
//     }

// }
