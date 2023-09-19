use std::thread::{self, ThreadId};

/// Primitive for checking that some actions remain bound to a specific
/// thread.
pub struct SameThread(ThreadId);

impl Default for SameThread {
    fn default() -> Self {
        SameThread(thread::current().id())
    }
}

impl SameThread {
    /// Panic if this is ever called from a different thread than where self
    /// was created.
    pub fn assert(&self) {
        // Optimization, don't keep calling current thread ID function.
        thread_local! {
            static CURRENT_THREAD_ID: std::thread::ThreadId = std::thread::current().id();
        }

        assert!(
            CURRENT_THREAD_ID.with(|id| *id) == self.0,
            "SameThread::assert called from different thread"
        );
    }
}
