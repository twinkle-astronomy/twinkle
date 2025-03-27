use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use super::ChangeAble;

impl<'a, 'b, K: Clone + Eq + PartialEq + Hash, V: Clone + PartialEq + Eq + Hash> ChangeAble
    for HashMap<K, V>
{
    type ChangeItem = HashSet<Change<K, V>>;

    fn change(&self, previous: Option<&Self>) -> Self::ChangeItem {
        let previous = match previous.as_ref() {
            Some(previous) => previous,
            None => return self.change(Some(&Self::default())),
        };
        let mut ret = HashSet::new();

        for (pk, pv) in previous.iter() {
            match self.get(pk) {
                Some(cv) => {
                    if !cv.eq(pv) {
                        ret.insert(Change::Update(pk.clone(), cv.clone()));
                    }
                }
                None => {
                    ret.insert(Change::Delete(pk.clone()));
                }
            }
        }
        for (ck, cv) in self.iter() {
            if let None = previous.get(ck) {
                ret.insert(Change::Add(ck.clone(), cv.clone()));
            }
        }
        ret
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Change<K: Clone, V: Clone> {
    Add(K, V),
    Update(K, V),
    Delete(K),
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use futures::stream;
    use futures::StreamExt;

    use crate::stream::changeable::ToChanges;

    use super::*;

    #[tokio::test]
    async fn test_hashmap() {
        let a = Arc::new(HashMap::from([("a", 1)]));
        let b = Arc::new(HashMap::from([("a", 1), ("b", 2)]));

        let c = Arc::new(HashMap::from([("a", 4), ("c", 3)]));

        let iter = vec![a, b, c];
        let stream = stream::iter(iter);
        let mut stream = stream.to_changes();

        assert_eq!(
            stream.next().await,
            Some(HashSet::from([Change::Add("a", 1)]))
        );
        assert_eq!(
            stream.next().await,
            Some(HashSet::from([Change::Add("b", 2)]))
        );
        assert_eq!(
            stream.next().await,
            Some(HashSet::from([
                Change::Delete("b"),
                Change::Update("a", 4),
                Change::Add("c", 3)
            ]))
        );
    }
}
