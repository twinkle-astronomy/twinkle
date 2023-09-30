use std::time::Duration;

use phd2::{
    serialization::{Event, Settle},
    Phd2Connection,
};

#[tokio::main]
async fn main() {
    let (phd2, mut events): (Phd2Connection<_>, _) = Phd2Connection::from(
        tokio::net::TcpStream::connect("astro.local:4400")
            .await
            .expect("Connecting to phd2"),
    );

    phd2.loop_().await.unwrap();

    phd2.find_star(None).await.ok();

    phd2.set_paused(true, true).await.expect("Pausing");
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
    .expect("guiding");

    phd2.disconnect().await.expect("disconnecting");

    while let Some(event) = events.recv().await {
        if let Event::GuideStep(guide) = &event.event {
            let delta = (guide.dx.powi(2) + guide.dy.powi(2)).sqrt();
            println!("guide error: {:2.2}", delta);
        }
    }
}
