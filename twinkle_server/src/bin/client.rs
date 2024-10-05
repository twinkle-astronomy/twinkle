use futures::{SinkExt, StreamExt};
use indi::{serialization::GetProperties, INDI_PROTOCOL_VERSION};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::error;

#[tokio::main]
async fn main() {
    let (websocket, _) = connect_async("ws://localhost:4000/".to_string()).await.unwrap();

    let (mut write, mut read) = websocket.split();

    let reader = tokio::spawn(async move {
        loop {
            match read.next().await {
                Some(Ok(msg)) => {
                    println!("-----------------");
                    println!("{}", &msg);
                    println!("-----------------");
                },
                Some(Err(e)) => {
                    dbg!(e);
                }
                None => {
                    break;
                }
            }
        }

    });

    let writer = tokio::spawn(async move {
        let cmd = GetProperties {
            version: INDI_PROTOCOL_VERSION.to_string(),
            device: None,
            name: None,
        };
        let msg = quick_xml::se::to_string(&cmd).unwrap();
        dbg!(&msg);
        write.send(Message::Text(msg)).await.expect("Sending command");
    });

    if let Err(e) = tokio::try_join!(reader, writer) {
        error!("Fatal: {:?}", e);
    }
}
