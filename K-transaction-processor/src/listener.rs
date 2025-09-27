use crate::config::AppConfig;
use anyhow::Result;
use sqlx::{Error as SqlxError, postgres::PgListener};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

pub struct NotificationListener {
    config: AppConfig,
    notification_sender: mpsc::UnboundedSender<String>,
}

impl NotificationListener {
    pub fn new(config: AppConfig, notification_sender: mpsc::UnboundedSender<String>) -> Self {
        Self {
            config,
            notification_sender,
        }
    }

    pub async fn start(&self) -> Result<()> {
        loop {
            match self.connect_and_listen().await {
                Ok(_) => {
                    info!("Notification listener stopped gracefully");
                    break;
                }
                Err(e) => {
                    error!("Notification listener error: {}", e);
                    warn!(
                        "Reconnecting in {} ms",
                        self.config.processing.retry_delay_ms
                    );
                    tokio::time::sleep(tokio::time::Duration::from_millis(
                        self.config.processing.retry_delay_ms,
                    ))
                    .await;
                }
            }
        }
        Ok(())
    }

    async fn connect_and_listen(&self) -> Result<(), SqlxError> {
        // Create connection string
        let connection_string = self.config.connection_string();

        // Create a PostgreSQL listener
        let mut listener = PgListener::connect(&connection_string).await?;

        info!("Connected to database for notifications");

        // Subscribe to the channel
        listener
            .listen(&self.config.processing.channel_name)
            .await?;
        info!(
            "Listening on channel: {}",
            self.config.processing.channel_name
        );

        let notification_sender = self.notification_sender.clone();

        info!("Notification listener is now active and waiting for database triggers");

        // Process notifications
        loop {
            // Wait for a notification
            match listener.recv().await {
                Ok(notification) => {
                    //info!("Listener received notification on channel '{}' with payload: '{}'", notification.channel(), notification.payload());

                    // Send the transaction ID to the processing queue
                    let payload = notification.payload().to_string();
                    if let Err(e) = notification_sender.send(payload) {
                        error!("Failed to send notification to queue: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    error!("Error receiving notification: {}", e);
                    return Err(e);
                }
            }
        }

        Ok(())
    }
}
