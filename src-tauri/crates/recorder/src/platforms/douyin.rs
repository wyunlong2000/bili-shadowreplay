#[cfg(test)]
use tokio::sync::broadcast;
pub mod api;
mod response;
pub mod stream_info;

use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use async_trait::async_trait;
use danmu_stream::provider::ProviderType;
use tokio::sync::{Mutex, RwLock};

use crate::account::Account;
use crate::errors::RecorderError;
use crate::platforms::common::{DanmuConfig, DanmuSpawn, PlatformApi, RoomPoll, StreamPull};
use crate::platforms::douyin::stream_info::{
    collect_available_streams, AvailableStream, DouyinStream,
};
use crate::platforms::PlatformType;
use crate::traits::{RecorderBasicTrait, RecorderTrait};
use crate::{Recorder, RoomInfo, UserInfo};

pub type DouyinRecorder = Recorder<DouyinExtra>;

#[derive(Clone)]
pub struct DouyinExtra {
    sec_user_id: String,
    live_stream: Arc<RwLock<Option<DouyinStream>>>,
    /// Room info of the latest poll, bridging `poll_room` to `poll_stream`.
    ///
    /// Per-poll state, not session state: every successful `poll_room`
    /// overwrites it and `poll_stream` is its only consumer, so it is
    /// deliberately kept out of `clear_stream` — clearing it there would wipe
    /// the just-polled stream on the poll that first sees the live.
    pending: Arc<RwLock<Option<api::DouyinBasicRoomInfo>>>,
    /// 缓存 ttwid 与空响应后的会话重建时间。
    api_session: Arc<Mutex<api::DouyinApiSession>>,
}

fn get_best_stream_url(stream: &DouyinStream) -> Option<String> {
    // find the best stream url
    if stream.data.origin.main.hls.is_empty() {
        log::error!("No stream url found in stream_data: {stream:#?}");
        return None;
    }

    Some(stream.data.origin.main.hls.clone())
}

fn log_douyin_stream_choice(room_id: &str, stream_data: &str, hls_pull_url: &str, selected: &str) {
    let mut available = collect_available_streams(stream_data);
    if !hls_pull_url.is_empty() && !available.iter().any(|stream| stream.url == hls_pull_url) {
        available.push(AvailableStream {
            quality: "hls_pull_url".to_string(),
            variant: "default".to_string(),
            format: "hls",
            url: hls_pull_url.to_string(),
        });
    }

    if available.is_empty() {
        log::info!("[{room_id}] Douyin available streams: (none)");
    } else {
        log::info!("[{room_id}] Douyin available streams:");
        for stream in &available {
            log::info!("[{room_id}]   {}: {}", stream.label(), stream.url);
        }
    }

    let selected_label = available
        .iter()
        .find(|stream| stream.url == selected)
        .map(AvailableStream::label)
        .unwrap_or_else(|| "origin/main hls".to_string());
    log::info!("[{room_id}] Douyin selected stream: {selected_label} {selected}");
}

impl DouyinRecorder {
    /// 创建抖音录制器，并设置主播标识类型和已知直播间号。
    pub async fn new(
        room_id: &str,
        sec_user_id: &str,
        account: &Account,
        cache_dir: PathBuf,
        channel: impl Into<crate::events::RecorderEventSender>,
        update_interval: Arc<AtomicU64>,
        enabled: bool,
        identifier_type: &str,
        web_rid: Option<&str>,
        profile_name: &str,
        profile_avatar: &str,
    ) -> Result<Self, crate::errors::RecorderError> {
        let effective_web_rid = match identifier_type {
            "room" => Some(room_id.to_string()),
            "profile" => web_rid.map(str::to_string),
            _ => return Err(RecorderError::InvalidValue),
        };
        let recorder = Self::with_extra(
            PlatformType::Douyin,
            room_id,
            account,
            cache_dir,
            channel,
            update_interval,
            enabled,
            DouyinExtra {
                sec_user_id: sec_user_id.to_string(),
                live_stream: Arc::new(RwLock::new(None)),
                pending: Arc::new(RwLock::new(None)),
                api_session: Arc::new(Mutex::new(api::DouyinApiSession::new(
                    account.cookies.clone(),
                ))),
            },
        );
        recorder.set_identity_metadata(identifier_type, effective_web_rid);
        if identifier_type == "profile" {
            *recorder.room_info().write().await = RoomInfo {
                platform: PlatformType::Douyin.as_str().to_string(),
                room_id: room_id.to_string(),
                room_title: profile_name.to_string(),
                room_cover: profile_avatar.to_string(),
                status: false,
            };
            *recorder.user_info().write().await = UserInfo {
                user_id: sec_user_id.to_string(),
                user_name: profile_name.to_string(),
                user_avatar: profile_avatar.to_string(),
            };
        }
        Ok(recorder)
    }

    /// 将主页接口确认的未开播状态转换为直播间轮询结果。
    async fn profile_offline_poll(&self, profile: api::DouyinProfileLiveInfo) -> RoomPoll {
        let current_room = self.room_info().read().await.clone();
        let current_user = self.user_info().read().await.clone();
        let nickname = if profile.nickname.is_empty() {
            current_user.user_name
        } else {
            profile.nickname
        };
        let avatar = if profile.avatar.is_empty() {
            current_user.user_avatar
        } else {
            profile.avatar
        };

        RoomPoll {
            live: false,
            room_title: if current_room.room_title.is_empty() {
                nickname.clone()
            } else {
                current_room.room_title
            },
            room_cover: if current_room.room_cover.is_empty() {
                avatar.clone()
            } else {
                current_room.room_cover
            },
            user: Some(UserInfo {
                user_id: self.extra.sec_user_id.clone(),
                user_name: nickname,
                user_avatar: avatar,
            }),
            platform_live_id: None,
        }
    }
}

#[async_trait]
impl PlatformApi for DouyinRecorder {
    async fn poll_room(&self) -> Result<RoomPoll, RecorderError> {
        let identifier_type = self.identifier_type();
        let mut request_account = self.account().clone();
        let (mut web_rid, resolved_now) = match self.effective_web_rid() {
            Some(web_rid) => {
                if identifier_type == "profile" {
                    let mut session = self.extra.api_session.lock().await;
                    api::ensure_douyin_session(&self.client(), &mut session).await?;
                    request_account.cookies = session.cookies.clone();
                }
                (web_rid, false)
            }
            None if identifier_type == "profile" => {
                let mut session = self.extra.api_session.lock().await;
                let profile = api::check_user_live_by_profile_api(
                    &self.client(),
                    &mut session,
                    &self.extra.sec_user_id,
                )
                .await?;
                request_account.cookies = session.cookies.clone();
                if !profile.live {
                    return Ok(self.profile_offline_poll(profile).await);
                }
                (
                    profile.web_rid.ok_or(RecorderError::NoRoomInfo)?,
                    true,
                )
            }
            None => return Err(RecorderError::NoRoomInfo),
        };
        let info = match api::get_room_info(
            &self.client,
            &request_account,
            &web_rid,
            &self.extra.sec_user_id,
        )
        .await
        {
            Ok(info) => {
                info
            }
            Err(room_error) if identifier_type == "profile" => {
                let consecutive_unknown =
                    self.detection_state().read().await.consecutive_unknown;
                if resolved_now || consecutive_unknown < 2 {
                    return Err(room_error);
                }
                let profile_result = {
                    let mut session = self.extra.api_session.lock().await;
                    let result = api::check_user_live_by_profile_api(
                        &self.client(),
                        &mut session,
                        &self.extra.sec_user_id,
                    )
                    .await;
                    request_account.cookies = session.cookies.clone();
                    result
                };
                match profile_result {
                    Ok(profile) if !profile.live => {
                        log::info!("抖音直播间接口查询失败，主页接口确认主播未开播");
                        return Ok(self.profile_offline_poll(profile).await);
                    }
                    Ok(profile) => {
                        let refreshed_web_rid =
                            profile.web_rid.ok_or(RecorderError::NoRoomInfo)?;
                        if refreshed_web_rid == web_rid {
                            return Err(room_error);
                        }
                        log::info!("抖音主页接口确认仍在直播，更新直播间号后重试详情查询");
                        let refreshed_info = api::get_room_info(
                            &self.client,
                            &request_account,
                            &refreshed_web_rid,
                            &self.extra.sec_user_id,
                        )
                        .await?;
                        if refreshed_info.status != 0 {
                            return Err(RecorderError::ApiError {
                                error: "抖音主页接口确认开播，但直播间详情未确认开播".to_string(),
                            });
                        }
                        web_rid = refreshed_web_rid;
                        self.set_identity_metadata("profile", Some(web_rid.clone()));
                        if self
                            .event_channel()
                            .send(crate::events::RecorderEvent::WebRidUpdated {
                                platform: PlatformType::Douyin,
                                room_id: self.room_id(),
                                web_rid: web_rid.clone(),
                            })
                            .await
                            .is_err()
                        {
                            log::warn!("发送抖音直播间号更新事件失败");
                        }
                        refreshed_info
                    }
                    Err(profile_error) => {
                        log::warn!("抖音直播间查询失败，主页状态复核也失败：{room_error}；{profile_error}");
                        return Err(profile_error);
                    }
                }
            }
            Err(error) => return Err(error),
        };
        if resolved_now && info.status == 0 {
            self.set_identity_metadata("profile", Some(web_rid));
        }
        *self.extra.pending.write().await = Some(info.clone());

        Ok(RoomPoll {
            // room_status == 0 表示正在直播
            live: info.status == 0,
            room_title: info.room_title.clone(),
            room_cover: info.cover.clone().unwrap_or_default(),
            user: Some(UserInfo {
                user_id: info.sec_user_id.clone(),
                user_name: info.user_name.clone(),
                user_avatar: info.user_avatar.clone(),
            }),
            // The douyin live id is only known together with the stream.
            platform_live_id: None,
        })
    }

    async fn poll_stream(&self) -> bool {
        let Some(info) = self.extra.pending.write().await.take() else {
            return true;
        };

        // An empty hls url keeps the stream of the previous poll.
        if info.hls_url.is_empty() {
            return true;
        }

        let Ok(stream) = serde_json::from_str::<DouyinStream>(&info.stream_data) else {
            log::error!("Failed to parse stream data: {info:#?}");
            return false;
        };
        let Some(new_stream_url) = get_best_stream_url(&stream) else {
            return false;
        };

        log_douyin_stream_choice(
            &self.room_id,
            &info.stream_data,
            &info.hls_url,
            &new_stream_url,
        );
        *self.extra.live_stream.write().await = Some(stream);
        *self.platform_live_id.write().await = info.room_id_str;

        true
    }

    async fn open_pull(&self, live_id: &str) -> Result<StreamPull, RecorderError> {
        let Some(stream) = self.extra.live_stream.read().await.clone() else {
            return Err(RecorderError::NoStreamAvailable);
        };
        let Some(stream_url) = get_best_stream_url(&stream) else {
            return Err(RecorderError::NoStreamAvailable);
        };

        StreamPull::hls(live_id, &stream_url, None).await
    }

    async fn clear_stream(&self) {
        // `pending` carries the fresh poll into `poll_stream` and must survive
        // the live-transition reset.
        *self.extra.live_stream.write().await = None;
    }

    fn danmu_config(&self) -> Option<DanmuConfig> {
        Some(DanmuConfig {
            provider: ProviderType::Douyin,
            spawn: DanmuSpawn::PerRecording,
        })
    }

    async fn danmu_room_id(&self) -> String {
        // The danmu service subscribes by numeric room id, which is only set
        // together with the stream.
        self.platform_live_id
            .read()
            .await
            .clone()
            .parse::<i64>()
            .unwrap_or(0)
            .to_string()
    }
}

#[async_trait]
impl RecorderTrait for DouyinRecorder {
    async fn run(&self) {
        self.run_recording_loop().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;

    #[tokio::test]
    async fn clear_stream_keeps_the_fresh_polls_bridge() {
        let (tx, _rx) = broadcast::channel(1);
        let recorder = DouyinRecorder::new(
            "room",
            "sec-user-id",
            &Account::default(),
            std::env::temp_dir().join(format!("bsr-douyin-{}", uuid::Uuid::new_v4())),
            tx,
            Arc::new(AtomicU64::new(30)),
            true,
            "room",
            None,
            "",
            "",
        )
        .await
        .unwrap();
        *recorder.extra.pending.write().await = Some(api::DouyinBasicRoomInfo {
            room_id_str: "123".to_string(),
            room_title: "title".to_string(),
            cover: None,
            status: 0,
            hls_url: "https://example.com/index.m3u8".to_string(),
            stream_data: "{}".to_string(),
            user_name: "anchor".to_string(),
            user_avatar: String::new(),
            sec_user_id: "sec-user-id".to_string(),
        });

        // The live-transition reset runs between `poll_room` and
        // `poll_stream`: it must clear the session stream but keep the fresh
        // poll's bridge, or the first recording attempt finds no stream.
        recorder.clear_stream().await;

        assert!(recorder.extra.pending.read().await.is_some());
        assert!(recorder.extra.live_stream.read().await.is_none());
    }
}
