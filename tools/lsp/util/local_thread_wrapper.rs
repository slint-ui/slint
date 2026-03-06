use std::pin::Pin;

use tokio::sync::mpsc;

pub struct LocalThreadWrapper<T>(
    mpsc::UnboundedSender<
        Box<dyn Send + for<'a> FnOnce(&'a mut T) -> Pin<Box<dyn Future<Output = ()> + 'a>>>,
    >,
);

impl<T> LocalThreadWrapper<T> {
    pub fn new_local(mut inner: T) -> Self
    where
        T: 'static,
    {
        let (sender, mut receiver) = mpsc::unbounded_channel::<
            Box<dyn Send + for<'a> FnOnce(&'a mut T) -> Pin<Box<dyn Future<Output = ()> + 'a>>>,
        >();
        tokio::task::spawn_local(async move {
            while let Some(task) = receiver.recv().await {
                task(&mut inner).await;
            }
        });
        Self(sender)
    }

    pub async fn exec<R: Send + 'static>(self, f: impl FnOnce(&mut T) -> R + Send + 'static) -> R {
        let (response_sender, response_receiver) = tokio::sync::oneshot::channel();
        self.0
            .send(Box::new(|r| {
                Box::pin(async move {
                    response_sender.send(f(r)).ok();
                })
            }))
            .ok();
        response_receiver.await.unwrap()
    }

    pub async fn exec_async<R: Send + 'static>(
        self,
        f: impl AsyncFnOnce(&mut T) -> R + Send + 'static,
    ) -> R {
        let (response_sender, response_receiver) = tokio::sync::oneshot::channel();
        self.0
            .send(Box::new(|r| {
                Box::pin(async move {
                    response_sender.send(f(r).await).ok();
                })
            }))
            .ok();
        response_receiver.await.unwrap()
    }

    pub async fn with<R: Send + 'static>(&self, f: impl FnOnce(&mut T) -> R + Send + 'static) -> R {
        let (response_sender, response_receiver) = tokio::sync::oneshot::channel();
        self.0
            .send(Box::new(|r| {
                Box::pin(async move {
                    response_sender.send(f(r)).ok();
                })
            }))
            .ok();
        response_receiver.await.unwrap()
    }

    pub async fn with_async<R: Send + 'static>(
        &self,
        f: impl AsyncFnOnce(&mut T) -> R + Send + 'static,
    ) -> R {
        let (response_sender, response_receiver) = tokio::sync::oneshot::channel();
        self.0
            .send(Box::new(|r| {
                Box::pin(async move {
                    response_sender.send(f(r).await).ok();
                })
            }))
            .ok();
        response_receiver.await.unwrap()
    }

    pub fn oneway(&self, f: impl FnOnce(&mut T) + Send + 'static) {
        self.0
            .send(Box::new(|r| {
                Box::pin(async move {
                    f(r);
                })
            }))
            .ok();
    }

    pub fn oneway_async(&self, f: impl AsyncFnOnce(&mut T) + Send + 'static) {
        self.0.send(Box::new(|r| Box::pin(f(r)))).ok();
    }
}

impl<T> Clone for LocalThreadWrapper<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
