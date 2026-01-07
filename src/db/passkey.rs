use sqlx::sqlite::SqlitePool;
use webauthn_rs::prelude::Passkey;

#[derive(Clone)]
pub struct PasskeyStore {
    pool: SqlitePool,
}

pub struct StoredPasskey {
    pub id: i64,
    pub user_id: i64,
    pub passkey: Passkey,
}

#[derive(sqlx::FromRow)]
struct PasskeyRow {
    id: i64,
    user_id: i64,
    passkey_json: String,
}

impl PasskeyStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Add a passkey for a user. Returns the passkey ID.
    pub async fn add(&self, user_id: i64, passkey: &Passkey) -> Result<i64, sqlx::Error> {
        let credential_id = base64_encode(passkey.cred_id().as_ref());
        let json = serde_json::to_string(passkey).map_err(|e| sqlx::Error::Encode(Box::new(e)))?;

        let result = sqlx::query(
            "INSERT INTO passkeys (credential_id, user_id, passkey_json) VALUES (?, ?, ?)",
        )
        .bind(&credential_id)
        .bind(user_id)
        .bind(&json)
        .execute(&self.pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    /// Get a passkey by ID.
    pub async fn get_by_id(&self, id: i64) -> Result<Option<StoredPasskey>, sqlx::Error> {
        let row: Option<PasskeyRow> =
            sqlx::query_as("SELECT id, user_id, passkey_json FROM passkeys WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;

        match row {
            Some(row) => {
                let passkey: Passkey = serde_json::from_str(&row.passkey_json)
                    .map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
                Ok(Some(StoredPasskey {
                    id: row.id,
                    user_id: row.user_id,
                    passkey,
                }))
            }
            None => Ok(None),
        }
    }

    /// Get the user_id for a passkey by ID (without deserializing the passkey).
    pub async fn get_user_id_by_passkey_id(&self, id: i64) -> Result<Option<i64>, sqlx::Error> {
        let row: Option<(i64,)> = sqlx::query_as("SELECT user_id FROM passkeys WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|(user_id,)| user_id))
    }

    /// Get a passkey by credential ID.
    pub async fn get_by_credential_id(
        &self,
        credential_id: &[u8],
    ) -> Result<Option<StoredPasskey>, sqlx::Error> {
        let credential_id_b64 = base64_encode(credential_id);

        let row: Option<PasskeyRow> = sqlx::query_as(
            "SELECT id, user_id, passkey_json FROM passkeys WHERE credential_id = ?",
        )
        .bind(&credential_id_b64)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let passkey: Passkey = serde_json::from_str(&row.passkey_json)
                    .map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
                Ok(Some(StoredPasskey {
                    id: row.id,
                    user_id: row.user_id,
                    passkey,
                }))
            }
            None => Ok(None),
        }
    }

    /// Get all passkeys for a user.
    pub async fn get_by_user_id(&self, user_id: i64) -> Result<Vec<StoredPasskey>, sqlx::Error> {
        let rows: Vec<PasskeyRow> =
            sqlx::query_as("SELECT id, user_id, passkey_json FROM passkeys WHERE user_id = ?")
                .bind(user_id)
                .fetch_all(&self.pool)
                .await?;

        let mut passkeys = Vec::with_capacity(rows.len());
        for row in rows {
            let passkey: Passkey = serde_json::from_str(&row.passkey_json)
                .map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
            passkeys.push(StoredPasskey {
                id: row.id,
                user_id: row.user_id,
                passkey,
            });
        }
        Ok(passkeys)
    }

    /// Update a passkey (e.g., after authentication to update the counter).
    pub async fn update(&self, passkey: &Passkey) -> Result<bool, sqlx::Error> {
        let credential_id = base64_encode(passkey.cred_id().as_ref());
        let json = serde_json::to_string(passkey).map_err(|e| sqlx::Error::Encode(Box::new(e)))?;

        let result = sqlx::query("UPDATE passkeys SET passkey_json = ? WHERE credential_id = ?")
            .bind(&json)
            .bind(&credential_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Delete a passkey by credential ID.
    pub async fn delete(&self, credential_id: &[u8]) -> Result<bool, sqlx::Error> {
        let credential_id_b64 = base64_encode(credential_id);
        let result = sqlx::query("DELETE FROM passkeys WHERE credential_id = ?")
            .bind(&credential_id_b64)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

fn base64_encode(data: &[u8]) -> String {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    URL_SAFE_NO_PAD.encode(data)
}
