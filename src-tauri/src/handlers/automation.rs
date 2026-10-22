use serde::{Deserialize, Serialize};

use crate::automation::policy::{AutoClippingPolicy, RoomPolicyOverride};
use crate::automation::types::{AutomationCandidateRow, AutomationOutputRow, AutomationRunRow};
use crate::automation::TimelineCompiler;
use crate::events::DataChange;
use crate::state::State;
use crate::state_type;

#[cfg(feature = "gui")]
use tauri::State as TauriState;

/// 查询自动化策略请求参数。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GetAutomationPolicyRequest {
    pub platform: Option<String>,
    pub room_id: Option<String>,
}

/// 查询自动化策略响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAutomationPolicyResponse {
    pub policy: AutoClippingPolicy,
    pub inherited: bool,
    pub revision: i64,
    pub can_write: bool,
}

/// 保存自动化策略请求参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveAutomationPolicyRequest {
    pub platform: Option<String>,
    pub room_id: Option<String>,
    pub expected_revision: Option<i64>,
    pub policy: AutoClippingPolicy,
    #[serde(default)]
    pub clear_override: Option<bool>,
}

/// 发起场次自动化分析请求参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartAutomationAnalysisRequest {
    pub platform: String,
    pub room_id: String,
    pub parent_id: String,
    pub live_id: String,
}

/// 查询场次运行记录请求参数。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GetAutomationRunsRequest {
    pub platform: Option<String>,
    pub room_id: Option<String>,
    pub limit: Option<i64>,
}

/// 查询候选片段请求参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAutomationCandidatesRequest {
    pub run_id: String,
}

/// 查询成片输出记录请求参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAutomationOutputsRequest {
    pub candidate_id: String,
}

/// 修改候选片段请求参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAutomationCandidateRequest {
    pub candidate_id: String,
    pub expected_revision: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    pub content_type: String,
}

/// 导出候选到工程请求参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateCandidateProjectRequest {
    pub candidate_id: String,
    #[serde(default)]
    pub output_mode: Option<String>,
}

/// 导出候选到工程响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateCandidateProjectResponse {
    pub material_id: i64,
    pub project_id: i64,
}

/// 重试单版本成片输出请求参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryAutomationOutputRequest {
    pub output_id: String,
}

/// 取消运行请求参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelAutomationRunRequest {
    pub run_id: String,
}

/// 获取全局或房间覆盖自动化策略。
#[cfg_attr(feature = "gui", tauri::command)]
pub async fn get_automation_policy(
    state: state_type!(),
    request: GetAutomationPolicyRequest,
) -> Result<GetAutomationPolicyResponse, String> {
    #[cfg(feature = "headless")]
    let can_write = !state.readonly;
    #[cfg(not(feature = "headless"))]
    let can_write = true;

    let global = state.config.read().await.auto_clipping.clone();
    let base_policy = AutoClippingPolicy {
        enabled: global.enabled,
        content_type: global.content_type,
        auto_classify: global.auto_classify,
        output_mode: global.output_mode,
        window_seconds: global.window_seconds,
        overlap_seconds: global.overlap_seconds,
        max_candidates: global.max_candidates,
        min_score: global.min_score,
    };

    if let (Some(platform), Some(room_id)) = (request.platform, request.room_id) {
        if let Ok(Some(row)) = state.db.get_room_automation_policy(&platform, &room_id).await {
            let room_override: Option<RoomPolicyOverride> = serde_json::from_str(&row.policy_json).ok();
            let effective = base_policy.merge_room_override(room_override.as_ref());
            return Ok(GetAutomationPolicyResponse {
                policy: effective,
                inherited: false,
                revision: row.revision,
                can_write,
            });
        }
    }

    Ok(GetAutomationPolicyResponse {
        policy: base_policy,
        inherited: true,
        revision: 0,
        can_write,
    })
}

/// 保存全局或指定房间的自动化策略。
#[cfg_attr(feature = "gui", tauri::command)]
pub async fn save_automation_policy(
    state: state_type!(),
    request: SaveAutomationPolicyRequest,
) -> Result<(), String> {
    #[cfg(feature = "headless")]
    if state.readonly {
        return Err("当前处于只读模式，无法修改策略".to_string());
    }

    request.policy.validate()?;

    if let (Some(platform), Some(room_id)) = (request.platform, request.room_id) {
        if request.clear_override.unwrap_or(false) {
            state
                .db
                .delete_room_automation_policy(&platform, &room_id)
                .await
                .map_err(|e| format!("清除房间策略失败: {e}"))?;
        } else {
            let room_override = RoomPolicyOverride {
                enabled: Some(request.policy.enabled),
                content_type: Some(request.policy.content_type),
                auto_classify: Some(request.policy.auto_classify),
                output_mode: Some(request.policy.output_mode),
                max_candidates: Some(request.policy.max_candidates),
                min_score: Some(request.policy.min_score),
            };
            let policy_json = serde_json::to_string(&room_override).unwrap_or_default();
            let expected_revision = request.expected_revision;

            state
                .db
                .save_room_automation_policy(&platform, &room_id, &policy_json, expected_revision)
                .await
                .map_err(|e| format!("保存房间策略失败: {e}"))?;
        }
    } else {
        let mut cfg = state.config.write().await;
        cfg.auto_clipping.enabled = request.policy.enabled;
        cfg.auto_clipping.content_type = request.policy.content_type;
        cfg.auto_clipping.auto_classify = request.policy.auto_classify;
        cfg.auto_clipping.output_mode = request.policy.output_mode;
        cfg.auto_clipping.window_seconds = request.policy.window_seconds;
        cfg.auto_clipping.overlap_seconds = request.policy.overlap_seconds;
        cfg.auto_clipping.max_candidates = request.policy.max_candidates;
        cfg.auto_clipping.min_score = request.policy.min_score;
        cfg.save();
    }

    state.db.notify_change(DataChange::Automation);
    Ok(())
}

/// 手动发起录播场次自动化切片分析。
#[cfg_attr(feature = "gui", tauri::command)]
pub async fn start_automation_analysis(
    state: state_type!(),
    request: StartAutomationAnalysisRequest,
) -> Result<String, String> {
    #[cfg(feature = "headless")]
    if state.readonly {
        return Err("当前处于只读模式，无法发起分析任务".to_string());
    }

    state
        .automation
        .enqueue_manual_run(
            &request.platform,
            &request.room_id,
            &request.parent_id,
            &request.live_id,
        )
        .await
}

/// 查询自动化场次运行记录列表。
#[cfg_attr(feature = "gui", tauri::command)]
pub async fn get_automation_runs(
    state: state_type!(),
    request: GetAutomationRunsRequest,
) -> Result<Vec<AutomationRunRow>, String> {
    let limit = request.limit.unwrap_or(20);
    state
        .db
        .get_automation_runs(
            request.platform.as_deref(),
            request.room_id.as_deref(),
            limit,
        )
        .await
        .map_err(|e| format!("查询运行记录失败: {e}"))
}

/// 查询某次运行下的全部候选高光片段。
#[cfg_attr(feature = "gui", tauri::command)]
pub async fn get_automation_candidates(
    state: state_type!(),
    request: GetAutomationCandidatesRequest,
) -> Result<Vec<AutomationCandidateRow>, String> {
    state
        .db
        .get_candidates_by_run(&request.run_id)
        .await
        .map_err(|e| format!("查询候选失败: {e}"))
}

/// 查询候选对应的成片输出记录。
#[cfg_attr(feature = "gui", tauri::command)]
pub async fn get_automation_outputs(
    state: state_type!(),
    request: GetAutomationOutputsRequest,
) -> Result<Vec<AutomationOutputRow>, String> {
    state
        .db
        .get_outputs_by_candidate(&request.candidate_id)
        .await
        .map_err(|e| format!("查询产物失败: {e}"))
}

/// 用户手动修改候选片段边界与分类。
#[cfg_attr(feature = "gui", tauri::command)]
pub async fn update_automation_candidate(
    state: state_type!(),
    request: UpdateAutomationCandidateRequest,
) -> Result<bool, String> {
    #[cfg(feature = "headless")]
    if state.readonly {
        return Err("当前处于只读模式，无法修改候选".to_string());
    }

    let updated = state
        .db
        .update_candidate_manual(
            &request.candidate_id,
            request.expected_revision,
            request.start_ms,
            request.end_ms,
            &request.content_type,
        )
        .await
        .map_err(|e| format!("更新候选失败: {e}"))?;

    if updated {
        state.db.notify_change(DataChange::Automation);
    }

    Ok(updated)
}

/// 从候选片段自动截取素材并生成对应模式的 Timeline 工程（原声版或解说版）。
#[cfg_attr(feature = "gui", tauri::command)]
pub async fn generate_candidate_project(
    state: state_type!(),
    request: GenerateCandidateProjectRequest,
) -> Result<GenerateCandidateProjectResponse, String> {
    #[cfg(feature = "headless")]
    if state.readonly {
        return Err("当前处于只读模式，无法创建工程".to_string());
    }

    let is_commentary = request.output_mode.as_deref() == Some("commentary");
    let (material_id, project_id) = if is_commentary {
        TimelineCompiler::process_candidate_commentary(&state, &request.candidate_id).await?
    } else {
        TimelineCompiler::process_candidate_acoustic(&state, &request.candidate_id).await?
    };

    Ok(GenerateCandidateProjectResponse {
        material_id,
        project_id,
    })
}

/// 重试单版本成片输出。
#[cfg_attr(feature = "gui", tauri::command)]
pub async fn retry_automation_output(
    state: state_type!(),
    request: RetryAutomationOutputRequest,
) -> Result<GenerateCandidateProjectResponse, String> {
    #[cfg(feature = "headless")]
    if state.readonly {
        return Err("当前处于只读模式，无法重试成片输出".to_string());
    }

    let output = state
        .db
        .get_automation_output(&request.output_id)
        .await
        .map_err(|e| format!("查询成片输出记录失败: {e}"))?
        .ok_or_else(|| "成片输出记录不存在".to_string())?;

    let is_commentary = output.output_mode == "commentary";
    let (material_id, project_id) = if is_commentary {
        TimelineCompiler::process_candidate_commentary(&state, &output.candidate_id).await?
    } else {
        TimelineCompiler::process_candidate_acoustic(&state, &output.candidate_id).await?
    };

    Ok(GenerateCandidateProjectResponse {
        material_id,
        project_id,
    })
}

/// 取消自动化场次分析任务。
#[cfg_attr(feature = "gui", tauri::command)]
pub async fn cancel_automation_run(
    state: state_type!(),
    request: CancelAutomationRunRequest,
) -> Result<(), String> {
    #[cfg(feature = "headless")]
    if state.readonly {
        return Err("当前处于只读模式，无法取消任务".to_string());
    }

    state.automation.cancel_run(&request.run_id).await
}
