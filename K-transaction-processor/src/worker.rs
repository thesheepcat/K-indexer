use crate::config::AppConfig;
use crate::database::{DbPool, Transaction, fetch_transaction};
use crate::k_protocol::KProtocolProcessor;
use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

pub struct Worker {
    id: usize,
    receiver: mpsc::UnboundedReceiver<String>,
    db_pool: DbPool,
    config: AppConfig,
    k_processor: KProtocolProcessor,
}

impl Worker {
    pub fn new(
        id: usize,
        receiver: mpsc::UnboundedReceiver<String>,
        db_pool: DbPool,
        config: AppConfig,
    ) -> Self {
        let k_processor = KProtocolProcessor::new(db_pool.clone());
        Self {
            id,
            receiver,
            db_pool,
            config,
            k_processor,
        }
    }

    pub async fn start(mut self) {
        info!("Worker {} started", self.id);

        while let Some(transaction_id) = self.receiver.recv().await {
            //info!("Worker {} received notification for transaction: {}", self.id, transaction_id);

            self.process_transaction(transaction_id).await;
        }

        info!("Worker {} stopped", self.id);
    }

    async fn process_transaction(&self, transaction_id: String) {
        //info!("Worker {} processing transaction: {}", self.id, transaction_id);

        match self.fetch_and_process_transaction(&transaction_id).await {
            Ok(Some(transaction)) => {
                // Process K protocol if payload starts with k:1:
                if let Some(ref payload_hex) = transaction.payload {
                    if let Ok(payload_bytes) = hex::decode(payload_hex) {
                        if let Ok(payload_str) = std::str::from_utf8(&payload_bytes) {
                            if payload_str.starts_with("k:1:") {
                                //info!("Worker {} - Processing K protocol transaction: {}", self.id, transaction_id);
                                if let Err(k_err) =
                                    self.k_processor.process_k_transaction(&transaction).await
                                {
                                    error!(
                                        "Worker {} - Error processing K protocol transaction {}: {}",
                                        self.id, transaction_id, k_err
                                    );
                                }
                            } else {
                                info!(
                                    "Worker {} - Transaction {} does not contain K protocol data",
                                    self.id, transaction_id
                                );
                            }
                        }
                    }
                }
            }
            Ok(None) => {
                warn!(
                    "Worker {} - Transaction {} not found in database",
                    self.id, transaction_id
                );
            }
            Err(e) => {
                error!(
                    "Worker {} - Error processing transaction {}: {}",
                    self.id, transaction_id, e
                );

                if let Err(retry_err) = self.retry_transaction(&transaction_id).await {
                    error!(
                        "Worker {} - Failed to retry transaction {}: {}",
                        self.id, transaction_id, retry_err
                    );
                }
            }
        }
    }

    async fn fetch_and_process_transaction(
        &self,
        transaction_id: &str,
    ) -> Result<Option<Transaction>> {
        //info!("Worker {} received transaction data for processing: {}", self.id, transaction_id);

        fetch_transaction(&self.db_pool, transaction_id).await
    }

    /*
    fn log_transaction(&self, transaction: &Transaction) {
        info!("=== Transaction Processed by Worker {} ===", self.id);
        //info!("Worker {} - Transaction ID (hex): {}", self.id, transaction.transaction_id);
        //info!("Worker {} - Subnetwork ID: {:?}", self.id, transaction.subnetwork_id);
        //info!("Worker {} - Hash (hex): {:?}", self.id, transaction.hash);
        //info!("Worker {} - Mass: {:?}", self.id, transaction.mass);

        // Format payload with hex prefix and check if it starts with 6b3a

        if let Some(ref payload) = transaction.payload {
            info!("Worker {} - Payload (hex): 0x{}", self.id, payload);
            if payload.starts_with("6b3a") {
                info!("Worker {} - Payload starts with 6b3a ✓ (trigger condition met)", self.id);
            }
            info!("Worker {} - Payload length: {} bytes", self.id, payload.len() / 2);
        } else {
            info!("Worker {} - Payload (hex): None", self.id);
        }

        info!("Worker {} - Block Time: {:?}", self.id, transaction.block_time);
        info!("Worker {} - ==========================================", self.id);

    }*/

    async fn retry_transaction(&self, transaction_id: &str) -> Result<()> {
        for attempt in 1..=self.config.processing.retry_attempts {
            warn!(
                "Worker {} - Retry attempt {} for transaction {}",
                self.id, attempt, transaction_id
            );

            tokio::time::sleep(tokio::time::Duration::from_millis(
                self.config.processing.retry_delay_ms,
            ))
            .await;

            match self.fetch_and_process_transaction(transaction_id).await {
                Ok(Some(transaction)) => {
                    info!(
                        "Worker {} - Retry successful for transaction {}",
                        self.id, transaction_id
                    );
                    // Process K protocol if payload starts with k:1:
                    if let Some(ref payload_hex) = transaction.payload {
                        if let Ok(payload_bytes) = hex::decode(payload_hex) {
                            if let Ok(payload_str) = std::str::from_utf8(&payload_bytes) {
                                if payload_str.starts_with("k:1:") {
                                    //info!("Worker {} - Processing K protocol transaction on retry: {}", self.id, transaction_id);
                                    if let Err(k_err) =
                                        self.k_processor.process_k_transaction(&transaction).await
                                    {
                                        error!(
                                            "Worker {} - Error processing K protocol transaction on retry {}: {}",
                                            self.id, transaction_id, k_err
                                        );
                                    }
                                }
                            }
                        }
                    }
                    return Ok(());
                }
                Ok(None) => {
                    warn!(
                        "Worker {} - Transaction {} still not found on retry {}",
                        self.id, transaction_id, attempt
                    );
                }
                Err(e) => {
                    error!(
                        "Worker {} - Retry {} failed for transaction {}: {}",
                        self.id, attempt, transaction_id, e
                    );
                }
            }
        }

        error!(
            "Worker {} - All retry attempts exhausted for transaction {}",
            self.id, transaction_id
        );
        Ok(())
    }
}

pub struct WorkerPool {
    workers: Vec<Worker>,
}

impl WorkerPool {
    pub fn new(
        worker_receivers: Vec<mpsc::UnboundedReceiver<String>>,
        db_pool: DbPool,
        config: AppConfig,
    ) -> Self {
        let workers = worker_receivers
            .into_iter()
            .enumerate()
            .map(|(id, receiver)| Worker::new(id, receiver, db_pool.clone(), config.clone()))
            .collect();

        Self { workers }
    }

    pub async fn start(self) {
        info!("Starting worker pool with {} workers", self.workers.len());

        let mut handles = Vec::new();

        for worker in self.workers {
            let handle = tokio::spawn(async move {
                worker.start().await;
            });
            handles.push(handle);
        }

        for handle in handles {
            if let Err(e) = handle.await {
                error!("Worker task failed: {}", e);
            }
        }

        info!("Worker pool stopped");
    }
}
