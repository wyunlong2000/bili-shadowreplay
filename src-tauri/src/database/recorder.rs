use super::Database;
use super::DatabaseError;
use crate::events::DataChange;
use chrono::Utc;
use recorder::platforms::PlatformType;
/// Recorder in database is pretty simple
/// because many room infos are collected in realtime
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct RecorderRow {
    pub room_id: String,
    pub created_at: String,
    pub platform: String,
    pub auto_start: bool,
    pub extra: String,
    pub web_rid: Option<String>,
    pub identifier_type: String,
    pub profile_name: String,
    pub profile_avatar: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct DetectionStateRow {
    pub platform: String,
    pub room_id: String,
    pub confirmed: Option<bool>,
    pub last_checked_at: i64,
    pub last_success_at: i64,
    pub error_kind: String,
    pub consecutive_unknown: i64,
    pub next_retry_at: i64,
    pub backoff_level: i64,
    pub updated_at: String,
}

// recorders
impl Database {
    pub async fn add_recorder(
        &self,
        platform: PlatformType,
        room_id: &str,
        extra: &str,
    ) -> Result<RecorderRow, DatabaseError> {
        self.add_recorder_with_identity(platform, room_id, extra, "room", None, "", "")
            .await
    }

    /// 按主播标识类型新增录制器配置。
    pub async fn add_recorder_with_identity(
        &self,
        platform: PlatformType,
        room_id: &str,
        extra: &str,
        identifier_type: &str,
        web_rid: Option<&str>,
        profile_name: &str,
        profile_avatar: &str,
    ) -> Result<RecorderRow, DatabaseError> {
        let recorder = RecorderRow {
            room_id: room_id.to_string(),
            created_at: Utc::now().to_rfc3339(),
            platform: platform.as_str().to_string(),
            auto_start: true,
            extra: extra.to_string(),
            web_rid: web_rid.map(str::to_string),
            identifier_type: identifier_type.to_string(),
            profile_name: profile_name.to_string(),
            profile_avatar: profile_avatar.to_string(),
        };
        let _ = sqlx::query(
            "INSERT OR REPLACE INTO recorders (room_id, created_at, platform, auto_start, extra, web_rid, identifier_type, profile_name, profile_avatar) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(room_id)
        .bind(&recorder.created_at)
        .bind(platform.as_str())
        .bind(recorder.auto_start)
        .bind(extra)
        .bind(&recorder.web_rid)
        .bind(&recorder.identifier_type)
        .bind(&recorder.profile_name)
        .bind(&recorder.profile_avatar)
        .execute(&self.pool)
        .await?;
        self.notify_change(DataChange::Recorders);
        Ok(recorder)
    }

    pub async fn remove_recorder(&self, room_id: &str) -> Result<RecorderRow, DatabaseError> {
        let recorder =
            sqlx::query_as::<_, RecorderRow>("SELECT * FROM recorders WHERE room_id = $1")
                .bind(room_id)
                .fetch_one(&self.pool)
                .await?;
        let sql = sqlx::query("DELETE FROM recorders WHERE room_id = $1")
            .bind(room_id)
            .execute(&self.pool)
            .await?;
        if sql.rows_affected() != 1 {
            return Err(DatabaseError::NotFound);
        }

        // remove related archive
        let _ = self.remove_archive(room_id).await;
        self.notify_change(DataChange::Recorders);
        Ok(recorder)
    }

    pub async fn get_recorders(&self) -> Result<Vec<RecorderRow>, DatabaseError> {
        Ok(sqlx::query_as::<_, RecorderRow>(
            "SELECT room_id, created_at, platform, auto_start, extra, web_rid, identifier_type, profile_name, profile_avatar FROM recorders",
        )
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn remove_archive(&self, room_id: &str) -> Result<(), DatabaseError> {
        let _ = sqlx::query("DELETE FROM records WHERE room_id = $1")
            .bind(room_id)
            .execute(&self.pool)
            .await?;
        self.notify_change(DataChange::Records);
        Ok(())
    }

    pub async fn update_recorder(
        &self,
        platform: PlatformType,
        room_id: &str,
        auto_start: bool,
    ) -> Result<(), DatabaseError> {
        let _ = sqlx::query(
            "UPDATE recorders SET auto_start = $1 WHERE platform = $2 AND room_id = $3",
        )
        .bind(auto_start)
        .bind(platform.as_str().to_string())
        .bind(room_id)
        .execute(&self.pool)
        .await?;
        self.notify_change(DataChange::Recorders);
        Ok(())
    }

    /// 更新主页标识对应的抖音直播间号。
    pub async fn update_recorder_web_rid(
        &self,
        platform: PlatformType,
        room_id: &str,
        web_rid: &str,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            "UPDATE recorders SET web_rid = $1 WHERE platform = $2 AND room_id = $3 AND identifier_type = 'profile'",
        )
        .bind(web_rid)
        .bind(platform.as_str())
        .bind(room_id)
        .execute(&self.pool)
        .await?;
        self.notify_change(DataChange::Recorders);
        Ok(())
    }

    /// 查询主播最近一次持久化的监测状态。
    pub async fn get_detection_state(
        &self,
        platform: PlatformType,
        room_id: &str,
    ) -> Result<Option<DetectionStateRow>, DatabaseError> {
        Ok(sqlx::query_as::<_, DetectionStateRow>(
            "SELECT platform, room_id, confirmed, last_checked_at, last_success_at, error_kind, consecutive_unknown, next_retry_at, backoff_level, updated_at FROM detection_states WHERE platform = $1 AND room_id = $2",
        )
        .bind(platform.as_str())
        .bind(room_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// 新增或更新主播的持久化监测状态。
    pub async fn upsert_detection_state(
        &self,
        state: &DetectionStateRow,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            "INSERT INTO detection_states (platform, room_id, confirmed, last_checked_at, last_success_at, error_kind, consecutive_unknown, next_retry_at, backoff_level, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) ON CONFLICT(platform, room_id) DO UPDATE SET confirmed = excluded.confirmed, last_checked_at = excluded.last_checked_at, last_success_at = excluded.last_success_at, error_kind = excluded.error_kind, consecutive_unknown = excluded.consecutive_unknown, next_retry_at = excluded.next_retry_at, backoff_level = excluded.backoff_level, updated_at = excluded.updated_at",
        )
        .bind(&state.platform)
        .bind(&state.room_id)
        .bind(state.confirmed)
        .bind(state.last_checked_at)
        .bind(state.last_success_at)
        .bind(&state.error_kind)
        .bind(state.consecutive_unknown)
        .bind(state.next_retry_at)
        .bind(state.backoff_level)
        .bind(&state.updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
