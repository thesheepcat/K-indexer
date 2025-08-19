use std::sync::Arc;

use kaspa_wrpc_client::prelude::*;
use kaspa_wrpc_client::result::Result;
use workflow_log::prelude::*;

use crate::kaspa_connection::Inner;

#[derive(Clone)]
pub struct NotificationHandler {
    inner: Arc<Inner>,
}

impl NotificationHandler {
    pub fn new(inner: Arc<Inner>) -> Self {
        Self { inner }
    }

    // generic notification handler fn called by the event task
    pub async fn handle_notification(&self, notification: Notification) -> Result<()> {
        match notification {
            Notification::BlockAdded(block_notification) => {
                self.extract_k_protocol_transactions(block_notification)
                    .await
            }
            _ => Ok(()),
        }
    }

    async fn extract_k_protocol_transactions(
        &self,
        block_notification: BlockAddedNotification,
    ) -> Result<()> {
        for transaction in &block_notification.block.transactions {
            if transaction.payload.len() >= 4 {
                // K protocol prefix (k:1:) in uint8: 107 58 49 58
                let K_PROTOCOL_PREFIX = [107, 58, 49, 58];
                if transaction.payload[0] == K_PROTOCOL_PREFIX[0]
                    && transaction.payload[1] == K_PROTOCOL_PREFIX[1]
                    && transaction.payload[2] == K_PROTOCOL_PREFIX[2]
                    && transaction.payload[3] == K_PROTOCOL_PREFIX[3]
                {
                    // Send the complete transaction to the K processor channel
                    if let Err(err) = self
                        .inner
                        .k_transaction_channel
                        .sender
                        .send(transaction.clone())
                        .await
                    {
                        log_error!("Failed to send K transaction to processor: {err}");
                    }
                }
            }
        }
        Ok(())
    }
}
