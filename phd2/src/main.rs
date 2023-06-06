use std::time::Duration;

use phd2::{
    serialization::{Event, Settle},
    Phd2Connection,
};

#[tokio::main]
async fn main() {
    let mut phd2: Phd2Connection<_> = tokio::net::TcpStream::connect("astro.local:4400")
        .await
        .expect("Connecting to phd2")
        .into();
    phd2.loop_().await.unwrap();

    phd2.find_star(None).await.ok();

    phd2.guide(
        Settle {
            pixels: 1.0,
            time: Duration::from_secs(5).into(),
            timeout: Duration::from_secs(60).into(),
        },
        None,
        None,
    )
    .await
    .unwrap();

    let mut sub = phd2.subscribe().await.unwrap();

    while let Ok(event) = sub.recv().await {
        if let Event::GuideStep(guide) = &event.event {
            let delta = (guide.dx.powi(2) + guide.dy.powi(2)).sqrt();
            println!("guide error: {:2.2}", delta);
        }
    }
}
