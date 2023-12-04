use tokio_stream::Stream;

mod changes;
use changes::Changes;

pub trait StreamExt {
    fn changes<T>(self) -> Changes<T, Self>
    where
        Self: Stream<Item = T> + Sized,
    {
        Changes::new(self)
    }
}

impl<T: Stream> StreamExt for T {}
