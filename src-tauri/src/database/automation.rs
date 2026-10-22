use chrono::Utc;
use sqlx::Sqlite;

use super::{Database, DatabaseError};
use crate::automation::types::{
    AutomationAnalysisRow, AutomationCandidateRow, AutomationOutputRow, AutomationRoomPolicyRow,
    AutomationRunRow,
};
use crate::database::record::RecordRow;

impl Database {
    /// 结合平台与场次标识精确查询同场录播记录（规避既有方法未绑定 platform 的缺陷）。
    pub async fn get_archives_by_parent_id_and_platform(
        &self,
        platform: &str,
        room_id: &str,
        parent_id: &str,
    ) -> Result<Vec<RecordRow>, DatabaseError> {
        let rows = sqlx::query_as::<Sqlite, RecordRow>(
            r#"
            SELECT live_id, platform, room_id, title, length, size, cover, created_at
            FROM records
            WHERE platform = $1 AND room_id = $2 AND (live_id = $3 OR parent_id = $3)
            ORDER BY created_at ASC
            "#,
        )
        .bind(platform)
        .bind(room_id)
        .bind(parent_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// 查询指定房间的自动化覆盖策略。
    pub async fn get_room_automation_policy(
        &self,
        platform: &str,
        room_id: &str,
    ) -> Result<Option<AutomationRoomPolicyRow>, DatabaseError> {
        let row = sqlx::query_as::<Sqlite, AutomationRoomPolicyRow>(
            r#"
            SELECT platform, room_id, policy_json, revision, created_at, updated_at
            FROM automation_room_policies
            WHERE platform = $1 AND room_id = $2
            "#,
        )
        .bind(platform)
        .bind(room_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// 保存或更新房间的自动化覆盖策略（具备乐观锁修订号机制）。
    pub async fn save_room_automation_policy(
        &self,
        platform: &str,
        room_id: &str,
        policy_json: &str,
        expected_revision: Option<i64>,
    ) -> Result<i64, DatabaseError> {
        let now = Utc::now().to_rfc3339();
        let current = self.get_room_automation_policy(platform, room_id).await?;

        match (current, expected_revision) {
            (None, _) => {
                sqlx::query(
                    r#"
                    INSERT INTO automation_room_policies (platform, room_id, policy_json, revision, created_at, updated_at)
                    VALUES ($1, $2, $3, 1, $4, $4)
                    "#,
                )
                .bind(platform)
                .bind(room_id)
                .bind(policy_json)
                .bind(&now)
                .execute(&self.pool)
                .await?;
                Ok(1)
            }
            (Some(row), Some(exp)) if row.revision != exp => Err(DatabaseError::Sql {
                sql: format!(
                    "策略版本冲突：当前修订号为 {}，预期为 {}",
                    row.revision, exp
                ),
            }),
            (Some(row), _) => {
                let next_rev = row.revision + 1;
                sqlx::query(
                    r#"
                    UPDATE automation_room_policies
                    SET policy_json = $1, revision = $2, updated_at = $3
                    WHERE platform = $4 AND room_id = $5
                    "#,
                )
                .bind(policy_json)
                .bind(next_rev)
                .bind(&now)
                .bind(platform)
                .bind(room_id)
                .execute(&self.pool)
                .await?;
                Ok(next_rev)
            }
        }
    }

    /// 创建新的自动化场次运行记录。
    pub async fn create_automation_run(&self, run: &AutomationRunRow) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            INSERT INTO automation_runs (
                id, platform, room_id, parent_id, live_id,
                range_start_ms, range_end_ms, request_key, input_hash,
                config_snapshot_json, policy_fingerprint, status, stage,
                task_id, lease_owner, lease_expires_at, source_manifest_json,
                completed_windows, total_windows, error_code, error_message,
                created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, $8, $9,
                $10, $11, $12, $13,
                $14, $15, $16, $17,
                $18, $19, $20, $21,
                $22, $23
            )
            "#,
        )
        .bind(&run.id)
        .bind(&run.platform)
        .bind(&run.room_id)
        .bind(&run.parent_id)
        .bind(&run.live_id)
        .bind(run.range_start_ms)
        .bind(run.range_end_ms)
        .bind(&run.request_key)
        .bind(&run.input_hash)
        .bind(&run.config_snapshot_json)
        .bind(&run.policy_fingerprint)
        .bind(&run.status)
        .bind(&run.stage)
        .bind(&run.task_id)
        .bind(&run.lease_owner)
        .bind(&run.lease_expires_at)
        .bind(&run.source_manifest_json)
        .bind(run.completed_windows)
        .bind(run.total_windows)
        .bind(&run.error_code)
        .bind(&run.error_message)
        .bind(&run.created_at)
        .bind(&run.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 根据 ID 查询场次运行记录。
    pub async fn get_automation_run(
        &self,
        id: &str,
    ) -> Result<Option<AutomationRunRow>, DatabaseError> {
        let row = sqlx::query_as::<Sqlite, AutomationRunRow>(
            r#"
            SELECT * FROM automation_runs WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// 根据唯一请求键查询运行记录（用于防重与幂等）。
    pub async fn get_automation_run_by_request_key(
        &self,
        request_key: &str,
    ) -> Result<Option<AutomationRunRow>, DatabaseError> {
        let row = sqlx::query_as::<Sqlite, AutomationRunRow>(
            r#"
            SELECT * FROM automation_runs WHERE request_key = $1
            "#,
        )
        .bind(request_key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// 获取某场次当前正在运行中的任务（非终态）。
    pub async fn get_active_automation_run(
        &self,
        platform: &str,
        room_id: &str,
        parent_id: &str,
    ) -> Result<Option<AutomationRunRow>, DatabaseError> {
        let row = sqlx::query_as::<Sqlite, AutomationRunRow>(
            r#"
            SELECT * FROM automation_runs
            WHERE platform = $1 AND room_id = $2 AND parent_id = $3
              AND status IN ('pending', 'processing', 'waiting')
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(platform)
        .bind(room_id)
        .bind(parent_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// 尝试认领自动化运行任务租约，成功返回 true。
    pub async fn claim_automation_run(
        &self,
        run_id: &str,
        lease_owner: &str,
        lease_duration_secs: i64,
    ) -> Result<bool, DatabaseError> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let expires_at_str = (now + chrono::Duration::seconds(lease_duration_secs)).to_rfc3339();

        let res = sqlx::query(
            r#"
            UPDATE automation_runs
            SET lease_owner = $1, lease_expires_at = $2, status = 'processing', updated_at = $3
            WHERE id = $4
              AND (lease_expires_at IS NULL OR lease_expires_at < $3 OR lease_owner = $1)
            "#,
        )
        .bind(lease_owner)
        .bind(&expires_at_str)
        .bind(&now_str)
        .bind(run_id)
        .execute(&self.pool)
        .await?;

        Ok(res.rows_affected() > 0)
    }

    /// 续约正在执行的任务租约。
    pub async fn renew_automation_run_lease(
        &self,
        run_id: &str,
        lease_owner: &str,
        lease_duration_secs: i64,
    ) -> Result<bool, DatabaseError> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let expires_at_str = (now + chrono::Duration::seconds(lease_duration_secs)).to_rfc3339();

        let res = sqlx::query(
            r#"
            UPDATE automation_runs
            SET lease_expires_at = $1, updated_at = $2
            WHERE id = $3 AND lease_owner = $4
            "#,
        )
        .bind(&expires_at_str)
        .bind(&now_str)
        .bind(run_id)
        .bind(lease_owner)
        .execute(&self.pool)
        .await?;

        Ok(res.rows_affected() > 0)
    }

    /// 更新自动化场次运行状态与错误信息。
    pub async fn update_automation_run_status(
        &self,
        run_id: &str,
        status: &str,
        stage: &str,
        error_code: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), DatabaseError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE automation_runs
            SET status = $1, stage = $2, error_code = $3, error_message = $4, updated_at = $5
            WHERE id = $6
            "#,
        )
        .bind(status)
        .bind(stage)
        .bind(error_code)
        .bind(error_message)
        .bind(&now)
        .bind(run_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 更新自动化分析进度（窗口数）。
    pub async fn update_automation_run_progress(
        &self,
        run_id: &str,
        completed_windows: i64,
        total_windows: i64,
    ) -> Result<(), DatabaseError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE automation_runs
            SET completed_windows = $1, total_windows = $2, updated_at = $3
            WHERE id = $4
            "#,
        )
        .bind(completed_windows)
        .bind(total_windows)
        .bind(&now)
        .bind(run_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 保存分段分析检查点记录。
    pub async fn save_automation_analysis(
        &self,
        analysis: &AutomationAnalysisRow,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            INSERT INTO automation_analysis (
                id, run_id, platform, room_id, live_id, parent_id,
                status, stage, processed_until_ms, window_seconds,
                subtitle_json, checkpoints_json, input_hash, task_id,
                error_message, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6,
                $7, $8, $9, $10,
                $11, $12, $13, $14,
                $15, $16, $17
            )
            ON CONFLICT(run_id, live_id) DO UPDATE SET
                status = excluded.status,
                stage = excluded.stage,
                processed_until_ms = excluded.processed_until_ms,
                subtitle_json = excluded.subtitle_json,
                checkpoints_json = excluded.checkpoints_json,
                error_message = excluded.error_message,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&analysis.id)
        .bind(&analysis.run_id)
        .bind(&analysis.platform)
        .bind(&analysis.room_id)
        .bind(&analysis.live_id)
        .bind(&analysis.parent_id)
        .bind(&analysis.status)
        .bind(&analysis.stage)
        .bind(analysis.processed_until_ms)
        .bind(analysis.window_seconds)
        .bind(&analysis.subtitle_json)
        .bind(&analysis.checkpoints_json)
        .bind(&analysis.input_hash)
        .bind(&analysis.task_id)
        .bind(&analysis.error_message)
        .bind(&analysis.created_at)
        .bind(&analysis.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 批量写入自动化候选片段。
    pub async fn create_automation_candidates(
        &self,
        candidates: &[AutomationCandidateRow],
    ) -> Result<(), DatabaseError> {
        let mut tx = self.pool.begin().await?;
        for c in candidates {
            sqlx::query(
                r#"
                INSERT INTO automation_candidates (
                    id, run_id, platform, room_id, parent_id, live_id,
                    start_ms, end_ms, score, signals_json, reason, status,
                    content_type, confidence, evidence_json, manual_modified,
                    revision, clip_task_id, video_id, notify_status,
                    error_message, created_at, updated_at
                ) VALUES (
                    $1, $2, $3, $4, $5, $6,
                    $7, $8, $9, $10, $11, $12,
                    $13, $14, $15, $16,
                    $17, $18, $19, $20,
                    $21, $22, $23
                )
                ON CONFLICT(run_id, live_id, start_ms, end_ms) DO UPDATE SET
                    score = excluded.score,
                    signals_json = excluded.signals_json,
                    reason = excluded.reason,
                    content_type = excluded.content_type,
                    confidence = excluded.confidence,
                    evidence_json = excluded.evidence_json,
                    updated_at = excluded.updated_at
                "#,
            )
            .bind(&c.id)
            .bind(&c.run_id)
            .bind(&c.platform)
            .bind(&c.room_id)
            .bind(&c.parent_id)
            .bind(&c.live_id)
            .bind(c.start_ms)
            .bind(c.end_ms)
            .bind(c.score)
            .bind(&c.signals_json)
            .bind(&c.reason)
            .bind(&c.status)
            .bind(&c.content_type)
            .bind(c.confidence)
            .bind(&c.evidence_json)
            .bind(c.manual_modified)
            .bind(c.revision)
            .bind(&c.clip_task_id)
            .bind(c.video_id)
            .bind(&c.notify_status)
            .bind(&c.error_message)
            .bind(&c.created_at)
            .bind(&c.updated_at)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;

        Ok(())
    }

    /// 查询某次运行下的所有候选片段。
    pub async fn get_candidates_by_run(
        &self,
        run_id: &str,
    ) -> Result<Vec<AutomationCandidateRow>, DatabaseError> {
        let rows = sqlx::query_as::<Sqlite, AutomationCandidateRow>(
            r#"
            SELECT * FROM automation_candidates
            WHERE run_id = $1
            ORDER BY start_ms ASC
            "#,
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// 用户手动修改候选边界与类型（带乐观锁修订号机制）。
    pub async fn update_candidate_manual(
        &self,
        candidate_id: &str,
        expected_revision: i64,
        start_ms: i64,
        end_ms: i64,
        content_type: &str,
    ) -> Result<bool, DatabaseError> {
        let now = Utc::now().to_rfc3339();
        let res = sqlx::query(
            r#"
            UPDATE automation_candidates
            SET start_ms = $1, end_ms = $2, content_type = $3,
                manual_modified = 1, revision = revision + 1, updated_at = $4
            WHERE id = $5 AND revision = $6
            "#,
        )
        .bind(start_ms)
        .bind(end_ms)
        .bind(content_type)
        .bind(&now)
        .bind(candidate_id)
        .bind(expected_revision)
        .execute(&self.pool)
        .await?;

        Ok(res.rows_affected() > 0)
    }

    /// 保存或更新单版本成片输出记录。
    pub async fn save_automation_output(
        &self,
        output: &AutomationOutputRow,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            INSERT INTO automation_outputs (
                id, candidate_id, output_mode, plan_version,
                plan_json, plan_hash, project_id, material_id,
                assets_json, render_task_id, status, error_code,
                error_message, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4,
                $5, $6, $7, $8,
                $9, $10, $11, $12,
                $13, $14, $15
            )
            ON CONFLICT(candidate_id, output_mode, plan_version) DO UPDATE SET
                plan_json = excluded.plan_json,
                plan_hash = excluded.plan_hash,
                project_id = excluded.project_id,
                material_id = excluded.material_id,
                assets_json = excluded.assets_json,
                render_task_id = excluded.render_task_id,
                status = excluded.status,
                error_code = excluded.error_code,
                error_message = excluded.error_message,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&output.id)
        .bind(&output.candidate_id)
        .bind(&output.output_mode)
        .bind(output.plan_version)
        .bind(&output.plan_json)
        .bind(&output.plan_hash)
        .bind(output.project_id)
        .bind(output.material_id)
        .bind(&output.assets_json)
        .bind(&output.render_task_id)
        .bind(&output.status)
        .bind(&output.error_code)
        .bind(&output.error_message)
        .bind(&output.created_at)
        .bind(&output.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 查询候选对应的所有成片版本记录。
    pub async fn get_outputs_by_candidate(
        &self,
        candidate_id: &str,
    ) -> Result<Vec<AutomationOutputRow>, DatabaseError> {
        let rows = sqlx::query_as::<Sqlite, AutomationOutputRow>(
            r#"
            SELECT * FROM automation_outputs
            WHERE candidate_id = $1
            ORDER BY plan_version DESC
            "#,
        )
        .bind(candidate_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// 根据 ID 查询单个输出结果。
    pub async fn get_automation_output(
        &self,
        id: &str,
    ) -> Result<Option<AutomationOutputRow>, DatabaseError> {
        let row = sqlx::query_as::<Sqlite, AutomationOutputRow>(
            r#"
            SELECT * FROM automation_outputs WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }
}
