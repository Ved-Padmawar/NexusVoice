//! Running backend calls from views. The call runs on the tokio runtime; the
//! view is updated back on the UI thread when it finishes, if it still exists.

use std::future::Future;

use gpui_kit::{Context, Window};

pub trait SpawnApi<V: 'static> {
    /// Run `call` on the backend runtime, then hand its result to `done`.
    fn spawn_api<T, F>(&mut self, call: F, done: impl FnOnce(&mut V, T, &mut Context<V>) + 'static)
    where
        T: Send + 'static,
        F: Future<Output = T> + Send + 'static;

    /// Like [`Self::spawn_api`], with the view's window.
    fn spawn_api_in<T, F>(
        &mut self,
        window: &Window,
        call: F,
        done: impl FnOnce(&mut V, T, &mut Window, &mut Context<V>) + 'static,
    ) where
        T: Send + 'static,
        F: Future<Output = T> + Send + 'static;
}

impl<V: 'static> SpawnApi<V> for Context<'_, V> {
    fn spawn_api<T, F>(&mut self, call: F, done: impl FnOnce(&mut V, T, &mut Context<V>) + 'static)
    where
        T: Send + 'static,
        F: Future<Output = T> + Send + 'static,
    {
        let task = crate::runtime::spawn(call);
        self.spawn(async move |this, cx| {
            let Ok(result) = task.await else { return };
            let _ = this.update(cx, |view, cx| done(view, result, cx));
        })
        .detach();
    }

    fn spawn_api_in<T, F>(
        &mut self,
        window: &Window,
        call: F,
        done: impl FnOnce(&mut V, T, &mut Window, &mut Context<V>) + 'static,
    ) where
        T: Send + 'static,
        F: Future<Output = T> + Send + 'static,
    {
        let task = crate::runtime::spawn(call);
        self.spawn_in(window, async move |this, cx| {
            let Ok(result) = task.await else { return };
            let _ = this.update_in(cx, |view, window, cx| done(view, result, window, cx));
        })
        .detach();
    }
}
