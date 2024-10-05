use futures::{Stream, StreamExt};
use pin_project_lite::pin_project;

pub mod arc;
pub mod hashmap;

pub trait ChangeAble {
    type ChangeItem;
    fn change(&self, other: Option<&Self>) -> Self::ChangeItem where Self: Sized;
}

pin_project! {
    #[must_use = "streams do nothing unless polled"]
    pub struct StreamChanges<T: ChangeAble, S: Stream<Item = T>> {
        #[pin]
        stream: S,
        previous: Option<T>
    }
}

pub trait ToChanges {
    fn to_changes<T: ChangeAble>(self) -> StreamChanges<T, Self> where Self: Stream<Item = T> + Sized{
        StreamChanges { stream: self, previous: None }
    }
}
impl<T: ChangeAble, S: Stream<Item = T>> ToChanges for S { }

impl<T: ChangeAble, S: Stream<Item = T>> Stream for StreamChanges<T, S> {
    type Item = T::ChangeItem;

    fn poll_next(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        match self.as_mut().project().stream.poll_next_unpin(cx) {
            std::task::Poll::Ready(Some(item)) => {
                let ret = item.change(self.previous.as_ref());
                *self.as_mut().project().previous = Some(item);
                std::task::Poll::Ready(Some(ret))
            },
            std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use futures::stream;
    use futures::StreamExt;

    use super::*;

    struct TestItem(i32);
    impl ChangeAble for TestItem {
        type ChangeItem = i32;
    
        fn change(&self, other: Option<&Self>) -> Self::ChangeItem where Self: Sized {
            match other {
                Some(other) => self.0 - other.0,
                None => self.0
            }
        }
    }
    impl From<i32> for TestItem {
        fn from(value: i32) -> Self {
            TestItem(value)
        }
    }
    #[tokio::test]
    async fn test_trivial() {
        let iter = vec![1, 2, 3, 4, 5, 6, 7, 8, 9].into_iter().map(|x| Into::<TestItem>::into(x));
        let stream = stream::iter(iter);
        let mut stream = stream.to_changes();
        assert_eq!(stream.next().await, Some(1));
        assert_eq!(stream.next().await, Some(1));
        assert_eq!(stream.next().await, Some(1));
        assert_eq!(stream.next().await, Some(1));
        assert_eq!(stream.next().await, Some(1));
        assert_eq!(stream.next().await, Some(1));
    }
}
