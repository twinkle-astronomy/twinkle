pub mod device;
pub mod notify;

use std::{
    collections::{BTreeSet, HashMap},
    ops::Deref,
    sync::Arc,
    time::Instant,
};

use crossbeam_channel::{Receiver, Select};

use crate::{serialization, Command, DeError, TypeError};

use super::notify::{Notify, Subscription};

#[derive(Debug)]
pub enum ChangeError<E> {
    NotifyError(notify::Error<E>),
    DeError(serialization::DeError),
    IoError(std::io::Error),
    Disconnected(crossbeam_channel::SendError<Command>),
    DisconnectedRecv(crossbeam_channel::TryRecvError),
    DisconnectedRecvTimeout(crossbeam_channel::RecvTimeoutError),
    Canceled,
    PropertyError,
    TypeMismatch,
}

impl<T> From<crossbeam_channel::RecvTimeoutError> for ChangeError<T> {
    fn from(value: crossbeam_channel::RecvTimeoutError) -> Self {
        ChangeError::DisconnectedRecvTimeout(value)
    }
}
impl From<notify::Error<ChangeError<serialization::Command>>> for ChangeError<Command> {
    fn from(value: notify::Error<ChangeError<serialization::Command>>) -> Self {
        match value {
            notify::Error::Timeout => ChangeError::Canceled,
            notify::Error::Canceled => ChangeError::Canceled,
            notify::Error::Abort(e) => e,
        }
    }
}
impl<E> From<std::io::Error> for ChangeError<E> {
    fn from(value: std::io::Error) -> Self {
        ChangeError::<E>::IoError(value)
    }
}
impl<E> From<notify::Error<E>> for ChangeError<E> {
    fn from(value: notify::Error<E>) -> Self {
        ChangeError::NotifyError(value)
    }
}
impl<E> From<DeError> for ChangeError<E> {
    fn from(value: DeError) -> Self {
        ChangeError::<E>::DeError(value)
    }
}
impl<E> From<TypeError> for ChangeError<E> {
    fn from(_: TypeError) -> Self {
        ChangeError::<E>::TypeMismatch
    }
}
impl<E> From<crossbeam_channel::SendError<Command>> for ChangeError<E> {
    fn from(value: crossbeam_channel::SendError<Command>) -> Self {
        ChangeError::Disconnected(value)
    }
}

impl<E> From<crossbeam_channel::TryRecvError> for ChangeError<E> {
    fn from(value: crossbeam_channel::TryRecvError) -> Self {
        ChangeError::DisconnectedRecv(value)
    }
}

pub trait Waitable {
    type Result;
    fn wait(self: Box<Self>) -> Result<Self::Result, ChangeError<Command>>;
    fn cancel(&self);
}

impl<I, R, T: Pending<Item = I, Result = R> + ?Sized> Waitable for T {
    type Result = R;

    fn wait(self: Box<Self>) -> Result<Self::Result, ChangeError<Command>> {
        loop {
            let next_value = self.receiver().recv_deadline(self.deadline())?;
            match self.tick(next_value)? {
                notify::Status::Pending => {}
                notify::Status::Complete(result) => return Ok(result),
            }
        }
    }

    fn cancel(&self) {
        self.cancel()
    }
}

pub trait Pending {
    type Item;
    type Result;

    fn deadline(&self) -> Instant;
    fn receiver(&self) -> &Receiver<Self::Item>;
    fn tick(&self, item: Self::Item) -> Result<notify::Status<Self::Result>, ChangeError<Command>>;
    fn cancel(&self);
}

pub struct PendingNotify<F, S, T>
where
    F: Fn(Arc<S>) -> Result<notify::Status<Arc<T>>, ChangeError<Command>>,
{
    subscription: Subscription<S>,
    notifier: Arc<Notify<S>>,
    deadline: Instant,
    func: F,
}

impl<F, S, T> Pending for PendingNotify<F, S, T>
where
    F: Fn(Arc<S>) -> Result<notify::Status<Arc<T>>, ChangeError<Command>>,
    S: Clone + std::fmt::Debug,
{
    type Item = Arc<S>;
    type Result = Arc<T>;
    fn tick(&self, next: Self::Item) -> Result<notify::Status<Self::Result>, ChangeError<Command>> {
        (self.func)(next)
    }

    fn deadline(&self) -> Instant {
        self.deadline
    }

    fn receiver(&self) -> &Receiver<Self::Item> {
        self.subscription.deref()
    }

    fn cancel(&self) {
        self.notifier.cancel(&self.subscription);
    }
}

pub struct WaitingSequence<R1, R2, F1, F2>
where
    F1: FnOnce() -> Result<Box<dyn Waitable<Result = R1>>, ChangeError<Command>>,
    F2: FnOnce(R1) -> Result<Box<dyn Waitable<Result = R2>>, ChangeError<Command>>,
{
    first: F1,
    next: F2,
}

impl<R1, R2, F1, F2> WaitingSequence<R1, R2, F1, F2>
where
    F1: FnOnce() -> Result<Box<dyn Waitable<Result = R1>>, ChangeError<Command>>,
    F2: FnOnce(R1) -> Result<Box<dyn Waitable<Result = R2>>, ChangeError<Command>>,
{
    pub fn new(first: F1, next: F2) -> WaitingSequence<R1, R2, F1, F2> {
        WaitingSequence { first, next }
    }
}

impl<R1, R2, F1, F2> Waitable for WaitingSequence<R1, R2, F1, F2>
where
    F1: FnOnce() -> Result<Box<dyn Waitable<Result = R1>>, ChangeError<Command>>,
    F2: FnOnce(R1) -> Result<Box<dyn Waitable<Result = R2>>, ChangeError<Command>>,
{
    type Result = R2;

    fn wait(self: Box<Self>) -> Result<Self::Result, ChangeError<Command>> {
        let item = (self.first)()?;
        let next = (self.next)(item.wait()?)?;
        next.wait()
    }

    fn cancel(&self) {
        todo!()
    }
}

pub struct PendingChangeBatch<I, R> {
    changes: Vec<Box<dyn Pending<Item = I, Result = R>>>,
}

impl<I, R> PendingChangeBatch<I, R> {
    pub fn new() -> PendingChangeBatch<I, R> {
        PendingChangeBatch {
            changes: Default::default(),
        }
    }

    pub fn add(
        mut self,
        pending_change: Box<dyn Pending<Item = I, Result = R> + 'static>,
    ) -> PendingChangeBatch<I, R> {
        self.changes.push(pending_change);
        self
    }

    pub fn wait(self) -> Result<HashMap<usize, R>, ChangeError<Command>> {
        Box::new(self).wait()
    }
}
impl<I, R> Waitable for PendingChangeBatch<I, R> {
    type Result = HashMap<usize, R>;
    fn wait(self: Box<Self>) -> Result<Self::Result, ChangeError<Command>> {
        // let results = Vec::with_capacity(self.changes.len());
        let mut sel = Select::new();
        let mut remaining = BTreeSet::new();
        for (i, r) in self.changes.iter().enumerate() {
            sel.recv(r.receiver());
            remaining.insert(i);
        }

        let mut results = HashMap::new();

        loop {
            let selected = sel.select();
            let i = selected.index();
            let r = selected.recv(self.changes[i].receiver()).unwrap();
            match self.changes[i].tick(r) {
                Ok(v) => {
                    if let notify::Status::Complete(v) = v {
                        results.insert(i, v);
                        remaining.remove(&i);

                        if remaining.is_empty() {
                            return Ok(results);
                        }
                    }
                }
                Err(e) => {
                    for i in &remaining {
                        self.changes[*i].cancel();
                    }
                    return Err(e);
                }
            }
        }
    }

    fn cancel(&self) {
        for change in &self.changes {
            change.cancel();
        }
    }
}

pub fn batch<T: Pending<Item = X, Result = Y> + 'static, X, Y>(
    changes: Vec<T>,
) -> Result<HashMap<usize, Y>, ChangeError<Command>> {
    let mut batch = PendingChangeBatch::<X, Y>::new();

    for f in changes {
        batch = batch.add(Box::new(f));
    }

    Box::new(batch).wait()
}
