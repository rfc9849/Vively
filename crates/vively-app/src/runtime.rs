use std::{future::Future, sync::Arc};

use futures_channel::oneshot;

#[derive(Clone)]
pub struct TokioBridge {
    runtime: Arc<tokio::runtime::Runtime>,
}

impl TokioBridge {
    pub fn new() -> std::io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()?;
        Ok(Self {
            runtime: Arc::new(runtime),
        })
    }

    pub fn handle(&self) -> tokio::runtime::Handle {
        self.runtime.handle().clone()
    }

    /// Run `fut` on the runtime and hand back a receiver for its output.
    pub fn spawn_oneshot<T, F>(&self, fut: F) -> oneshot::Receiver<T>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        self.runtime.spawn(async move {
            let out = fut.await;
            let _ = tx.send(out);
        });
        rx
    }
}
