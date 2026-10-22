use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use chrono::Utc;
use tokio::sync::{Mutex, Notify, RwLock};

use super::policy::AutoClippingPolicy;
use super::source::SourceManifest;
use super::types::{AutomationRunRow, AutomationRunStatus, AutomationStage};
use crate::config::Config;
use crate::database::Database;
use crate::events::{DataChange, EventBus};
use crate::task::TaskManager;

/// 自动化切片调度服务，负责场次监听、任务认领、检查点恢复与流水线调度。
pub struct AutomationService {
    db: Arc<Database>,
    config: Arc<RwLock<Config>>,
    task_manager: Arc<TaskManager>,
    events: EventBus,
    cache_dir: PathBuf,
    output_dir: PathBuf,
    notify: Arc<Notify>,
    stopped: Arc<AtomicBool>,
    worker_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl AutomationService {
    /// 构造新的自动化切片调度服务实例。
    pub fn new(
        db: Arc<Database>,
        config: Arc<RwLock<Config>>,
        task_manager: Arc<TaskManager>,
        events: EventBus,
        cache_dir: PathBuf,
        output_dir: PathBuf,
        notify: Arc<Notify>,
    ) -> Self {
        Self {
            db,
            config,
            task_manager,
            events,
            cache_dir,
            output_dir,
            notify,
            stopped: Arc::new(AtomicBool::new(false)),
            worker_handle: Mutex::new(None),
        }
    }

    /// 获取共享唤醒通知器。
    pub fn notifier(&self) -> Arc<Notify> {
        self.notify.clone()
    }

    /// 启动后台调度轮询与唤醒循环。
    pub fn start(self: &Arc<Self>) {
        let this = self.clone();
        let handle = tokio::spawn(async move {
            this.run_loop().await;
        });
        let this2 = self.clone();
        tokio::spawn(async move {
            let mut lock = this2.worker_handle.lock().await;
            *lock = Some(handle);
        });
    }

    /// 停止后台调度循环。
    pub async fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
        let handle = self.worker_handle.lock().await.take();
        if let Some(h) = handle {
            let _ = h.await;
        }
    }

    /// 应用启动时恢复异常中断的场次任务。
    pub async fn recover_interrupted_runs(&self) -> Result<(), String> {
        let count = self
            .db
            .recover_interrupted_runs()
            .await
            .map_err(|e| format!("恢复中断的自动化任务失败: {e}"))?;

        if count > 0 {
            log::info!("已重置 {} 个异常中断的自动化任务为 interrupted 状态", count);
            self.db.notify_change(DataChange::Automation);
        }

        Ok(())
    }

    /// 下播事件发生时独立登记自动化待处理记录（不阻断既有业务）。
    pub async fn enqueue_ended_live(
        &self,
        platform: &str,
        room_id: &str,
        parent_id: &str,
        live_id: &str,
    ) -> Result<Option<String>, String> {
        let global_config = self.config.read().await.auto_clipping.clone();
        let room_policy_row = self
            .db
            .get_room_automation_policy(platform, room_id)
            .await
            .unwrap_or(None);

        let room_override = room_policy_row.and_then(|r| serde_json::from_str(&r.policy_json).ok());
        let policy = AutoClippingPolicy {
            enabled: global_config.enabled,
            content_type: global_config.content_type,
            auto_classify: global_config.auto_classify,
            output_mode: global_config.output_mode,
            window_seconds: global_config.window_seconds,
            overlap_seconds: global_config.overlap_seconds,
            max_candidates: global_config.max_candidates,
            min_score: global_config.min_score,
        }
        .merge_room_override(room_override.as_ref());

        if !policy.enabled {
            log::debug!("房间自动切片未启用: {platform}:{room_id}");
            return Ok(None);
        }

        let request_key = format!("{}:{}:{}", platform, room_id, parent_id);
        if let Ok(Some(existing)) = self.db.get_automation_run_by_request_key(&request_key).await {
            log::info!("场次自动化任务已存在，无需重复登记: {request_key} (状态: {})", existing.status);
            return Ok(Some(existing.id));
        }

        let run_id = format!("run_{}", uuid::Uuid::new_v4().simple());
        let now = Utc::now().to_rfc3339();
        let config_snapshot_json = serde_json::to_string(&policy).unwrap_or_default();
        let policy_fingerprint = policy.fingerprint();

        let run = AutomationRunRow {
            id: run_id.clone(),
            platform: platform.to_string(),
            room_id: room_id.to_string(),
            parent_id: parent_id.to_string(),
            live_id: live_id.to_string(),
            range_start_ms: 0,
            range_end_ms: 0,
            request_key,
            input_hash: String::new(),
            config_snapshot_json,
            policy_fingerprint,
            status: "pending".to_string(),
            stage: "indexing".to_string(),
            task_id: None,
            lease_owner: None,
            lease_expires_at: None,
            source_manifest_json: "[]".to_string(),
            completed_windows: 0,
            total_windows: 0,
            error_code: None,
            error_message: None,
            created_at: now.clone(),
            updated_at: now,
        };

        self.db
            .create_automation_run(&run)
            .await
            .map_err(|e| format!("登记自动化场次任务失败: {e}"))?;

        log::info!("已成功登记场次自动化任务: {} (run_id: {})", run.request_key, run_id);
        self.db.notify_change(DataChange::Automation);
        self.notify.notify_waiters();

        Ok(Some(run_id))
    }

    /// 手动对指定场次发起自动化切片分析。
    pub async fn enqueue_manual_run(
        &self,
        platform: &str,
        room_id: &str,
        parent_id: &str,
        live_id: &str,
    ) -> Result<String, String> {
        let global_config = self.config.read().await.auto_clipping.clone();
        let room_policy_row = self
            .db
            .get_room_automation_policy(platform, room_id)
            .await
            .unwrap_or(None);

        let room_override = room_policy_row.and_then(|r| serde_json::from_str(&r.policy_json).ok());
        let mut policy = AutoClippingPolicy {
            enabled: true, // 手动触发强制开启
            content_type: global_config.content_type,
            auto_classify: global_config.auto_classify,
            output_mode: global_config.output_mode,
            window_seconds: global_config.window_seconds,
            overlap_seconds: global_config.overlap_seconds,
            max_candidates: global_config.max_candidates,
            min_score: global_config.min_score,
        }
        .merge_room_override(room_override.as_ref());
        policy.enabled = true;

        let request_key = format!(
            "{}:{}:{}:manual_{}",
            platform,
            room_id,
            parent_id,
            Utc::now().timestamp_millis()
        );

        let run_id = format!("run_{}", uuid::Uuid::new_v4().simple());
        let now = Utc::now().to_rfc3339();
        let config_snapshot_json = serde_json::to_string(&policy).unwrap_or_default();
        let policy_fingerprint = policy.fingerprint();

        let run = AutomationRunRow {
            id: run_id.clone(),
            platform: platform.to_string(),
            room_id: room_id.to_string(),
            parent_id: parent_id.to_string(),
            live_id: live_id.to_string(),
            range_start_ms: 0,
            range_end_ms: 0,
            request_key,
            input_hash: String::new(),
            config_snapshot_json,
            policy_fingerprint,
            status: "pending".to_string(),
            stage: "indexing".to_string(),
            task_id: None,
            lease_owner: None,
            lease_expires_at: None,
            source_manifest_json: "[]".to_string(),
            completed_windows: 0,
            total_windows: 0,
            error_code: None,
            error_message: None,
            created_at: now.clone(),
            updated_at: now,
        };

        self.db
            .create_automation_run(&run)
            .await
            .map_err(|e| format!("手动提交自动化场次任务失败: {e}"))?;

        log::info!("手动登记场次自动化任务: {} (run_id: {})", run.request_key, run_id);
        self.db.notify_change(DataChange::Automation);
        self.notify.notify_waiters();

        Ok(run_id)
    }

    /// 取消指定自动化场次任务。
    pub async fn cancel_run(&self, run_id: &str) -> Result<(), String> {
        let run = self
            .db
            .get_automation_run(run_id)
            .await
            .map_err(|e| format!("查询自动化任务失败: {e}"))?
            .ok_or_else(|| "自动化任务不存在".to_string())?;

        if let Some(ref tid) = run.task_id {
            let _ = self.task_manager.cancel_task(tid);
        }

        self.db
            .update_automation_run_status(
                run_id,
                "cancelled",
                "idle",
                Some("USER_CANCELLED"),
                Some("用户取消了自动化分析"),
            )
            .await
            .map_err(|e| format!("更新取消状态失败: {e}"))?;

        self.db.notify_change(DataChange::Automation);
        Ok(())
    }

    /// 调度主工作循环。
    async fn run_loop(&self) {
        log::info!("自动化切片调度器已启动");
        while !self.stopped.load(Ordering::SeqCst) {
            tokio::select! {
                _ = self.notify.notified() => {
                    self.tick().await;
                }
                _ = tokio::time::sleep(Duration::from_secs(60)) => {
                    self.tick().await;
                }
            }
        }
        log::info!("自动化切片调度器已退出");
    }

    /// 执行单次调度巡检与任务认领。
    async fn tick(&self) {
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }

        let pending_run = match self.db.get_next_pending_run().await {
            Ok(Some(run)) => run,
            _ => return,
        };

        let lease_owner = format!("worker_{}", std::process::id());
        let claimed = self
            .db
            .claim_automation_run(&pending_run.id, &lease_owner, 300)
            .await
            .unwrap_or(false);

        if !claimed {
            return;
        }

        log::info!("认领自动化场次任务成功: {} (run_id: {})", pending_run.request_key, pending_run.id);

        let manifest_res = SourceManifest::build(
            &self.db,
            &pending_run.platform,
            &pending_run.room_id,
            &pending_run.parent_id,
            &self.cache_dir,
        )
        .await;

        match manifest_res {
            Ok(Some(manifest)) if !manifest.all_sealed => {
                log::info!("录播切片分段尚未封口收尾，等待后重试: run_id={}", pending_run.id);
                let _ = self
                    .db
                    .update_automation_run_status(
                        &pending_run.id,
                        "waiting",
                        "indexing",
                        None,
                        Some("等待录播文件封口落盘"),
                    )
                    .await;
            }
            Ok(Some(manifest)) => {
                log::info!(
                    "录播切片已封口就绪，共 {} 段，总时长 {} ms",
                    manifest.items.len(),
                    manifest.total_duration_ms
                );
                let manifest_json = serde_json::to_string(&manifest.items).unwrap_or_default();

                let _ = self
                    .db
                    .update_run_manifest(
                        &pending_run.id,
                        &manifest_json,
                        &manifest.input_hash,
                        manifest.total_duration_ms,
                    )
                    .await;

                self.db.notify_change(DataChange::Automation);

                let config = self.config.read().await.clone();
                let policy: AutoClippingPolicy =
                    serde_json::from_str(&pending_run.config_snapshot_json).unwrap_or_default();

                let db = self.db.clone();
                let run_id = pending_run.id.clone();
                let platform = pending_run.platform.clone();
                let room_id = pending_run.room_id.clone();
                let parent_id = pending_run.parent_id.clone();
                let live_id = pending_run.live_id.clone();
                let manifest_items = manifest.items.clone();

                tokio::spawn(async move {
                    if let Err(e) = super::analysis::run_analysis_pipeline(
                        &db,
                        &config,
                        &run_id,
                        &platform,
                        &room_id,
                        &parent_id,
                        &live_id,
                        &manifest_items,
                        &policy,
                    )
                    .await
                    {
                        log::error!("自动化切片分析流水线执行失败: {e}");
                        let _ = db
                            .update_automation_run_status(
                                &run_id,
                                "failed",
                                "failed",
                                Some("ANALYSIS_FAILED"),
                                Some(&e),
                            )
                            .await;
                        db.notify_change(DataChange::Automation);
                    }
                });
            }
            Ok(None) => {
                log::warn!("未找到场次录播分段: run_id={}", pending_run.id);
                let _ = self
                    .db
                    .update_automation_run_status(
                        &pending_run.id,
                        "failed",
                        "failed",
                        Some("NO_RECORD_FOUND"),
                        Some("未找到场次录播分段"),
                    )
                    .await;
                self.db.notify_change(DataChange::Automation);
            }
            Err(e) => {
                log::error!("构建来源清单失败: {e}");
                let _ = self
                    .db
                    .update_automation_run_status(
                        &pending_run.id,
                        "failed",
                        "failed",
                        Some("MANIFEST_ERROR"),
                        Some(&e),
                    )
                    .await;
                self.db.notify_change(DataChange::Automation);
            }
        }
    }
}
