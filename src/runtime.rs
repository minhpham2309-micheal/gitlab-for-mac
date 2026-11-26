use std::future::Future;
use std::sync::OnceLock;

use gpui_kit::{App, Global};
use tokio::runtime::{Builder, Handle, Runtime};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

static TOKIO_RUNTIME: OnceLock<Runtime> = OnceLock::new();

pub fn runtime_handle() -> Handle {
    TOKIO_RUNTIME
        .get_or_init(|| {
            Builder::new_multi_thread()
                .worker_threads(2)
                .thread_name("glab-tokio")
                .enable_all()
                .build()
                .expect("failed to initialize Tokio runtime")
        })
        .handle()
        .clone()
}

struct AbortOnDrop(Option<JoinHandle<()>>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            handle.abort();
        }
    }
}

#[derive(Clone)]
pub struct TokioBridge {
    handle: Handle,
}

impl Global for TokioBridge {}

impl TokioBridge {
    pub fn init(cx: &mut App) {
        if cx.try_global::<Self>().is_none() {
            cx.set_global(Self {
                handle: runtime_handle(),
            });
        }
    }

    pub fn spawn<F, T>(&self, fut: F) -> impl Future<Output = Result<T, String>> + Send + 'static
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        spawn_on(&self.handle, fut)
    }
}

pub fn spawn_tokio<F, T>(fut: F) -> impl Future<Output = Result<T, String>> + Send + 'static
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let handle = runtime_handle();
    spawn_on(&handle, fut)
}

fn spawn_on<F, T>(
    handle: &Handle,
    fut: F,
) -> impl Future<Output = Result<T, String>> + Send + 'static
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = oneshot::channel();
    let join = handle.spawn(async move {
        let value = fut.await;
        let _ = tx.send(value);
    });
    let guard = AbortOnDrop(Some(join));
    async move {
        let _guard = guard;
        rx.await
            .map_err(|_| "background task cancelled".to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;

    #[test]
    fn executes_tokio_future_and_returns_value() {
        let rt = runtime_handle();
        let val = rt.block_on(async {
            spawn_tokio(async {
                tokio::time::sleep(Duration::from_millis(5)).await;
                42usize
            })
            .await
            .unwrap()
        });
        assert_eq!(val, 42);
    }

    #[test]
    fn dropping_future_aborts_tokio_task() {
        struct DropFlag(Arc<AtomicBool>);
        impl Drop for DropFlag {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }

        let dropped = Arc::new(AtomicBool::new(false));
        let guard = DropFlag(Arc::clone(&dropped));
        let fut = spawn_tokio(async move {
            let _guard = guard;
            tokio::time::sleep(Duration::from_secs(30)).await;
        });
        drop(fut);

        let rt = runtime_handle();
        rt.block_on(async {
            for _ in 0..40 {
                if dropped.load(Ordering::SeqCst) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });
        assert!(dropped.load(Ordering::SeqCst));
    }
}
