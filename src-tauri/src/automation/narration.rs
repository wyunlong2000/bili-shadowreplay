//! AI 解说文案规划、TTS 语音合成与音频资产管理模块。
//!
//! 负责根据高光片段与证据生成适合的解说文案，调用 TTS 引擎合成音频，
//! 并将其作为音频资产注册到切片工程时间轴中。

use std::path::Path;
use chrono::Local;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::evidence::EvidenceCue;
use super::types::AutomationCandidateRow;
use crate::database::audio::AudioRow;
use crate::database::Database;

/// 规划生成的解说片段。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedNarration {
    /// 解说文案内容。
    pub text: String,
    /// 在素材内的触发开始时间（秒）。
    pub start_sec: f64,
    /// 触发时的原声压低音量比例（0.0 ~ 1.0，通常 0.25）。
    pub ducking_volume: f64,
}

/// 解说规划器。
pub struct NarrationPlanner;

impl NarrationPlanner {
    /// 根据候选片段类型、理由与字幕证据，规划解说文案与触发时间点。
    pub fn plan_narrations(
        candidate: &AutomationCandidateRow,
        cues: &[EvidenceCue],
        clip_duration_sec: f64,
    ) -> Vec<GeneratedNarration> {
        let mut list = Vec::new();
        let content_type = candidate.content_type.as_str();
        let reason = candidate.reason.trim();

        // 1. 开场引入解说（通常在素材开头 0.5 秒切入）
        let opening_text = match content_type {
            "gaming" => {
                if !reason.is_empty() {
                    format!("注意看！这波操作非同寻常，主播即将展现精彩瞬间。")
                } else {
                    format!("高能预警！关键团战一触即发，来看主播的极限操作。")
                }
            }
            "chat" => {
                format!("直播间气氛突然热烈起来，主播和弹幕聊到了高潮部分。")
            }
            "knowledge" => {
                format!("这里主播分享了一个非常核心的关键干货知识点，值得细品。")
            }
            "sports" => {
                format!("比赛进入到了最扣人心弦的白热化阶段，胜负在此一举！")
            }
            _ => {
                format!("前方高能！主播迎来了本场直播的绝对高光时刻。")
            }
        };

        list.push(GeneratedNarration {
            text: opening_text,
            start_sec: 0.5,
            ducking_volume: 0.25,
        });

        // 2. 如果片段时长超过 12 秒，在后半程规划第二段点评解说
        if clip_duration_sec >= 12.0 {
            let second_start = (clip_duration_sec * 0.55).min(clip_duration_sec - 4.0).max(4.5);
            let highlight_text = if !cues.is_empty() {
                format!("这一波直接点燃了全场弹幕，可以说是行云流水、一气呵成！")
            } else if !reason.is_empty() {
                format!("正如大家所见，{}，这波发挥堪称教科书级别。", reason)
            } else {
                format!("这波操作直接拉满，细节决定成败，实在太秀了！")
            };

            list.push(GeneratedNarration {
                text: highlight_text,
                start_sec: second_start,
                ducking_volume: 0.25,
            });
        }

        list
    }
}

/// TTS 语音合成器。
pub struct TtsSynthesizer;

impl TtsSynthesizer {
    /// 合成解说音频并保存到指定路径，返回合成音频的真实时长（秒）。
    pub async fn synthesize(
        endpoint: &str,
        api_key: &str,
        model: &str,
        voice: &str,
        text: &str,
        save_path: &Path,
    ) -> Result<f64, String> {
        // 如果用户配置了有效 API Key，则发起标准 OpenAI-compatible TTS 请求
        if !api_key.trim().is_empty() {
            match Self::call_openai_tts(endpoint, api_key, model, voice, text, save_path).await {
                Ok(duration) => return Ok(duration),
                Err(err) => {
                    log::warn!("调用在线 TTS 失败，将回退生成离线提示音: {}", err);
                }
            }
        }

        // 离线/未配置 Key 时的安全回退：生成标准 PCM 16kHz WAV 音频
        Self::generate_placeholder_wav(text, save_path).await
    }

    /// 发起 OpenAI TTS 协议的 HTTP 请求。
    async fn call_openai_tts(
        endpoint: &str,
        api_key: &str,
        model: &str,
        voice: &str,
        text: &str,
        save_path: &Path,
    ) -> Result<f64, String> {
        let clean_endpoint = endpoint.trim().trim_end_matches('/');
        let url = if clean_endpoint.ends_with("/audio/speech") {
            clean_endpoint.to_string()
        } else {
            format!("{clean_endpoint}/audio/speech")
        };

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(45))
            .build()
            .map_err(|e| format!("构建 HTTP 客户端失败: {e}"))?;

        let body = serde_json::json!({
            "model": if model.is_empty() { "tts-1" } else { model },
            "input": text,
            "voice": if voice.is_empty() { "alloy" } else { voice },
            "response_format": "wav"
        });

        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key.trim()))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("TTS 网络请求失败: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await.unwrap_or_default();
            return Err(format!("TTS 服务响应错误 (HTTP {status}): {err_text}"));
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("读取 TTS 音频数据失败: {e}"))?;

        if let Some(parent) = save_path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        tokio::fs::write(save_path, &bytes)
            .await
            .map_err(|e| format!("写入 TTS 文件失败: {e}"))?;

        // 估算或探测音频时长
        let duration = Self::probe_wav_duration(&bytes)
            .unwrap_or_else(|| (bytes.len() as f64) / (16000.0 * 2.0));
        Ok(duration.max(1.0))
    }

    /// 生成轻量标准单声道 16-bit 16kHz PCM WAV 引导音频。
    pub async fn generate_placeholder_wav(text: &str, save_path: &Path) -> Result<f64, String> {
        let char_count = text.chars().count().max(4);
        // 按每字约 0.22 秒估算时长
        let duration_sec = ((char_count as f64) * 0.22).clamp(1.5, 15.0);
        let sample_rate = 16000u32;
        let num_samples = (duration_sec * sample_rate as f64) as u32;

        let mut pcm_bytes = Vec::with_capacity((num_samples * 2) as usize);
        for i in 0..num_samples {
            // 生成柔和的正弦引导音，前 0.2 秒 440Hz，随后平稳淡出
            let t = (i as f64) / (sample_rate as f64);
            let amp = if t < 0.2 {
                0.15 * (1.0 - t / 0.2)
            } else {
                0.001
            };
            let sample = (f64::sin(2.0 * std::f64::consts::PI * 440.0 * t) * amp * 32767.0) as i16;
            pcm_bytes.extend_from_slice(&sample.to_le_bytes());
        }

        let wav_header = build_wav_header(sample_rate, 1, 16, pcm_bytes.len() as u32);
        let mut full_file = wav_header;
        full_file.extend_from_slice(&pcm_bytes);

        if let Some(parent) = save_path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        tokio::fs::write(save_path, full_file)
            .await
            .map_err(|e| format!("写入引导音频文件失败: {e}"))?;

        Ok(duration_sec)
    }

    /// 简易解析 WAV 头部获取时长（秒）。
    fn probe_wav_duration(bytes: &[u8]) -> Option<f64> {
        if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
            return None;
        }
        let sample_rate = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]) as f64;
        let byte_rate = u32::from_le_bytes([bytes[28], bytes[29], bytes[30], bytes[31]]) as f64;
        let data_size = (bytes.len() - 44) as f64;
        if byte_rate > 0.0 {
            Some(data_size / byte_rate)
        } else if sample_rate > 0.0 {
            Some(data_size / (sample_rate * 2.0))
        } else {
            None
        }
    }
}

/// 注册音频文件到数据库 `audio_files` 表。
pub async fn register_narration_audio_file(
    db: &Database,
    relative_file: &str,
    name: &str,
    duration_sec: f64,
    size: i64,
) -> Result<i64, String> {
    let now = Local::now().to_rfc3339();
    let row = db
        .add_audio_file(&AudioRow {
            id: 0,
            name: name.to_string(),
            file: relative_file.to_string(),
            duration: duration_sec,
            size,
            created_at: now,
        })
        .await
        .map_err(|e| format!("注册解说音频资产失败: {e}"))?;

    Ok(row.id)
}

/// 构造标准的 WAV 文件头部（44 字节）。
fn build_wav_header(sample_rate: u32, channels: u16, bits_per_sample: u16, data_len: u32) -> Vec<u8> {
    let mut header = Vec::with_capacity(44);
    let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
    let block_align = channels * bits_per_sample / 8;
    let total_len = 36 + data_len;

    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&total_len.to_le_bytes());
    header.extend_from_slice(b"WAVE");
    header.extend_from_slice(b"fmt ");
    header.extend_from_slice(&16u32.to_le_bytes()); // PCM 头部长度
    header.extend_from_slice(&1u16.to_le_bytes());  // PCM 格式
    header.extend_from_slice(&channels.to_le_bytes());
    header.extend_from_slice(&sample_rate.to_le_bytes());
    header.extend_from_slice(&byte_rate.to_le_bytes());
    header.extend_from_slice(&block_align.to_le_bytes());
    header.extend_from_slice(&bits_per_sample.to_le_bytes());
    header.extend_from_slice(b"data");
    header.extend_from_slice(&data_len.to_le_bytes());

    header
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_placeholder_wav_generation() {
        let dir = tempfile::tempdir().unwrap();
        let wav_path = dir.path().join("test_narration.wav");

        let duration = TtsSynthesizer::generate_placeholder_wav("精彩操作拉满", &wav_path)
            .await
            .expect("生成引导音频失败");

        assert!(duration > 1.0);
        let bytes = tokio::fs::read(&wav_path).await.unwrap();
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
    }

    #[test]
    fn test_narration_planning() {
        let candidate = AutomationCandidateRow {
            id: "c_test".into(),
            run_id: "r_1".into(),
            platform: "bilibili".into(),
            room_id: "1001".into(),
            parent_id: "p_1".into(),
            live_id: "l_1".into(),
            start_ms: 10_000,
            end_ms: 25_000,
            score: 85.0,
            reason: "逆风翻盘五杀".into(),
            status: "pending".into(),
            content_type: "gaming".into(),
            confidence: 0.9,
            manual_modified: false,
            revision: 0,
            clip_task_id: None,
            video_id: None,
            error_code: None,
            error_message: None,
            created_at: "".into(),
            updated_at: "".into(),
        };

        let narrations = NarrationPlanner::plan_narrations(&candidate, &[], 15.0);
        assert_eq!(narrations.len(), 2);
        assert!(narrations[0].start_sec < 1.0);
        assert!(narrations[1].start_sec >= 4.0);
        assert_eq!(narrations[0].ducking_volume, 0.25);
    }
}
