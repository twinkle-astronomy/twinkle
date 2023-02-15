use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

#[derive(Debug, PartialEq)]
pub enum Error<E> {
    Timeout,
    Abort(E),
}

pub enum Status<S> {
    Pending,
    Complete(S),
}

pub struct Notify<T> {
    subject: Mutex<T>,
    change_condition: std::sync::Condvar,
}

impl<T: Debug> Debug for Notify<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.subject.fmt(f)
    }
}

impl<T> Notify<T> {
    pub fn new(value: T) -> Notify<T> {
        Notify {
            subject: Mutex::new(value),
            change_condition: std::sync::Condvar::new(),
        }
    }
}
impl<T: Default> Default for Notify<T> {
    fn default() -> Self {
        Notify {
            subject: Mutex::new(Default::default()),
            change_condition: std::sync::Condvar::new(),
        }
    }
}

impl<T> From<T> for Error<T> {
    fn from(value: T) -> Self {
        Error::Abort(value)
    }
}

pub struct NotifyMutexGuard<'a, T: Debug> {
    // Made an option to allow explicit drop order
    // Must never be None unless in the process of dropping
    // https://stackoverflow.com/a/41056727
    guard: Option<MutexGuard<'a, T>>,
    to_notify: &'a Notify<T>,
}

impl<'a, T: Debug> Debug for NotifyMutexGuard<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.guard.fmt(f)
    }
}
impl<'a, T: Debug> AsRef<T> for NotifyMutexGuard<'a, T> {
    fn as_ref(&self) -> &T {
        &self.guard.as_ref().unwrap()
    }
}

impl<'a, T: Debug> Deref for NotifyMutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.guard.as_deref().unwrap()
    }
}

impl<'a, T: Debug> DerefMut for NotifyMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.as_deref_mut().unwrap()
    }
}

impl<'a, T: Debug> Drop for NotifyMutexGuard<'a, T> {
    fn drop(&mut self) {
        self.guard = None;
        self.to_notify.notify_all();
    }
}

impl<T: Debug> Notify<T> {
    pub fn lock(&self) -> NotifyMutexGuard<T> {
        NotifyMutexGuard {
            guard: Some(self.subject.lock().unwrap()),
            to_notify: &self,
        }
    }

    pub fn notify_all(&self) {
        self.change_condition.notify_all();
    }

    pub fn wait(&self, dur: Duration) -> Result<MutexGuard<T>, ()> {
        let subject_lock = self.subject.lock().unwrap();
        let (m, result) = self
            .change_condition
            .wait_timeout(subject_lock, dur)
            .unwrap();
        match result.timed_out() {
            true => Err(()),
            false => Ok(m),
        }
    }

    pub fn wait_fn<'a, S, E, F: FnMut(&MutexGuard<T>) -> Result<Status<S>, E>>(
        &self,
        dur: Duration,
        mut f: F,
    ) -> Result<S, Error<E>> {
        let start = Instant::now();
        let mut subject_lock = self.subject.lock().unwrap();

        loop {
            if let Status::Complete(ret) = f(&subject_lock)? {
                return Ok(ret);
            }

            let elapsed = start.elapsed();
            let remaining = if dur > elapsed {
                dur - elapsed
            } else {
                return Err(Error::Timeout);
            };

            let (next_lock, timeout) = self
                .change_condition
                .wait_timeout(subject_lock, remaining)
                .unwrap();

            if timeout.timed_out() {
                return Err(Error::Timeout);
            }

            subject_lock = next_lock;
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::{sync::Arc, thread};

    #[test]
    fn test_wait_fn_trivial() {
        let notify: Arc<Notify<u32>> = Default::default();

        let wait_fn_result = notify
            .wait_fn::<u32, (), _>(Duration::from_secs(1), move |i| {
                let state = **i;
                Ok(Status::Complete(state))
            })
            .expect("Notify completed");
        assert_eq!(wait_fn_result, 0);
    }

    #[test]
    fn test_wait_fn_abort() {
        let notify: Arc<Notify<u32>> = Default::default();

        let wait_fn_result = notify.wait_fn::<(), u32, _>(Duration::from_secs(1), |_| Err(1));
        assert_eq!(wait_fn_result, Err(Error::Abort(1)));
    }

    #[test]
    fn test_wait_fn_timeout() {
        let notify: Arc<Notify<u32>> = Default::default();

        let wait_fn_result =
            notify.wait_fn::<(), (), _>(Duration::from_millis(100), |_| Ok(Status::Pending));
        assert_eq!(wait_fn_result, Err(Error::Timeout));
    }

    #[test]
    fn test_wait_fn_condition() {
        let notify: Arc<Notify<u32>> = Default::default();
        let fn_count: Arc<Mutex<u32>> = Default::default();

        let thread_notify = notify.clone();
        let thread_count = fn_count.clone();
        let t = thread::spawn(move || {
            thread_notify
                .wait_fn::<u32, (), _>(Duration::from_secs(1), move |i| {
                    {
                        let mut l = thread_count.lock().unwrap();
                        *l = *l + 1;
                    }
                    if **i == 0 {
                        return Ok(Status::Pending);
                    } else {
                        return Ok(Status::Complete(**i));
                    }
                })
                .expect("Notify completed")
        });
        // ugly race-based thread syncronization
        thread::sleep(Duration::from_millis(100));
        {
            let mut nl = notify.lock();
            *nl = 1;
        }

        let t_result = t.join().unwrap();
        assert_eq!(t_result, 1);
        {
            let l = fn_count.lock().unwrap();
            assert_eq!(*l, 2);
        }
    }
}
