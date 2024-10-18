use indi::{client::{AsyncClientConnection, AsyncReadConnection, AsyncWriteConnection}, serialization::GetProperties, INDI_PROTOCOL_VERSION};
use tokio_tungstenite::connect_async;
use tracing::error;

#[tokio::main]
async fn main() {
    let (websocket, _) = connect_async("ws://localhost:4000/".to_string()).await.unwrap();

    let (mut write, mut read) = websocket.to_indi();

    let reader = tokio::spawn(async move {
        loop {
            match read.read().await {
                Some(Ok(msg)) => {
                    println!("-----------------");
                    dbg!(&msg);
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
        let cmd = indi::serialization::Command::GetProperties(GetProperties {
            version: INDI_PROTOCOL_VERSION.to_string(),
            device: None,
            name: None,
        });
        write.write(cmd).await.expect("Sending command");
    });

    if let Err(e) = tokio::try_join!(reader, writer) {
        error!("Fatal: {:?}", e);
    }
}
