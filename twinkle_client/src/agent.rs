use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use std::time::Duration;
use std::{future::Future, pin::Pin, sync::Arc};
use tokio_stream::{wrappers::errors::BroadcastStreamRecvError, Stream};

use crate::notify::NotifyArc;
use crate::task::Status;
use crate::{
    notify::Notify,
    task::{AsyncTask, Task},
    MaybeSend,
};

/// An error returned from the inner stream of a [`BroadcastStream`].
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum StreamRecvError {
    /// The receiver lagged too far behind. Attempting to receive again will
    /// return the oldest message still retained by the channel.
    ///
    /// Includes the number of skipped messages.
    Lagged(u64),
}

impl From<BroadcastStreamRecvError> for StreamRecvError {
    fn from(value: BroadcastStreamRecvError) -> Self {
        match value {
            BroadcastStreamRecvError::Lagged(n) => StreamRecvError::Lagged(n),
        }
    }
}


pub struct Agent<S> {
    task: AsyncTask<(), Arc<Notify<S>>>,
}

impl<S> Default for Agent<S> {
    fn default() -> Self {
        Agent {
            task: AsyncTask::default()
        }
    }
}

impl<S> Deref for Agent<S> {
    type Target = AsyncTask<(), Arc<Notify<S>>>;
    fn deref(&self) -> &Self::Target {
        &self.task
    }
}

impl<S> DerefMut for Agent<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.task
    }
}

impl<S, E> From<Status<Result<S, E>>> for Result<Status<S>, E> {
    fn from(value: Status<Result<S, E>>) -> Self {
        match value {
            Status::Running(Ok(v)) => Ok(Status::Running(v)),
            Status::Running(Err(e)) => Err(e),
            Status::Completed => Ok(Status::Completed),
            Status::Aborted => Ok(Status::Aborted),
            Status::Pending => Ok(Status::Pending),
        }
    }
}

impl<S: Send + Sync + 'static> Agent<S> {
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.task = self.task.with_timeout(timeout);
        self
    }
    pub fn spawn<F: FnOnce(Arc<Notify<S>>) -> U, U: Future<Output = ()> + MaybeSend + 'static>(
        &mut self,
        state: S,
        func: F,
    ) {
        self.task
            .spawn(Arc::new(Notify::new(state)), |state| func(state.clone()));
    }
    pub fn spawn_timeout<
        F: FnOnce(Arc<Notify<S>>) -> U,
        U: Future<Output = ()> + MaybeSend + 'static,
    >(
        &mut self,
        timeout: Duration,
        state: S,
        func: F,
    ) {
        let mut notify = Notify::new(state);
        notify.set_timeout(timeout);
        self.task.spawn(
            Arc::new(notify),
            |state| func(state.clone()),
        );
    }
    
    pub async fn subscribe(
        &self,
    ) -> impl Stream<Item = Result<crate::task::Status<Result<NotifyArc<S>, StreamRecvError>>, StreamRecvError>>
    {
        futures::stream::unfold(
            (
                self.task.status().subscribe().await,
                std::option::Option::<
                    Pin<
                        Box<
                            dyn Stream<Item = Result<NotifyArc<S>, BroadcastStreamRecvError>>
                                + Send,
                        >,
                    >,
                >::None,
            ),
            move |(mut status_sub, status_notify_sub)| async move {
                if let Some(mut status_notify_sub) = status_notify_sub {
                    if let Some(next) = status_notify_sub.next().await {
                        match next {
                            Ok(next) => {
                                return Some((
                                    Ok(crate::task::Status::Running(Ok(next))),
                                    (status_sub, Some(status_notify_sub)),
                                ))
                            }
                            Err(e) => return Some((Err(e.into()), (status_sub, None))),
                        };
                    }
                }
                match status_sub.next().await? {
                    Ok(next) => match next.deref() {
                        crate::task::Status::Running(status_notify) => {
                            let mut status_notify_sub = status_notify.subscribe().await.boxed();

                            match status_notify_sub.next().await.unwrap() {
                                Ok(next) => Some((
                                    Ok(crate::task::Status::Running(Ok(next))),
                                    (status_sub, Some(status_notify_sub.boxed())),
                                )),
                                Err(e) => Some((Err(e.into()), (status_sub, None))),
                            }
                        }
                        crate::task::Status::Completed => {
                            Some((Ok(crate::task::Status::Completed), (status_sub, None)))
                        }
                        crate::task::Status::Aborted => {
                            Some((Ok(crate::task::Status::Aborted), (status_sub, None)))
                        }
                        crate::task::Status::Pending => {
                            Some((Ok(crate::task::Status::Pending), (status_sub, None)))
                        }
                    },
                    Err(e) => Some((Err(e.into()), (status_sub, None))),
                }
            },
        )
        .boxed()
    }
}

#[cfg(test)]
mod test {
    use std::future::pending;

    use tracing_test::traced_test;

    use crate::{notify::{self, wait_fn}, task::{Abortable, Joinable, Status}, OnDropFutureExt};

    use super::*;

    #[tokio::test]
    #[traced_test]
    async fn test_state_drop() {
        let value = Arc::new(0);
        let mut agent = Agent::<Arc<usize>>::default();
        
        let status = agent.status().read().await;
        match status.deref() {
            Status::Pending => {},
            _ => panic!("unexpected status"),
        }
        drop(status);

        assert_eq!(Arc::strong_count(&value), 1);

        agent.spawn(value.clone(), |v| {
            let v = v.clone();
            async move {
                pending::<()>().await;
                dbg!(v);
            }.on_drop(|| tracing::info!("dropped"))
        });

        let mut status_sub = agent.status().subscribe().await;
        wait_fn(&mut status_sub, Duration::from_secs(1), |status| {
            match status.deref() {
                Status::Running(_) => Result::<_, ()>::Ok(notify::Status::Complete(())),
                _ => Ok(notify::Status::Pending),
            }
        }).await.unwrap();
        let status = agent.status().read().await;
        match status.deref() {
            Status::Running(_) => {},
            _ => panic!("unexpected status"),
        }
        drop(status);

        agent.abort();
        wait_fn(&mut status_sub, Duration::from_secs(1), |status| {
            match status.deref() {
                Status::Aborted => Result::<_, ()>::Ok(notify::Status::Complete(())),
                _ => Ok(notify::Status::Pending),
            }
        }).await.unwrap();
        assert_eq!(Arc::strong_count(&value), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    // #[tokio::test]
    #[traced_test]
    async fn test_stress() {
        let mut stresses = vec![];
        for _ in 0..1 {
            stresses.push(tokio::task::spawn(async move {
                let mut agent: Agent<u64> = Agent::default().with_timeout(Duration::from_secs(100));

                let mut subscribers = vec![];
                for _ in 0..10 {
                    let mut agent_sub = agent.subscribe().await;

                    subscribers.push(tokio::task::spawn(async move {
                        assert_eq!(Some(Ok(Status::Pending)), agent_sub.next().await);
                        assert_eq!(Some(Ok(Status::Running(Ok(10.into())))), agent_sub.next().await);
                        assert_eq!(Some(Ok(Status::Running(Ok(11.into())))), agent_sub.next().await);
                        assert_eq!(Some(Ok(Status::Completed)), agent_sub.next().await);
                        assert_eq!(None, agent_sub.next().await);
                    }));
                }

                agent.spawn_timeout(Duration::from_secs(100), 10, |num| async move {
                    *num.write().await = 11;
                });
                let _ = agent.join().await;
                drop(agent);

                for sub in subscribers.into_iter() {
                    sub.await.unwrap();
                }
            }));
        }

        for (i, stress) in stresses.into_iter().enumerate() {
            stress.await.unwrap();

            if i % 1000 == 0 {
                dbg!(i);
            }
        }
    }
}
