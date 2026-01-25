//! Scheduled cleanup tasks for expired/orphaned data.

use crate::db::Database;
use std::time::Duration;
use tracing::{error, info};

/// Age threshold for orphaned attachments (in minutes).
/// Attachments with reference_count = 0 older than this will be deleted.
const ORPHANED_ATTACHMENT_AGE_MINUTES: i64 = 60;

/// Interval between cleanup runs.
const CLEANUP_INTERVAL: Duration = Duration::from_secs(60 * 60); // 1 hour

/// Run all cleanup tasks once.
pub async fn run_cleanup(db: &Database) {
    // Clean up expired tokens
    match db.tokens().delete_expired().await {
        Ok(count) if count > 0 => info!("Cleaned up {} expired tokens", count),
        Ok(_) => {}
        Err(e) => error!("Failed to clean up expired tokens: {}", e),
    }

    // Clean up expired registration challenges
    match db.challenges().cleanup_expired().await {
        Ok(count) if count > 0 => info!("Cleaned up {} expired registration challenges", count),
        Ok(_) => {}
        Err(e) => error!("Failed to clean up registration challenges: {}", e),
    }

    // Clean up expired login challenges
    match db.login_challenges().cleanup_expired().await {
        Ok(count) if count > 0 => info!("Cleaned up {} expired login challenges", count),
        Ok(_) => {}
        Err(e) => error!("Failed to clean up login challenges: {}", e),
    }

    // Clean up pending (unactivated) users
    match db.users().cleanup_pending().await {
        Ok(count) if count > 0 => info!("Cleaned up {} pending users", count),
        Ok(_) => {}
        Err(e) => error!("Failed to clean up pending users: {}", e),
    }

    // Clean up orphaned attachments
    match db
        .attachments()
        .cleanup_orphaned(ORPHANED_ATTACHMENT_AGE_MINUTES)
        .await
    {
        Ok(count) if count > 0 => info!("Cleaned up {} orphaned attachments", count),
        Ok(_) => {}
        Err(e) => error!("Failed to clean up orphaned attachments: {}", e),
    }
}

/// Spawn a background task that runs cleanup periodically.
/// Returns a handle that can be used to abort the task.
pub fn spawn_cleanup_scheduler(db: Database) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(CLEANUP_INTERVAL);

        loop {
            interval.tick().await;
            run_cleanup(&db).await;
        }
    })
}
