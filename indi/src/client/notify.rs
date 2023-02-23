use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{
    collections::BTreeSet,
    fmt::Debug,
    ops::{Deref, DerefMut},
    sync::{atomic::AtomicU64, Mutex, MutexGuard},
};

use crossbeam_channel::bounded;

static ID_GENERATOR: once_cell::sync::Lazy<Arc<AtomicU64>> =
    once_cell::sync::Lazy::new(|| Arc::new(AtomicU64::new(0)));

#[derive(Debug, PartialEq)]
pub enum Error<E> {
    Timeout,
    Abort(E),
}

impl<T> From<T> for Error<T> {
    fn from(value: T) -> Self {
        Error::Abort(value)
    }
}

pub enum Status<S> {
    Pending,
    Complete(S),
}

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

impl<T: Debug> Debug for Notify<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.subject.fmt(f)
    }
}

impl<T> Notify<T> {
    pub fn new(value: T) -> Notify<T> {
        Notify {
            subject: Mutex::new(Arc::new(value)),
            senders: Mutex::new(BTreeSet::new()),
        }
    }
}

pub struct NotifyMutexGuard<'a, T: Clone> {
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

impl<'a, T: Clone> AsRef<T> for NotifyMutexGuard<'a, T> {
    fn as_ref(&self) -> &T {
        &self.guard
    }
}

impl<'a, T: Clone> Deref for NotifyMutexGuard<'a, T> {
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

impl<'a, T: Clone> Drop for NotifyMutexGuard<'a, T> {
    fn drop(&mut self) {
        if self.should_notify {
            let mut senders = self.to_notify.senders.lock().unwrap();
            senders.retain(|s| {
                s.item
                    .send_timeout(self.guard.deref().clone(), Duration::from_secs(1))
                    .is_ok()
            });
        }
        drop(&mut self.guard);
    }
}

impl<T: Clone + Debug> Notify<T> {
    pub fn lock(&self) -> NotifyMutexGuard<T> {
        NotifyMutexGuard {
            guard: self.subject.lock().unwrap(),
            to_notify: self,
            should_notify: false,
        }
    }

    pub fn subscribe(&self) -> crossbeam_channel::Receiver<Arc<T>> {
        let (s, r) = bounded::<Arc<T>>(0);
        // let _subject = self.subject.lock().unwrap();
        let mut senders = self.senders.lock().unwrap();
        // s.send(subject.deref().clone())
        //     .expect("sending initial value");
        senders.insert(OrdBox {
            id: ID_GENERATOR.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            item: s,
        });
        r
    }

    pub fn wait_fn<'a, S, E, F: FnMut(&T) -> Result<Status<S>, E>>(
        &self,
        dur: Duration,
        mut f: F,
    ) -> Result<S, Error<E>> {
        let start = Instant::now();

        let (mut next, recv) = {
            let lock = self.subject.lock().unwrap();
            (lock.clone(), self.subscribe())
        };

        loop {
            if let Status::Complete(ret) = f(next.deref())? {
                return Ok(ret);
            }
            let elapsed = start.elapsed();
            let remaining = if dur > elapsed {
                dur - elapsed
            } else {
                return Err(Error::Timeout);
            };

            next = match recv.recv_timeout(remaining) {
                Ok(n) => n,
                Err(_) => return Err(Error::Timeout),
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::{thread, time::Duration};

    #[test]
    fn test_sequence() {
        let mut joins = Vec::new();
        {
            let n = Arc::new(Notify::new(-1));

            for _ in 0..10 {
                let thread_n = n.clone();

                joins.push(thread::spawn(move || {
                    let mut prev = -1;
                    let r = thread_n.subscribe();
                    for j in r.iter() {
                        if *j == 90 {
                            break;
                        }
                        assert_eq!(*j, prev + 1);
                        prev = *j;
                    }
                }));
            }
            // ugly race-based attempt to wait for threads to be ready
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

        let len = n.senders.lock().unwrap().len();
        assert_eq!(len, 0);

        {
            let _r = n.subscribe();

            let len = n.senders.lock().unwrap().len();
            assert_eq!(len, 1);
        }
        let len = n.senders.lock().unwrap().len();
        assert_eq!(len, 1);
        {
            let mut l = n.lock();
            *l = 1;
        }

        let len = n.senders.lock().unwrap().len();
        assert_eq!(len, 0);
    }

    #[test]
    fn test_notify_on_mut() {
        let n = Arc::new(Notify::new(0));

        let r = n.subscribe();

        let thread_n = n.clone();
        let j = thread::spawn(move || {
            {
                let _no_mut = thread_n.lock();
            }
            {
                let mut with_mut = thread_n.lock();
                *with_mut = 1;
            }
        });
        let update = r.recv_timeout(Duration::from_millis(100)).unwrap();
        assert!(r.try_recv().is_err());
        assert_eq!(*update, 1);

        j.join().unwrap();
    }

    #[test]
    fn test_wakes() {
        let notify: Arc<Notify<u32>> = Arc::new(Notify::new(0));

        let count = Arc::new(Mutex::new(0));
        let count_thread = count.clone();
        let thread_notify = notify.clone();
        let j = thread::spawn(move || {
            thread_notify
                .wait_fn::<(), (), _>(Duration::from_secs(1), |iteration| {
                    {
                        let mut l = count_thread.lock().unwrap();
                        *l += 1;
                    }
                    if *iteration == 9 {
                        Ok(Status::Complete(()))
                    } else {
                        Ok(Status::Pending)
                    }
                })
                .unwrap();
        });
        // ugly race-based thread syncronization
        thread::sleep(Duration::from_millis(100));
        for i in 0..=9 {
            let mut lock = notify.lock();
            *lock = i;
        }

        j.join().unwrap();
        {
            let lock = count.lock().unwrap();
            assert_eq!(*lock, 11);
        }
    }
}
