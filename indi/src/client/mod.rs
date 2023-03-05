pub mod device;
pub mod notify;


use std::{sync::Arc, time::Instant, ops::Deref, collections::BTreeSet};

use crossbeam_channel::{Receiver, Select};

use crate::{serialization, Command, DeError, TypeError, Parameter, ToCommand, TryEq, PropertyState};

use super::notify::{Notify, Subscription};


#[derive(Debug)]
pub enum ChangeError<E> {
    NotifyError(notify::Error<E>),
    DeError(serialization::DeError),
    IoError(std::io::Error),
    Disconnected(crossbeam_channel::SendError<Command>),
    DisconnectedRecv(crossbeam_channel::TryRecvError),
    DisconnectedRecvTimeout(crossbeam_channel::RecvTimeoutError),
    Abort,
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
            notify::Error::Timeout => ChangeError::Abort,
            notify::Error::Canceled => ChangeError::Abort,
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

pub trait Pending {
    type Item;
    type Result;
    fn deadline(&self) -> Instant;
    fn receiver(&self) -> &Receiver<Self::Item>;
    fn tick(&self, item: Self::Item) -> Result<notify::Status<Self::Result>, ChangeError<Command>>;
    fn abort(&self);
}

pub struct PendingChangeImpl<P: Clone + TryEq<Parameter> + ToCommand<P> + 'static> {
    subscription: Subscription<Parameter>,
    param: Arc<Notify<Parameter>>,
    deadline: Instant,
    values: P,
}

impl<P: Clone + TryEq<Parameter> + ToCommand<P> + 'static> PendingChangeImpl<P> {

    pub fn wait(&self) -> Result<Arc<Parameter>, ChangeError<Command>> {
        let r = self
            .subscription
            .wait_fn::<Arc<Parameter>, ChangeError<Command>, _>(
                self.deadline() - Instant::now(),
                |param_lock| {
                    self.tick(param_lock)
                },
            )?;
        Ok(r)
    }
}
impl<P: Clone + TryEq<Parameter> + ToCommand<P> + 'static> Pending for PendingChangeImpl<P> {
    type Item = Arc<Parameter>;
    type Result = Arc<Parameter>;
    fn tick(&self, next: Arc<Parameter>) -> Result<notify::Status<Arc<Parameter>>, ChangeError<Command>> {
        if *next.get_state() == PropertyState::Alert {
            return Err(ChangeError::PropertyError);
        }
        if self.values.try_eq(&next)? {
            Ok(notify::Status::Complete(next.clone()))
        } else {
            Ok(notify::Status::Pending)
        }
    }

    fn deadline(&self) -> Instant {
        self.deadline
    }

    fn receiver(&self) -> &Receiver<Arc<Parameter>> {
        self.subscription.deref()
    }

    fn abort(&self) {
        self.param.cancel(&self.subscription);
    }
}
// pub struct PendingSequence {
//     pendings: Vec<Box<dyn Pending>>
// }
pub struct PendingChangeBatch<I, R> {
    changes: Vec<Box<dyn Pending<Item=I, Result=R>>>,
}

impl<I, R> PendingChangeBatch<I, R> {
    pub fn new() -> PendingChangeBatch<I, R>  {
        PendingChangeBatch {
            changes: Default::default(),
        }
    }

    pub fn add<T: Pending<Item=I, Result=R> + 'static>(mut self, pending_change: T) -> PendingChangeBatch<I, R>  {
        self.changes.push(Box::new(pending_change));
        self
    }

    pub fn wait(self) -> Result<R, ChangeError<Command>> {
        let mut sel = Select::new();
        let mut remaining = BTreeSet::new();
        for (i, r) in self.changes.iter().enumerate() {
            sel.recv(r.receiver());
            remaining.insert(i);
        }

        loop {
            let selected = sel.select();
            let i = selected.index();
            let r = selected.recv(self.changes[i].receiver()).unwrap();
            match self.changes[i].tick(r) {
                Ok(v) => {
                    if let notify::Status::Complete(v) = v {
                        remaining.remove(&i);
                     
                        if remaining.is_empty() {
                            return Ok(v);
                        }
                    }
                }
                Err(e) => {
                    for i in &remaining {
                        self.changes[*i].abort();
                    }
                    return Err(e);
                }
            }
        }
    }
}

pub fn batch<T: Pending<Item=X, Result=Y> + 'static, X, Y>( changes: Vec<T> ) -> Result<Y, ChangeError<Command>> {
    let mut batch = PendingChangeBatch::<X, Y>::new();

    for f in changes {
        batch = batch.add(f);
    }

    batch.wait()
}

