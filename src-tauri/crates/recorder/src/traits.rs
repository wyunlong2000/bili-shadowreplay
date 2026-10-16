use std::{
    path::PathBuf,
    sync::{atomic, Arc},
};

use crate::platforms::PlatformType;
use crate::{
    account::Account, danmu::DanmuStorage, errors::RecorderError, events::RecorderEvent,
    CachePath, DetectionErrorKind, DetectionState, RecorderInfo, RoomInfo, UserInfo,
};
use async_trait::async_trait;
use tokio::{
    sync::{Mutex, RwLock},
    task::JoinHandle,
};

#[allow(dead_code)]
pub trait RecorderBasicTrait {
    fn platform(&self) -> PlatformType;
    fn room_id(&self) -> String;
    fn account(&self) -> &Account;
    fn client(&self) -> &reqwest::Client;
    fn event_channel(&self) -> &crate::events::RecorderEventSender;
    fn set_preview_sink(&self, sink: crate::events::PreviewEventSink);
    fn emit_preview_event(&self, event: RecorderEvent);
    fn cache_dir(&self) -> PathBuf;
    fn quit(&self) -> Arc<atomic::AtomicBool>;
    fn enabled(&self) -> Arc<atomic::AtomicBool>;
    fn is_recording(&self) -> Arc<atomic::AtomicBool>;
    fn room_info(&self) -> Arc<RwLock<RoomInfo>>;
    fn user_info(&self) -> Arc<RwLock<UserInfo>>;
    fn platform_live_id(&self) -> Arc<RwLock<String>>;
    fn live_id(&self) -> Arc<RwLock<String>>;
    fn danmu_task(&self) -> Arc<Mutex<Option<JoinHandle<()>>>>;
    fn record_task(&self) -> Arc<Mutex<Option<JoinHandle<()>>>>;
    fn danmu_storage(&self) -> Arc<RwLock<Option<DanmuStorage>>>;
    fn last_update(&self) -> &atomic::AtomicI64;
    fn last_sequence(&self) -> &atomic::AtomicU64;
    fn update_interval(&self) -> Arc<atomic::AtomicU64>;
    /// The live id of the previous recording attempt, kept for resuming the
    /// same recording after a stream expiry.
    fn pre_live_id(&self) -> Arc<RwLock<Option<String>>>;
    /// Whether the next recording attempt should reuse `pre_live_id`.
    fn should_continue(&self) -> &atomic::AtomicBool;
    /// 返回由录制器共享的监测状态。
    fn detection_state(&self) -> Arc<RwLock<DetectionState>>;
    /// 返回主播标识类型，默认按房间号处理。
    fn identifier_type(&self) -> String {
        "room".to_string()
    }
    /// 指示当前标识是否已解析为有效房间号。
    fn room_resolved(&self) -> bool {
        true
    }
    /// 指示轮询间隔是否由监测退避状态控制。
    fn uses_detection_backoff(&self) -> bool {
        false
    }
    /// 返回已解析的有效直播间号。
    fn effective_web_rid(&self) -> Option<String> {
        None
    }
    /// 将录制器错误映射为监测错误分类。
    fn classify_detection_error(&self, error: &RecorderError) -> DetectionErrorKind {
        match error {
            RecorderError::SecurityControlError => DetectionErrorKind::RateLimited,
            RecorderError::InvalidCookies => DetectionErrorKind::AccountInvalid,
            RecorderError::ClientError(_) => DetectionErrorKind::Network,
            RecorderError::InvalidResponse
            | RecorderError::InvalidResponseJson { .. }
            | RecorderError::InvalidResponseStatus { .. }
            | RecorderError::M3u8ParseFailed { .. } => DetectionErrorKind::Parse,
            RecorderError::ApiError { error } => {
                let message = error.to_ascii_lowercase();
                if message.contains("captcha")
                    || message.contains("验证码")
                    || message.contains("安全验证")
                    || message.contains("访问过于频繁")
                {
                    DetectionErrorKind::Verification
                } else if message.contains("cookie")
                    || message.contains("登录")
                    || message.contains("invalid session")
                {
                    DetectionErrorKind::AccountInvalid
                } else if message.contains("request params error")
                    || message.contains("status_code: 10011")
                {
                    DetectionErrorKind::RequestRejected
                } else {
                    DetectionErrorKind::Parse
                }
            }
            _ => DetectionErrorKind::Network,
        }
    }
}

#[async_trait]
pub trait RecorderTrait: RecorderBasicTrait {
    async fn run(&self);
    async fn stop(&self) {
        self.quit().store(true, atomic::Ordering::Relaxed);
        if let Some(danmu_task) = self.danmu_task().lock().await.take() {
            danmu_task.abort();
            let _ = danmu_task.await;
        }
        if let Some(record_task) = self.record_task().lock().await.take() {
            record_task.abort();
            let _ = record_task.await;
        }
    }
    async fn should_record(&self) -> bool {
        if self.quit().load(atomic::Ordering::Relaxed) {
            return false;
        }

        self.enabled().load(atomic::Ordering::Relaxed)
    }

    async fn work_dir(&self, live_id: &str) -> CachePath {
        CachePath::new(self.cache_dir(), self.platform(), &self.room_id(), live_id)
    }
    async fn info(&self) -> RecorderInfo {
        let room_info = self.room_info().read().await.clone();
        let user_info = self.user_info().read().await.clone();
        let detection = self.detection_state().read().await.clone();
        let is_recording = self.is_recording().load(atomic::Ordering::Relaxed);
        RecorderInfo {
            platform_live_id: self.platform_live_id().read().await.clone(),
            live_id: self.live_id().read().await.clone(),
            recording: is_recording,
            enabled: self.enabled().load(atomic::Ordering::Relaxed),
            room_info: RoomInfo {
                platform: self.platform().as_str().to_string(),
                room_id: self.room_id().to_string(),
                room_title: room_info.room_title.clone(),
                room_cover: room_info.room_cover.clone(),
                status: room_info.status,
            },
            user_info: UserInfo {
                user_id: user_info.user_id.to_string(),
                user_name: user_info.user_name.clone(),
                user_avatar: user_info.user_avatar.clone(),
            },
            detection_status: detection.status().to_string(),
            detection_error_kind: detection.error_kind.as_str().to_string(),
            last_checked_at: detection.last_checked_at,
            last_success_at: detection.last_success_at,
            next_retry_at: detection.next_retry_at,
            consecutive_unknown: detection.consecutive_unknown,
            room_resolved: self.room_resolved(),
            identifier_type: self.identifier_type(),
            web_rid: self.effective_web_rid(),
        }
    }

    /// 注入从数据库恢复的监测状态。
    async fn set_detection_state(&self, state: DetectionState) {
        *self.detection_state().write().await = state;
    }

    /// 向管理器发送监测状态变更事件。
    async fn publish_detection_state(&self) {
        let state = self.detection_state().read().await.clone();
        let _ = self
            .event_channel()
            .send(RecorderEvent::DetectionUpdated {
                platform: self.platform(),
                room_id: self.room_id(),
                state,
            })
            .await;
    }

    /// 记录一次成功探测并清除连续未知和退避状态。
    async fn record_detection_success(&self, live: bool) {
        let now = chrono::Utc::now().timestamp();
        {
            let detection_state = self.detection_state();
            let mut state = detection_state.write().await;
            state.confirmed = Some(live);
            state.last_checked_at = now;
            state.last_success_at = now;
            state.error_kind = DetectionErrorKind::None;
            state.consecutive_unknown = 0;
            state.next_retry_at = now;
            state.backoff_level = 0;
        }
        self.publish_detection_state().await;
    }

    /// 记录一次失败探测并按已确认的退避序列安排重试。
    async fn record_detection_failure(&self, error: &RecorderError) {
        let now = chrono::Utc::now().timestamp();
        {
            let detection_state = self.detection_state();
            let mut state = detection_state.write().await;
            state.last_checked_at = now;
            state.error_kind = self.classify_detection_error(error);
            state.consecutive_unknown = state.consecutive_unknown.saturating_add(1);
            state.backoff_level = state.backoff_level.saturating_add(1).min(5);
            let delays = [30_i64, 60, 120, 240, 300];
            let delay = delays[state.backoff_level.saturating_sub(1) as usize];
            state.next_retry_at = now.saturating_add(delay);
        }
        self.publish_detection_state().await;
    }

    /// 返回当前监测状态对应的下一次轮询等待时长。
    async fn current_poll_delay(&self) -> std::time::Duration {
        let detection_state = self.detection_state();
        let state = detection_state.read().await;
        let interval = self.update_interval().load(atomic::Ordering::Relaxed).max(30);
        let delay = if state.backoff_level == 0 {
            interval
        } else {
            let delays = [30_u64, 60, 120, 240, 300];
            delays[state.backoff_level.saturating_sub(1) as usize]
        };
        std::time::Duration::from_secs(delay.max(interval))
    }

    /// 等待已持久化的重试时间，并定期检查手动重置。
    async fn wait_for_detection_retry(&self) {
        loop {
            let state = self.detection_state().read().await.clone();
            if state.consecutive_unknown == 0 {
                return;
            }
            let remaining = state
                .next_retry_at
                .saturating_sub(chrono::Utc::now().timestamp());
            if remaining <= 0 {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_secs(
                u64::try_from(remaining.min(1)).unwrap_or_default(),
            ))
            .await;
        }
    }

    /// 启用录制器，并在抖音模式下重置连续未知计数和退避状态。
    async fn enable(&self) {
        self.enabled().store(true, atomic::Ordering::Relaxed);
        if self.uses_detection_backoff() {
            let now = chrono::Utc::now().timestamp();
            {
                let detection_state = self.detection_state();
                let mut state = detection_state.write().await;
                state.consecutive_unknown = 0;
                state.backoff_level = 0;
                state.next_retry_at = now;
            }
            self.publish_detection_state().await;
        }
    }
    async fn disable(&self) {
        self.enabled().store(false, atomic::Ordering::Relaxed);
    }
}
