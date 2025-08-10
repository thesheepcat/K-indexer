use polodb_core::{Collection, Database};
use crate::models::{KPostRecord, KReplyRecord, KBroadcastRecord, KVoteRecord};

pub struct DatabaseManager {
    pub db: Database,
}

impl DatabaseManager {
    pub fn new(db_path: &str) -> Result<Self, polodb_core::Error> {
        let db = Database::open_path(db_path)?;
        Ok(Self { db })
    }

    

    pub fn get_k_posts_collection(&self) -> Collection<KPostRecord> {
        self.db.collection("k-posts")
    }

    pub fn get_k_replies_collection(&self) -> Collection<KReplyRecord> {
        self.db.collection("k-replies")
    }

    pub fn get_k_broadcasts_collection(&self) -> Collection<KBroadcastRecord> {
        self.db.collection("k-broadcasts")
    }

    pub fn get_k_votes_collection(&self) -> Collection<KVoteRecord> {
        self.db.collection("k-votes")
    }

    pub fn get_database(&self) -> &Database {
        &self.db
    }
}