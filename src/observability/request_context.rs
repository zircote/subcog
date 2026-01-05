//! Request context propagation for correlation IDs.

use std::cell::RefCell;
use std::future::Future;
use uuid::Uuid;

/// Per-request context with correlation ID.
#[derive(Clone, Debug)]
pub struct RequestContext {
    request_id: String,
}

impl RequestContext {
    /// Creates a new request context with a generated ID.
    #[must_use]
    pub fn new() -> Self {
        Self {
            request_id: Uuid::new_v4().to_string(),
        }
    }

    /// Creates a new request context with an existing request ID.
    #[must_use]
    pub fn from_id(request_id: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
        }
    }

    /// Returns the request ID.
    #[must_use]
    pub fn request_id(&self) -> &str {
        &self.request_id
    }
}

tokio::task_local! {
    static TASK_CONTEXT: RequestContext;
}

thread_local! {
    static THREAD_CONTEXT: RefCell<Option<RequestContext>> = RefCell::new(None);
}

/// Guard that restores the previous thread-local context on drop.
pub struct RequestContextGuard {
    previous: Option<RequestContext>,
}

impl Drop for RequestContextGuard {
    fn drop(&mut self) {
        THREAD_CONTEXT.with(|slot| {
            *slot.borrow_mut() = self.previous.take();
        });
    }
}

/// Enters a request context for synchronous flows.
#[must_use]
pub fn enter_request_context(context: RequestContext) -> RequestContextGuard {
    let previous = THREAD_CONTEXT.with(|slot| slot.borrow_mut().replace(context));
    RequestContextGuard { previous }
}

/// Scopes a request context across an async future.
pub async fn scope_request_context<F, T>(context: RequestContext, fut: F) -> T
where
    F: Future<Output = T>,
{
    TASK_CONTEXT
        .scope(context.clone(), async move {
            let _guard = enter_request_context(context);
            fut.await
        })
        .await
}

/// Returns the current request ID, if set.
#[must_use]
pub fn current_request_id() -> Option<String> {
    if let Ok(id) = TASK_CONTEXT.try_with(|ctx| ctx.request_id.clone()) {
        return Some(id);
    }

    THREAD_CONTEXT.with(|slot| slot.borrow().as_ref().map(|ctx| ctx.request_id.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_context_guard_propagates_request_id() {
        let context = RequestContext::from_id("thread-test");
        let _guard = enter_request_context(context);
        assert_eq!(current_request_id().as_deref(), Some("thread-test"));
    }

    #[tokio::test]
    async fn test_scope_request_context_propagates_across_await() {
        let context = RequestContext::from_id("async-test");
        let observed = scope_request_context(context, async {
            tokio::task::yield_now().await;
            current_request_id()
        })
        .await;
        assert_eq!(observed.as_deref(), Some("async-test"));
    }
}
