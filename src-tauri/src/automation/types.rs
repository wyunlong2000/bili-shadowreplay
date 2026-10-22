use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// 直播内容类型，决定候选筛选与剪辑策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LiveContentType {
    Chat,
    Knowledge,
    Gaming,
    Sports,
    Commerce,
    #[default]
    Mixed,
    #[serde(other)]
    Other,
}

impl LiveContentType {
    /// 返回内容类型的英文标识字符串。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Knowledge => "knowledge",
            Self::Gaming => "gaming",
            Self::Sports => "sports",
            Self::Commerce => "commerce",
            Self::Mixed => "mixed",
            Self::Other => "other",
        }
    }

    /// 返回内容类型的中文可读名称。
    pub fn label(&self) -> &'static str {
        match self {
            Self::Chat => "聊天互动",
            Self::Knowledge => "知识科普",
            Self::Gaming => "游戏实况",
            Self::Sports => "赛事竞技",
            Self::Commerce => "带货解说",
            Self::Mixed => "混合内容",
            Self::Other => "其他类型",
        }
    }
}

/// 候选切片需要生成的成片版本偏好。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ClipOutputMode {
    Original,
    Commentary,
    #[default]
    Both,
}

impl ClipOutputMode {
    /// 返回成片版本的中文说明。
    pub fn label(&self) -> &'static str {
        match self {
            Self::Original => "原声增强版",
            Self::Commentary => "AI 解说版",
            Self::Both => "双版本同时生成",
        }
    }
}

/// 成片结果表中记录的具体生成模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationOutputMode {
    Original,
    Commentary,
}

/// 自动化场次运行状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AutomationRunStatus {
    #[default]
    Pending,
    Processing,
    Waiting,
    Success,
    PartialSuccess,
    Failed,
    Interrupted,
    Cancelled,
}

/// 自动化处理细分阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AutomationStage {
    #[default]
    Idle,
    Indexing,
    Transcribing,
    Analyzing,
    Reviewing,
    Arranging,
    Synthesizing,
    Rendering,
    Verifying,
    Completed,
    Failed,
}

/// 候选切片状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AutomationCandidateStatus {
    #[default]
    Pending,
    Processing,
    Completed,
    Rejected,
    Failed,
}

/// 单版本成片状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AutomationOutputStatus {
    #[default]
    Pending,
    Processing,
    Success,
    Failed,
    Cancelled,
}

/// 房间自动化策略数据库行结构。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AutomationRoomPolicyRow {
    pub platform: String,
    pub room_id: String,
    pub policy_json: String,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// 自动化场次运行记录数据库行结构。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AutomationRunRow {
    pub id: String,
    pub platform: String,
    pub room_id: String,
    pub parent_id: String,
    pub live_id: String,
    pub range_start_ms: i64,
    pub range_end_ms: i64,
    pub request_key: String,
    pub input_hash: String,
    pub config_snapshot_json: String,
    pub policy_fingerprint: String,
    pub status: String,
    pub stage: String,
    pub task_id: Option<String>,
    pub lease_owner: Option<String>,
    pub lease_expires_at: Option<String>,
    pub source_manifest_json: String,
    pub completed_windows: i64,
    pub total_windows: i64,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 自动化分析检查点数据库行结构。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AutomationAnalysisRow {
    pub id: String,
    pub run_id: String,
    pub platform: String,
    pub room_id: String,
    pub live_id: String,
    pub parent_id: String,
    pub status: String,
    pub stage: String,
    pub processed_until_ms: i64,
    pub window_seconds: i64,
    pub subtitle_json: Option<String>,
    pub checkpoints_json: Option<String>,
    pub input_hash: String,
    pub task_id: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 自动化高光候选记录数据库行结构。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AutomationCandidateRow {
    pub id: String,
    pub run_id: String,
    pub platform: String,
    pub room_id: String,
    pub parent_id: String,
    pub live_id: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub score: f64,
    pub signals_json: String,
    pub reason: String,
    pub status: String,
    pub content_type: String,
    pub confidence: f64,
    pub evidence_json: String,
    pub manual_modified: i64,
    pub revision: i64,
    pub clip_task_id: Option<String>,
    pub video_id: Option<i64>,
    pub notify_status: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 自动化成片输出记录数据库行结构。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AutomationOutputRow {
    pub id: String,
    pub candidate_id: String,
    pub output_mode: String,
    pub plan_version: i64,
    pub plan_json: String,
    pub plan_hash: String,
    pub project_id: Option<i64>,
    pub material_id: Option<i64>,
    pub assets_json: String,
    pub render_task_id: Option<String>,
    pub status: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 录播来源清单项，对应一个已录制完成的切片录制段。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceManifestItem {
    pub live_id: String,
    pub start_time_ms: i64,
    pub duration_ms: i64,
    pub file_path: String,
    pub sealed: bool,
}

/// 候选依据引用结构，用于可追溯审查。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CandidateEvidence {
    #[serde(default)]
    pub cue_ids: Vec<String>,
    #[serde(default)]
    pub frame_timestamps_ms: Vec<i64>,
    #[serde(default)]
    pub danmaku_peaks: Vec<i64>,
    #[serde(default)]
    pub notes: String,
}

/// 剪辑方案片段项（时间单位：毫秒）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditPlanClip {
    pub id: String,
    pub live_id: String,
    pub source_start_ms: i64,
    pub source_end_ms: i64,
}

/// 画面提示轨项（标题、重点高亮等）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditPlanOverlay {
    pub id: String,
    pub clip_id: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub kind: String,
    pub text: String,
    pub position_normalized: Option<(f64, f64)>,
}

/// AI 解说音轨项（触发起点锚定片段，时间映射到全局工程时间轴）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditPlanNarration {
    pub id: String,
    pub clip_id: String,
    pub start_ms: i64,
    pub text: String,
    pub audio_asset_id: Option<String>,
    pub measured_duration_ms: Option<i64>,
    pub ducking_volume: Option<f64>,
}

/// 完整受控剪辑方案结构。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditPlan {
    pub version: u32,
    pub output_mode: AutomationOutputMode,
    pub title: String,
    pub clips: Vec<EditPlanClip>,
    pub overlays: Vec<EditPlanOverlay>,
    pub narrations: Vec<EditPlanNarration>,
    pub target_aspect_ratio: Option<String>,
}
