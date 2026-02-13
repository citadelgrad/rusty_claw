//! Pending request tracking for control protocol
//!
//! This module manages the lifecycle of outgoing control requests,
//! tracking pending requests by their UUID and routing responses back
//! to the waiting caller via oneshot channels.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐
//! │  request()  │ → Generate UUID, insert sender, send to CLI
//! └──────┬──────┘
//!        │
//!        ├─ Pending: HashMap<String, oneshot::Sender>
//!        │
//!        ↓
//! ┌──────────────────┐
//! │ handle_response()│ → Remove sender, send response
//! └──────────────────┘
//! ```
//!
//! # Example
//!
//! ```
//! use rusty_claw::control::pending::PendingRequests;
//! use rusty_claw::control::messages::ControlResponse;
//! use tokio::sync::oneshot;
//! use serde_json::json;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let pending = PendingRequests::new();
//!
//! let (tx, rx) = oneshot::channel();
//! pending.insert("req_123".to_string(), tx).await;
//!
//! // ... send request to CLI ...
//!
//! // Later, when response arrives:
//! let response = ControlResponse::Success { data: json!({}) };
//! pending.complete("req_123", response.clone()).await;
//!
//! // Caller receives the response:
//! let received = rx.await.unwrap();
//! # }
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};

use crate::control::messages::ControlResponse;

/// Tracks pending control requests awaiting responses
///
/// Uses a shared HashMap to store oneshot senders, keyed by request UUID.
/// When a response arrives from the CLI, the corresponding sender is removed
/// and used to deliver the response to the waiting caller.
///
/// # Thread Safety
///
/// The internal HashMap is protected by a Tokio Mutex, allowing safe
/// concurrent access from multiple tasks.
#[derive(Clone)]
pub struct PendingRequests {
    /// Map of request_id → response sender
    inner: Arc<Mutex<HashMap<String, oneshot::Sender<ControlResponse>>>>,
}

impl PendingRequests {
    /// Create a new empty pending requests tracker
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Insert a new pending request
    ///
    /// Stores the oneshot sender for the given request ID. When the response
    /// arrives, [`complete`](Self::complete) will use this sender to deliver
    /// the response.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique request identifier (UUID)
    /// * `sender` - Oneshot channel sender for delivering the response
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::control::pending::PendingRequests;
    /// use tokio::sync::oneshot;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let pending = PendingRequests::new();
    /// let (tx, rx) = oneshot::channel();
    /// pending.insert("req_123".to_string(), tx).await;
    /// # }
    /// ```
    pub async fn insert(&self, id: String, sender: oneshot::Sender<ControlResponse>) {
        self.inner.lock().await.insert(id, sender);
    }

    /// Complete a pending request with a response
    ///
    /// Removes the request from the pending map and sends the response
    /// to the waiting caller via the oneshot channel.
    ///
    /// # Arguments
    ///
    /// * `id` - Request identifier to complete
    /// * `response` - Response to send to the waiting caller
    ///
    /// # Returns
    ///
    /// * `true` - Request was found and response was sent successfully
    /// * `false` - Request was not found or receiver was dropped
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::control::pending::PendingRequests;
    /// use rusty_claw::control::messages::ControlResponse;
    /// use tokio::sync::oneshot;
    /// use serde_json::json;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let pending = PendingRequests::new();
    /// let (tx, rx) = oneshot::channel();
    /// pending.insert("req_123".to_string(), tx).await;
    ///
    /// let response = ControlResponse::Success { data: json!({}) };
    /// let sent = pending.complete("req_123", response).await;
    /// assert!(sent);
    /// # }
    /// ```
    pub async fn complete(&self, id: &str, response: ControlResponse) -> bool {
        if let Some(sender) = self.inner.lock().await.remove(id) {
            sender.send(response).is_ok()
        } else {
            false
        }
    }

    /// Cancel a pending request
    ///
    /// Removes the request from the pending map without sending a response.
    /// This is typically used when a request times out or the caller drops
    /// the receiver.
    ///
    /// # Arguments
    ///
    /// * `id` - Request identifier to cancel
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::control::pending::PendingRequests;
    /// use tokio::sync::oneshot;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let pending = PendingRequests::new();
    /// let (tx, rx) = oneshot::channel();
    /// pending.insert("req_123".to_string(), tx).await;
    ///
    /// // Request timed out, cancel it
    /// pending.cancel("req_123").await;
    /// # }
    /// ```
    pub async fn cancel(&self, id: &str) {
        self.inner.lock().await.remove(id);
    }

    /// Get the number of pending requests
    ///
    /// Useful for monitoring and testing.
    #[cfg(test)]
    pub async fn len(&self) -> usize {
        self.inner.lock().await.len()
    }
}

impl Default for PendingRequests {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_insert_and_complete() {
        let pending = PendingRequests::new();
        let (tx, rx) = oneshot::channel();

        pending.insert("req_1".to_string(), tx).await;
        assert_eq!(pending.len().await, 1);

        let response = ControlResponse::Success {
            data: json!({ "result": "ok" }),
        };

        let sent = pending.complete("req_1", response.clone()).await;
        assert!(sent);
        assert_eq!(pending.len().await, 0);

        let received = rx.await.unwrap();
        assert_eq!(received, response);
    }

    #[tokio::test]
    async fn test_complete_nonexistent() {
        let pending = PendingRequests::new();

        let response = ControlResponse::Success {
            data: json!({ "result": "ok" }),
        };

        let sent = pending.complete("nonexistent", response).await;
        assert!(!sent);
    }

    #[tokio::test]
    async fn test_cancel() {
        let pending = PendingRequests::new();
        let (tx, _rx) = oneshot::channel();

        pending.insert("req_1".to_string(), tx).await;
        assert_eq!(pending.len().await, 1);

        pending.cancel("req_1").await;
        assert_eq!(pending.len().await, 0);
    }

    #[tokio::test]
    async fn test_cancel_nonexistent() {
        let pending = PendingRequests::new();
        pending.cancel("nonexistent").await; // Should not panic
    }

    #[tokio::test]
    async fn test_multiple_pending() {
        let pending = PendingRequests::new();

        let (tx1, mut rx1) = oneshot::channel();
        let (tx2, rx2) = oneshot::channel();
        let (tx3, mut rx3) = oneshot::channel();

        pending.insert("req_1".to_string(), tx1).await;
        pending.insert("req_2".to_string(), tx2).await;
        pending.insert("req_3".to_string(), tx3).await;
        assert_eq!(pending.len().await, 3);

        let response2 = ControlResponse::Success {
            data: json!({ "id": 2 }),
        };
        pending.complete("req_2", response2.clone()).await;

        assert_eq!(pending.len().await, 2);

        let received2 = rx2.await.unwrap();
        assert_eq!(received2, response2);

        // rx1 and rx3 should still be pending
        assert!(rx1.try_recv().is_err());
        assert!(rx3.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_complete_after_receiver_dropped() {
        let pending = PendingRequests::new();
        let (tx, rx) = oneshot::channel();

        pending.insert("req_1".to_string(), tx).await;
        drop(rx); // Receiver dropped

        let response = ControlResponse::Success {
            data: json!({ "result": "ok" }),
        };

        let sent = pending.complete("req_1", response).await;
        assert!(!sent); // Should return false because receiver was dropped
        assert_eq!(pending.len().await, 0);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let pending = PendingRequests::new();

        let mut handles = vec![];

        // Spawn 10 tasks that insert and complete requests concurrently
        for i in 0..10 {
            let pending_clone = pending.clone();
            let handle = tokio::spawn(async move {
                let (tx, rx) = oneshot::channel();
                let id = format!("req_{}", i);

                pending_clone.insert(id.clone(), tx).await;

                let response = ControlResponse::Success {
                    data: json!({ "id": i }),
                };

                pending_clone.complete(&id, response.clone()).await;

                rx.await.unwrap()
            });
            handles.push(handle);
        }

        // Wait for all tasks and verify responses
        for (i, handle) in handles.into_iter().enumerate() {
            let response = handle.await.unwrap();
            match response {
                ControlResponse::Success { data } => {
                    assert_eq!(data["id"], i);
                }
                _ => panic!("Wrong response type"),
            }
        }

        // All requests should be completed
        assert_eq!(pending.len().await, 0);
    }
}
