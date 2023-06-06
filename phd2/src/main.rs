use phd2::{serialization::Event, Phd2Connection};

#[tokio::main]
async fn main() {
    let mut phd2: Phd2Connection<_> = tokio::net::TcpStream::connect("localhost:4400")
        .await
        .expect("Connecting to phd2")
        .into();
    let mut pixel_scale = phd2.get_pixel_scale().await.expect("Getting pixel scale.");

    let mut sub = phd2.subscribe().await.expect("Subscribing to events");

    while let Ok(event) = sub.recv().await {
        if let Event::GuideStep(guide) = &event.event {
            let delta = pixel_scale * (guide.dx.powi(2) + guide.dy.powi(2)).sqrt();
            println!("guide event: {} arcsec.", delta);
        }
        if let Event::ConfigurationChange(_) = &event.event {
            pixel_scale = phd2.get_pixel_scale().await.expect("Getting pixel scale.");
        }
    }
}
