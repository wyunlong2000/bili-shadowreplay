use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::types::{ClipOutputMode, LiveContentType};

/// 全局或合并后的自动化切片有效策略。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AutoClippingPolicy {
    /// 是否在下播后自动触发分析。
    pub enabled: bool,
    /// 默认直播内容类型。
    pub content_type: LiveContentType,
    /// 是否开启大模型自动分类。
    pub auto_classify: bool,
    /// 期望生成的成片版本。
    pub output_mode: ClipOutputMode,
    /// 分析滑窗大小（秒）。
    pub window_seconds: u64,
    /// 滑窗前后重叠上下文（秒）。
    pub overlap_seconds: u64,
    /// 单场录播最大生成的候选数量。
    pub max_candidates: usize,
    /// 候选录取的最低评分门槛。
    pub min_score: f64,
}

impl Default for AutoClippingPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            content_type: LiveContentType::Mixed,
            auto_classify: true,
            output_mode: ClipOutputMode::Both,
            window_seconds: 300,
            overlap_seconds: 30,
            max_candidates: 5,
            min_score: 60.0,
        }
    }
}

/// 房间级覆盖策略，None 字段表示继承全局默认配置。
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RoomPolicyOverride {
    pub enabled: Option<bool>,
    pub content_type: Option<LiveContentType>,
    pub auto_classify: Option<bool>,
    pub output_mode: Option<ClipOutputMode>,
    pub max_candidates: Option<usize>,
    pub min_score: Option<f64>,
}

impl AutoClippingPolicy {
    /// 将全局策略与房间级覆盖合并为最终生效策略。
    pub fn merge_room_override(&self, room_override: Option<&RoomPolicyOverride>) -> Self {
        let Some(overrides) = room_override else {
            return self.clone();
        };

        Self {
            enabled: overrides.enabled.unwrap_or(self.enabled),
            content_type: overrides.content_type.unwrap_or(self.content_type),
            auto_classify: overrides.auto_classify.unwrap_or(self.auto_classify),
            output_mode: overrides.output_mode.unwrap_or(self.output_mode),
            window_seconds: self.window_seconds,
            overlap_seconds: self.overlap_seconds,
            max_candidates: overrides.max_candidates.unwrap_or(self.max_candidates),
            min_score: overrides.min_score.unwrap_or(self.min_score),
        }
    }

    /// 校验策略的合法性约束。
    pub fn validate(&self) -> Result<(), String> {
        if self.content_type == LiveContentType::Mixed && !self.auto_classify {
            return Err("混合内容类型必须开启自动片段识别".to_string());
        }
        if self.window_seconds <= self.overlap_seconds {
            return Err("分析窗口时长必须大于重叠时长".to_string());
        }
        if self.max_candidates == 0 {
            return Err("单场候选数量上限必须大于 0".to_string());
        }
        Ok(())
    }

    /// 计算策略的唯一指纹，用于幂等判定与缓存失效。
    pub fn fingerprint(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}
