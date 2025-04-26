use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use std::time::Duration;
use std::{future::Future, pin::Pin, sync::Arc};
use tokio_stream::{wrappers::errors::BroadcastStreamRecvError, Stream};

use crate::task::Status;
use crate::{
    notify::{ArcCounter, Notify},
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


#[derive(Default)]
pub struct Agent<S> {
    task: AsyncTask<(), Arc<Notify<S>>>,
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

impl<S: Clone + Send + Sync + 'static> Agent<S> {
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
    ) -> impl Stream<Item = Result<crate::task::Status<Result<S, StreamRecvError>>, StreamRecvError>>
    {
        futures::stream::unfold(
            (
                self.task.status().subscribe().await,
                std::option::Option::<
                    Pin<
                        Box<
                            dyn Stream<Item = Result<ArcCounter<S>, BroadcastStreamRecvError>>
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
                                    Ok(crate::task::Status::Running(Ok(next.deref().clone()))),
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
                                    Ok(crate::task::Status::Running(Ok(next.deref().clone()))),
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
    use tracing_test::traced_test;

    use crate::task::{Joinable, Status};

    use super::*;

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
                        assert_eq!(Some(Ok(Status::Running(Ok(10)))), agent_sub.next().await);
                        assert_eq!(Some(Ok(Status::Running(Ok(11)))), agent_sub.next().await);
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
