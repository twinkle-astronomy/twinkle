use std::sync::Arc;

use super::ChangeAble;


impl<T: ChangeAble> ChangeAble for Arc<T> {
    type ChangeItem = T::ChangeItem;

    fn change(&self, other: Option<&Self>) -> Self::ChangeItem where Self: Sized {
        let other = other.map(|x| x.as_ref());
        self.as_ref().change(other)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use futures::StreamExt;

    use futures::stream;

    use crate::stream::changeable::{ChangeAble, ToChanges};

    // use super::*;

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
        let iter = vec![1, 2, 3, 4, 5, 6, 7, 8, 9].into_iter().map(|x| Arc::new(Into::<TestItem>::into(x)));
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
