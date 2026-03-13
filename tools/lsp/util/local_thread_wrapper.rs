use std::pin::Pin;

use tokio::sync::mpsc;

pub struct LocalThreadWrapper<T> {
    sender: Option<
        mpsc::Sender<
            Box<dyn Send + for<'a> FnOnce(&'a mut T) -> Pin<Box<dyn Future<Output = ()> + 'a>>>,
        >,
    >,
    #[cfg(not(target_arch = "wasm32"))]
    join_handle: Arc<std::thread::JoinHandle<()>>,
}

impl<T: 'static> LocalThreadWrapper<T> {
    pub fn new(generator: impl FnOnce() -> T + Send + 'static) -> Self {
        let (sender, mut receiver) = mpsc::channel::<
            Box<dyn Send + for<'a> FnOnce(&'a mut T) -> Pin<Box<dyn Future<Output = ()> + 'a>>>,
        >(4);
        #[cfg(not(target_arch = "wasm32"))]
        let join_handle = std::thread::spawn(async move {
            let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
            let local = tokio::task::LocalSet::new();
            local.block_on(&rt, async move {
                let inner = generator();
                while let Some(task) = receiver.recv().await {
                    task(&mut inner).await;
                }
            });
        });
        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move {
            let mut inner = generator();
            while let Some(task) = receiver.recv().await {
                task(&mut inner).await;
            }
        });
        Self {
            sender: Some(sender),
            #[cfg(not(target_arch = "wasm32"))]
            join_handle: Arc::new(join_handle),
        }
    }

    pub async fn exec<R: Send + 'static>(self, f: impl FnOnce(&mut T) -> R + Send + 'static) -> R {
        let (response_sender, response_receiver) = tokio::sync::oneshot::channel();
        self.sender
            .as_ref()
            .unwrap()
            .send(Box::new(|r| {
                Box::pin(async move {
                    response_sender.send(f(r)).ok();
                })
            }))
            .await
            .ok();
        response_receiver.await.unwrap()
    }

    pub async fn exec_async<R: Send + 'static>(
        self,
        f: impl AsyncFnOnce(&mut T) -> R + Send + 'static,
    ) -> R {
        let (response_sender, response_receiver) = tokio::sync::oneshot::channel();
        self.sender
            .as_ref()
            .unwrap()
            .send(Box::new(|r| {
                Box::pin(async move {
                    response_sender.send(f(r).await).ok();
                })
            }))
            .await
            .ok();
        response_receiver.await.unwrap()
    }

    pub async fn with<R: Send + 'static>(&self, f: impl FnOnce(&mut T) -> R + Send + 'static) -> R {
        let (response_sender, response_receiver) = tokio::sync::oneshot::channel();
        self.sender
            .as_ref()
            .unwrap()
            .send(Box::new(|r| {
                Box::pin(async move {
                    response_sender.send(f(r)).ok();
                })
            }))
            .await
            .ok();
        response_receiver.await.unwrap()
    }

    pub async fn with_async<R: Send + 'static>(
        &self,
        f: impl AsyncFnOnce(&mut T) -> R + Send + 'static,
    ) -> R {
        let (response_sender, response_receiver) = tokio::sync::oneshot::channel();
        self.sender
            .as_ref()
            .unwrap()
            .send(Box::new(|r| {
                Box::pin(async move {
                    response_sender.send(f(r).await).ok();
                })
            }))
            .await
            .ok();
        response_receiver.await.unwrap()
    }

    #[cfg(test)]
    pub fn blocking_with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        tokio::runtime::Handle::current().block_on(async move {
            let (response_sender, response_receiver) = tokio::sync::oneshot::channel();
            self.sender
                .as_ref()
                .unwrap()
                .blocking_send(Box::new(|r| {
                    Box::pin(async move {
                        response_sender.send(f(r)).ok();
                    })
                }))
                .ok();
            response_receiver.await.unwrap()
        })
    }

    pub fn oneway(&self, f: impl FnOnce(&mut T) + Send + 'static) {
        let sender = self.sender.clone().unwrap();
        crate::spawn_local(async move {
            sender
                .send(Box::new(|r| {
                    Box::pin(async move {
                        f(r);
                    })
                }))
                .await
                .ok();
        });
    }

    pub fn oneway_async(&self, f: impl AsyncFnOnce(&mut T) + Send + 'static) {
        let sender = self.sender.clone().unwrap();
        crate::spawn_local(async move {
            sender.send(Box::new(|r| Box::pin(f(r)))).await.ok();
        });
    }
}

impl<T> Clone for LocalThreadWrapper<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            #[cfg(not(target_arch = "wasm32"))]
            join_handle: self.join_handle.clone(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<T> Drop for LocalThreadWrapper<T> {
    fn drop(&mut self) {
        self.sender = None;
        if let Ok(join_handle) = self.join_handle.try_unwrap() {
            join_handle.join().ok();
        }
    }
}
