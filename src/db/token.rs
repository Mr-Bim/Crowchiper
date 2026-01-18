//! Active token storage for refresh token tracking and revocation.
//!
//! Only refresh tokens are stored in the database for revocation support.
//! Access tokens are stateless and short-lived (5 minutes).

use sqlx::sqlite::SqlitePool;

/// An active refresh token record.
#[derive(Debug, Clone)]
pub struct ActiveToken {
    pub id: i64,
    pub jti: String,
    pub user_id: i64,
    pub last_ip: Option<String>,
    pub issued_at: String,
    pub expires_at: String,
    pub created_at: String,
    pub token_type: String,
}

/// Store for managing active refresh tokens.
pub struct TokenStore {
    pool: SqlitePool,
}

impl TokenStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a new refresh token record.
    pub async fn create(
        &self,
        jti: &str,
        user_id: i64,
        ip: Option<&str>,
        issued_at: u64,
        expires_at: u64,
    ) -> Result<i64, sqlx::Error> {
        let issued_at_str = timestamp_to_datetime(issued_at);
        let expires_at_str = timestamp_to_datetime(expires_at);

        let result = sqlx::query(
            "INSERT INTO active_tokens (jti, user_id, last_ip, issued_at, expires_at, token_type) VALUES (?, ?, ?, ?, ?, 'refresh')",
        )
        .bind(jti)
        .bind(user_id)
        .bind(ip)
        .bind(&issued_at_str)
        .bind(&expires_at_str)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Get an active token by its JWT ID.
    pub async fn get_by_jti(&self, jti: &str) -> Result<Option<ActiveToken>, sqlx::Error> {
        let row: Option<(i64, String, i64, Option<String>, String, String, String, String)> =
            sqlx::query_as(
                "SELECT id, jti, user_id, last_ip, issued_at, expires_at, created_at, token_type FROM active_tokens WHERE jti = ?",
            )
            .bind(jti)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(
            |(id, jti, user_id, last_ip, issued_at, expires_at, created_at, token_type)| {
                ActiveToken {
                    id,
                    jti,
                    user_id,
                    last_ip,
                    issued_at,
                    expires_at,
                    created_at,
                    token_type,
                }
            },
        ))
    }

    /// Update the last IP address for a token.
    pub async fn update_ip(&self, jti: &str, ip: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE active_tokens SET last_ip = ? WHERE jti = ?")
            .bind(ip)
            .bind(jti)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Delete a token by its JWT ID (revoke).
    pub async fn delete_by_jti(&self, jti: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM active_tokens WHERE jti = ?")
            .bind(jti)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Delete all expired tokens.
    pub async fn delete_expired(&self) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM active_tokens WHERE expires_at < datetime('now')")
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// List all active refresh tokens for a user.
    pub async fn list_by_user(&self, user_id: i64) -> Result<Vec<ActiveToken>, sqlx::Error> {
        let rows: Vec<(i64, String, i64, Option<String>, String, String, String, String)> = sqlx::query_as(
            "SELECT id, jti, user_id, last_ip, issued_at, expires_at, created_at, token_type FROM active_tokens WHERE user_id = ? AND expires_at >= datetime('now') ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(id, jti, user_id, last_ip, issued_at, expires_at, created_at, token_type)| {
                    ActiveToken {
                        id,
                        jti,
                        user_id,
                        last_ip,
                        issued_at,
                        expires_at,
                        created_at,
                        token_type,
                    }
                },
            )
            .collect())
    }

    /// Delete all tokens for a user (logout everywhere).
    pub async fn delete_all_by_user(&self, user_id: i64) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM active_tokens WHERE user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}

/// Convert a Unix timestamp to an ISO 8601 datetime string for SQLite.
fn timestamp_to_datetime(timestamp: u64) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    let datetime = UNIX_EPOCH + Duration::from_secs(timestamp);
    let secs = datetime
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Convert to ISO 8601 format: YYYY-MM-DD HH:MM:SS
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Calculate year, month, day from days since epoch (1970-01-01)
    let (year, month, day) = days_to_ymd(days_since_epoch as i64);

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        year, month, day, hours, minutes, seconds
    )
}

/// Convert days since Unix epoch to year, month, day.
fn days_to_ymd(days: i64) -> (i32, u32, u32) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_to_datetime() {
        // 2024-01-15 12:30:45 UTC
        let ts = 1705321845;
        let dt = timestamp_to_datetime(ts);
        assert_eq!(dt, "2024-01-15 12:30:45");
    }

    #[test]
    fn test_epoch() {
        let dt = timestamp_to_datetime(0);
        assert_eq!(dt, "1970-01-01 00:00:00");
    }
}
