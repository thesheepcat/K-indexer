use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use polodb_core::Collection;
use workflow_core::channel::{Channel, DuplexChannel};
use workflow_log::prelude::*;
use futures::FutureExt;

use kaspa_wrpc_client::prelude::*;
use kaspa_wrpc_client::result::Result;

use crate::models::{KPostRecord, KReplyRecord, KBroadcastRecord, KVoteRecord};
use crate::notification_handler::NotificationHandler;
use crate::transaction_processor::TransactionProcessor;

pub struct Inner {
    // task control duplex channel - a pair of channels where sender
    // is used to signal an async task termination request and receiver
    // is used to signal task termination completion.
    pub task_ctl: DuplexChannel<()>,
    // Kaspa wRPC client instance
    pub client: Arc<KaspaRpcClient>,
    // our own view on the connection state
    pub is_connected: AtomicBool,
    // channel supplied to the notification subsystem
    // to receive the node notifications we subscribe to
    pub notification_channel: Channel<Notification>,
    // listener id used to manage notification scopes
    // we can have multiple IDs for different scopes
    // paired with multiple notification channels
    pub listener_id: Mutex<Option<ListenerId>>,
    // K protocol collections
    pub k_posts_collection: Collection<KPostRecord>,
    pub k_replies_collection: Collection<KReplyRecord>,
    pub k_broadcasts_collection: Collection<KBroadcastRecord>,
    pub k_votes_collection: Collection<KVoteRecord>,
    // channel for processing K protocol transactions
    pub k_transaction_channel: Channel<RpcTransaction>,
    // task control for K transaction processor
    pub k_processor_task_ctl: DuplexChannel<()>,
}

// Connection manager that handles RPC connection and
// runs its own event task to handle RPC connection
// events and node notifications we subscribe to.
#[derive(Clone)]
pub struct KaspaConnection {
    pub inner: Arc<Inner>,
    notification_handler: NotificationHandler,
    transaction_processor: TransactionProcessor,
}

impl KaspaConnection {
    pub fn try_new(
        network_id: NetworkId,
        url: Option<String>,
        k_posts_collection: Collection<KPostRecord>,
        k_replies_collection: Collection<KReplyRecord>,
        k_broadcasts_collection: Collection<KBroadcastRecord>,
        k_votes_collection: Collection<KVoteRecord>,
    ) -> Result<Self> {
        // if not url is supplied we use the default resolver to
        // obtain the public node rpc endpoint
        let (resolver, url) = if let Some(url) = url {
            (None, Some(url))
        } else {
            (Some(Resolver::default()), None)
        };

        // Create a basic Kaspa RPC client instance using Borsh encoding.
        let client = Arc::new(KaspaRpcClient::new_with_args(
            WrpcEncoding::Borsh,
            url.as_deref(),
            resolver,
            Some(network_id),
            None,
        )?);

        let inner = Inner {
            task_ctl: DuplexChannel::oneshot(),
            client,
            is_connected: AtomicBool::new(false),
            notification_channel: Channel::unbounded(),
            listener_id: Mutex::new(None),
            k_posts_collection,
            k_replies_collection,
            k_broadcasts_collection,
            k_votes_collection,
            k_transaction_channel: Channel::unbounded(),
            k_processor_task_ctl: DuplexChannel::oneshot(),
        };

        let inner_arc = Arc::new(inner);
        let notification_handler = NotificationHandler::new(inner_arc.clone());
        let transaction_processor = TransactionProcessor::new(inner_arc.clone());

        Ok(Self {
            inner: inner_arc,
            notification_handler,
            transaction_processor,
        })
    }

    // Helper fn to check if we are currently connected
    // to the node. This only represents our own view of
    // the connection state (i.e. if in a different setup
    // our event task is shutdown, the RPC client may remain
    // connected.
    pub fn is_connected(&self) -> bool {
        self.inner.is_connected.load(Ordering::SeqCst)
    }

    // Start the connection
    pub async fn start(&self) -> Result<()> {
        // we do not block the async connect() function
        // as we handle the connection state in the event task
        let options = ConnectOptions {
            block_async_connect: false,
            ..Default::default()
        };

        // start the K transaction processor task
        self.transaction_processor.start_task().await?;

        // start the event processing task
        self.start_event_task().await?;

        // start the RPC connection. this will initiate an RPC connection background task that will
        // continuously try to connect to the given URL or query a URL from the resolver if one is provided.
        self.client().connect(Some(options)).await?;

        Ok(())
    }

    // Stop the connection
    pub async fn stop(&self) -> Result<()> {
        // Disconnect the RPC client
        self.client().disconnect().await?;
        // make sure to stop the event task after the RPC client is disconnected to receive and handle disconnection events.
        self.stop_event_task().await?;
        // stop the K transaction processor task
        self.transaction_processor.stop_task().await?;
        Ok(())
    }

    pub fn client(&self) -> &Arc<KaspaRpcClient> {
        &self.inner.client
    }

    async fn register_notification_listeners(&self) -> Result<()> {
        // IMPORTANT: notification scopes are managed by the node
        // for the lifetime of the RPC connection, as such they
        // are "lost" if we disconnect. For that reason we must
        // re-register all notification scopes when we connect.

        let listener_id = self
            .client()
            .rpc_api()
            .register_new_listener(ChannelConnection::new(
                "wrpc-example-subscriber",
                self.inner.notification_channel.sender.clone(),
                ChannelType::Persistent,
            ));
        *self.inner.listener_id.lock().unwrap() = Some(listener_id);
        self.client()
            .rpc_api()
            .start_notify(listener_id, Scope::BlockAdded(BlockAddedScope {}))
            .await?;
        Ok(())
    }

    async fn unregister_notification_listener(&self) -> Result<()> {
        let listener_id = self.inner.listener_id.lock().unwrap().take();
        if let Some(id) = listener_id {
            // We do not need to unregister previously registered
            // notifications as we are unregistering the entire listener.

            // If we do want to unregister individual notifications we can do:
            // `self.client().rpc_api().stop_notify(listener_id, Scope:: ... ).await?;`
            // for each previously registered notification scope.

            self.client().rpc_api().unregister_listener(id).await?;
        }
        Ok(())
    }

    // generic connection handler fn called by the event task
    pub async fn handle_connect(&self) -> Result<()> {
        // make an RPC method call to the node...
        let server_info = self.client().get_server_info().await?;
        log_info!("Connected to Kaspa node - Server: {}", server_info.server_version);

        // now that we have successfully connected we
        // can register for notifications
        self.register_notification_listeners().await?;

        // store internal state indicating that we are currently connected
        self.inner.is_connected.store(true, Ordering::SeqCst);
        Ok(())
    }

    // generic disconnection handler fn called by the event task
    pub async fn handle_disconnect(&self) -> Result<()> {
        log_info!("Disconnected from {:?}", self.client().url());

        // Unregister notifications
        self.unregister_notification_listener().await?;

        // store internal state indicating that we are currently disconnected
        self.inner.is_connected.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn start_event_task(&self) -> Result<()> {
        // clone self for the async task
        let connection = self.clone();

        // clone the "rpc control channel" that posts notifications
        // when the RPC channel is connected or disconnected
        let rpc_ctl_channel = self.client().rpc_ctl().multiplexer().channel();

        // clone our sender and receiver channels for task control
        // these are obtained from the `DuplexChannel` - a pair of
        // channels where sender acts as a trigger signaling termination
        // and the receiver is used to signal termination completion.
        // (this is a common pattern used for channel lifetime management
        // in the rusty kaspa framework)
        let task_ctl_receiver = self.inner.task_ctl.request.receiver.clone();
        let task_ctl_sender = self.inner.task_ctl.response.sender.clone();

        // clone notification event channel that we provide to the RPC client
        // notification subsystem to receive notifications from the node.
        let notification_receiver = self.inner.notification_channel.receiver.clone();

        workflow_core::task::spawn(async move {
            loop {
                futures::select_biased! {
                    msg = rpc_ctl_channel.receiver.recv().fuse() => {
                        match msg {
                            Ok(msg) => {
                                // handle RPC channel connection and disconnection events
                                match msg {
                                    RpcState::Connected => {
                                        if let Err(err) = connection.handle_connect().await {
                                            log_error!("Error in connect handler: {err}");
                                        }
                                    },
                                    RpcState::Disconnected => {
                                        if let Err(err) = connection.handle_disconnect().await {
                                            log_error!("Error in disconnect handler: {err}");
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                // this will never occur if the RpcClient is owned and
                                // properly managed. This can only occur if RpcClient is
                                // deleted while this task is still running.
                                log_error!("RPC CTL channel error: {err}");
                                panic!("Unexpected: RPC CTL channel closed, halting...");
                            }
                        }
                    }
                    notification = notification_receiver.recv().fuse() => {
                        match notification {
                            Ok(notification) => {
                                if let Err(err) = connection.notification_handler.handle_notification(notification).await {
                                    log_error!("Error while handling notification: {err}");
                                }
                            }
                            Err(err) => {
                                panic!("RPC notification channel error: {err}");
                            }
                        }
                    },

                    // we use select_biased to drain rpc_ctl
                    // and notifications before shutting down
                    // as such task_ctl is last in the poll order
                    _ = task_ctl_receiver.recv().fuse() => {
                        break;
                    },
                }
            }

            log_info!("Event task exiting...");

            // handle our own power down on the rpc channel that remains connected
            if connection.is_connected() {
                connection
                    .handle_disconnect()
                    .await
                    .unwrap_or_else(|err| log_error!("{err}"));
            }

            // post task termination event
            task_ctl_sender.send(()).await.unwrap();
        });
        Ok(())
    }

    async fn stop_event_task(&self) -> Result<()> {
        self.inner
            .task_ctl
            .signal(())
            .await
            .expect("stop_event_task() signal error");
        Ok(())
    }
}