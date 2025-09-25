use tokio::sync::mpsc;
use tracing::{error, info};

pub struct NotificationQueue {
    receiver: mpsc::UnboundedReceiver<String>,
    worker_senders: Vec<mpsc::UnboundedSender<String>>,
    current_worker: usize,
}

impl NotificationQueue {
    pub fn new(
        receiver: mpsc::UnboundedReceiver<String>,
        worker_count: usize,
    ) -> (Self, Vec<mpsc::UnboundedReceiver<String>>) {
        let mut worker_senders = Vec::new();
        let mut worker_receivers = Vec::new();

        for _ in 0..worker_count {
            let (sender, receiver) = mpsc::unbounded_channel();
            worker_senders.push(sender);
            worker_receivers.push(receiver);
        }

        let queue = Self {
            receiver,
            worker_senders,
            current_worker: 0,
        };

        (queue, worker_receivers)
    }

    pub async fn start(&mut self) {
        info!(
            "Starting notification queue with {} workers",
            self.worker_senders.len()
        );

        while let Some(transaction_id) = self.receiver.recv().await {
            self.distribute_to_worker(transaction_id).await;
        }

        info!("Notification queue stopped");
    }

    async fn distribute_to_worker(&mut self, transaction_id: String) {
        let worker_index = self.current_worker;

        if let Some(sender) = self.worker_senders.get(worker_index) {
            if let Err(e) = sender.send(transaction_id.clone()) {
                error!(
                    "Failed to send transaction {} to worker {}: {}",
                    transaction_id, worker_index, e
                );
            } else {
                //info!("Sent transaction {} to worker {}", transaction_id, worker_index);
            }
        }

        self.current_worker = (self.current_worker + 1) % self.worker_senders.len();
    }
}
