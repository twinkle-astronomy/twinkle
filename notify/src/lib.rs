use std::sync::Arc;
use std::{
    collections::BTreeSet,
    fmt::Debug,
    ops::{Deref, DerefMut},
    sync::{atomic::AtomicU64, Mutex, MutexGuard},
};

use crossbeam_channel::unbounded;

static ID_GENERATOR: once_cell::sync::Lazy<Arc<AtomicU64>> =
    once_cell::sync::Lazy::new(|| Arc::new(AtomicU64::new(0)));

#[derive(Clone)]
struct OrdBox<T> {
    id: u64,
    item: T,
}

impl<T> std::cmp::Ord for OrdBox<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}
impl<T> std::cmp::PartialOrd for OrdBox<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.id.partial_cmp(&other.id)
    }
}

impl<T> std::cmp::PartialEq for OrdBox<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl<T> std::cmp::Eq for OrdBox<T> {}

#[derive(Default)]
pub struct Notify<T> {
    subject: Mutex<Arc<T>>,
    senders: Mutex<BTreeSet<OrdBox<crossbeam_channel::Sender<Arc<T>>>>>,
}

impl<T: Debug + Clone> Debug for Notify<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.subject.fmt(f)
    }
}

impl<T: Clone> Notify<T> {
    pub fn new(value: T) -> Notify<T> {
        Notify {
            subject: Mutex::new(Arc::new(value)),
            senders: Mutex::new(BTreeSet::new()),
        }
    }
}

pub struct NotifyMutexGuard<'a, T: Debug + Clone> {
    // Made an option to allow explicit drop order
    // Must never be None unless in the process of dropping
    // https://stackoverflow.com/a/41056727
    guard: MutexGuard<'a, Arc<T>>,
    to_notify: &'a Notify<T>,
    should_notify: bool,
}

impl<'a, T: Debug + Clone> Debug for NotifyMutexGuard<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_notify.subject.fmt(f)
    }
}

impl<'a, T: Debug + Clone> AsRef<T> for NotifyMutexGuard<'a, T> {
    fn as_ref(&self) -> &T {
        &self.guard
    }
}

impl<'a, T: Debug + Clone> Deref for NotifyMutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl<'a, T: Debug + Clone> DerefMut for NotifyMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.should_notify = true;
        Arc::make_mut(self.guard.deref_mut())
    }
}

impl<'a, T: Debug + Clone> Drop for NotifyMutexGuard<'a, T> {
    fn drop(&mut self) {
        if self.should_notify {
            let mut senders = self.to_notify.senders.lock().unwrap();
            senders.retain(|s| s.item.send(self.guard.deref().clone()).is_ok());
        }
        drop(&mut self.guard);
    }
}

impl<T: Debug + Clone> Notify<T> {
    pub fn lock(&self) -> NotifyMutexGuard<T> {
        NotifyMutexGuard {
            guard: self.subject.lock().unwrap(),
            to_notify: self,
            should_notify: false,
        }
    }

    pub fn subscribe(&self) -> crossbeam_channel::Receiver<Arc<T>> {
        let (s, r) = unbounded::<Arc<T>>();
        let subject = self.subject.lock().unwrap();
        let mut senders = self.senders.lock().unwrap();

        s.send(subject.deref().clone())
            .expect("sending initial value");
        senders.insert(OrdBox {
            id: ID_GENERATOR.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            item: s,
        });
        r
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::{thread, time::Duration};

    #[test]

    fn test_atomic() {
        let current = ID_GENERATOR.load(std::sync::atomic::Ordering::Relaxed);
        ID_GENERATOR.store(0, std::sync::atomic::Ordering::Relaxed);

        let joins = (0..100).map(|_| {
            thread::spawn(move || {
                ID_GENERATOR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            })
        });

        joins.for_each(|j| j.join().unwrap());

        assert_eq!(current + 100, 100);
    }

    #[test]
    fn test_first_item_is_current_value() {
        let n = Notify::new(1);

        assert_eq!(
            n.subscribe()
                .recv_timeout(Duration::from_millis(100))
                .unwrap(),
            Arc::new(1)
        );
    }

    #[test]
    fn test_sequence() {
        let mut joins = Vec::new();
        {
            let n = Arc::new(Notify::new(-1));

            for _ in 0..10 {
                let thread_n = n.clone();

                joins.push(thread::spawn(move || {
                    let r = thread_n.subscribe();

                    let mut prev = r.recv_timeout(Duration::from_millis(100)).unwrap();
                    for i in r.iter() {
                        if *i == 90 {
                            break;
                        }
                        assert_eq!(*i, *prev + 1);
                        prev = i;
                    }
                }));
            }
            thread::sleep(Duration::from_millis(100));

            for i in 0..=90 {
                let mut l = n.lock();
                *l = i;
            }
        }

        joins.into_iter().for_each(|x| x.join().unwrap());
    }

    #[test]
    fn test_destroying_receivers() {
        let n = Notify::new(0);

        {
            let r = n.subscribe();
            {
                let mut l = n.lock();
                *l = 1;
            }
            assert_eq!(*r.recv_timeout(Duration::from_millis(10)).unwrap(), 0);
            assert_eq!(*r.recv_timeout(Duration::from_millis(10)).unwrap(), 1);
        }
        {
            let r = n.subscribe();
            {
                let mut l = n.lock();
                *l = 2;
            }
            assert_eq!(*r.recv_timeout(Duration::from_millis(10)).unwrap(), 1);
            assert_eq!(*r.recv_timeout(Duration::from_millis(10)).unwrap(), 2);
        }
    }

    #[test]
    fn test_notify_on_mut() {
        let n = Notify::new(0);

        let r = n.subscribe();

        r.try_recv().unwrap();
        {
            let _no_mut = n.lock();
        }
        assert!(r.try_recv().is_err());
        {
            let mut with_mut = n.lock();
            *with_mut = 1;
        }

        assert_eq!(*r.recv_timeout(Duration::from_millis(100)).unwrap(), 1);
    }

    #[test]
    fn test_mut_from_channel() {
        let n = Notify::new(0);

        let mut m = n.subscribe().recv().unwrap();
        *Arc::make_mut(&mut m) = 1;

        assert_ne!(*n.lock(), *m);
    }
}
