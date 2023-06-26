use core::pin::Pin;
use core::task::Context;
use pin_project_lite::pin_project;
use std::task::{ready, Poll};
use tokio_stream::Stream;

pin_project! {
    #[must_use = "streams do nothing unless polled"]
    pub struct Changes<I, S> {
        #[pin]
        stream: S,

        prev: Option<I>
    }
}

impl<I: PartialEq + Clone, S: Stream<Item = I>> Stream for Changes<I, S> {
    type Item = I;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match ready!(self.as_mut().project().stream.poll_next(cx)) {
                Some(cur) => match self.as_mut().project().prev {
                    Some(prev) => {
                        if *prev != cur {
                            *prev = cur.clone();
                            return std::task::Poll::Ready(Some(cur));
                        }
                    }
                    None => {
                        *self.as_mut().project().prev = Some(cur.clone());
                        return std::task::Poll::Ready(Some(cur));
                    }
                },
                None => return Poll::Ready(None),
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.stream.size_hint().1) // can't know a lower bound, due to the predicate
    }
}

impl<T, S: Stream<Item = T>> Changes<T, S> {
    pub fn new(stream: S) -> Self {
        Changes { prev: None, stream }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_stream::iter;
    use tokio_stream::StreamExt;

    #[tokio::test]
    async fn test_trivial() {
        let stream = iter(vec![1, 2, 2, 3]);
        let items = stream.collect::<Vec<i32>>().await;
        assert_eq!(items, vec![1, 2, 2, 3]);
    }
    #[tokio::test]
    async fn test_from() {
        let stream = Changes::new(iter(vec![1, 2, 2, 3]));
        let items = stream.collect::<Vec<i32>>().await;
        assert_eq!(items, vec![1, 2, 3]);
    }
}
