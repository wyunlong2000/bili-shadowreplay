use std::sync::Arc;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::evidence::{EvidenceCue, EvidenceIndex};
use super::policy::AutoClippingPolicy;
use super::scoring::{CandidateRanker, RawHighlightCandidate};
use super::types::{AutomationAnalysisRow, AutomationCandidateRow, SourceManifestItem};
use crate::config::Config;
use crate::database::Database;
use crate::events::DataChange;

/// 单个滑窗定义（毫秒区间）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlidingWindow {
    pub index: usize,
    pub start_ms: i64,
    pub end_ms: i64,
}

/// 计算整场录播的时间滑窗。
pub fn compute_sliding_windows(
    total_duration_ms: i64,
    window_seconds: i64,
    overlap_seconds: i64,
) -> Vec<SlidingWindow> {
    if total_duration_ms <= 0 {
        return Vec::new();
    }

    let window_size_ms = (window_seconds * 1000).max(10_000);
    let overlap_ms = (overlap_seconds * 1000).min(window_size_ms - 5000).max(0);
    let step_ms = (window_size_ms - overlap_ms).max(5000);

    let mut windows = Vec::new();
    let mut start_ms = 0i64;
    let mut index = 0usize;

    while start_ms < total_duration_ms {
        let end_ms = (start_ms + window_size_ms).min(total_duration_ms);
        windows.push(SlidingWindow {
            index,
            start_ms,
            end_ms,
        });
        index += 1;
        if end_ms >= total_duration_ms {
            break;
        }
        start_ms += step_ms;
    }

    windows
}

const ANALYSIS_PREAMBLE: &str = r#"
你是一名专业中文直播高光切片剪辑师。请根据提供的带时间戳的字幕和证据ID，识别该时间窗口内最精彩、最具传播或讨论价值的高光片段。
要求：
1. 提取 0 到 3 个真正具备内容价值的高光片段。若该时间段内无精彩内容，返回空列表。
2. 每个片段的时间必须有充分的字幕证据支持，时间单位为秒。
3. 必须在 evidence_cue_ids 中列出作为依据的字幕 cue_id 列表。
4. 给出吸引人的标题（title）、评分（score: 0-100）和推荐理由（reason）。
5. 输出必须是合法 JSON，格式严格如下：
{"candidates":[{"title":"片段标题","start_seconds":12.5,"end_seconds":45.0,"score":88.0,"reason":"推荐理由","evidence_cue_ids":["cue_0001"]}]}
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmHighlightItem {
    pub title: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub score: f64,
    pub reason: String,
    #[serde(default)]
    pub evidence_cue_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmAnalysisResponse {
    #[serde(default)]
    pub candidates: Vec<LlmHighlightItem>,
}

/// 启发式窗口分析器（在离线或无 LLM Key 时的安全回退实现）。
pub fn heuristic_analyze_window(
    window: &SlidingWindow,
    cues: &[EvidenceCue],
    policy: &AutoClippingPolicy,
) -> Vec<RawHighlightCandidate> {
    if cues.is_empty() {
        return Vec::new();
    }

    // 常见直播高光与热烈情绪关键词
    const HIGHLIGHT_KEYWORDS: &[&str] = &[
        "哈哈", "笑死", "牛逼", "卧槽", "太强", "太帅", "极限", "反杀", "单杀", "翻盘",
        "绝了", "不会吧", "好活", "救命", "神了", "封神", "无敌", "泪目", "起飞", "可以的",
    ];

    let mut hit_cues = Vec::new();
    for cue in cues {
        for kw in HIGHLIGHT_KEYWORDS {
            if cue.text.contains(kw) {
                hit_cues.push(cue);
                break;
            }
        }
    }

    if hit_cues.is_empty() {
        // 如果没有命中关键词，检查是否语句密集（例如 30 秒内有超过 4 句话）
        if cues.len() >= 4 {
            let start = cues.first().map(|c| c.start_ms).unwrap_or(window.start_ms);
            let end = cues.last().map(|c| c.end_ms).unwrap_or(window.end_ms);
            let cue_ids = cues.iter().map(|c| c.id.clone()).collect();
            return vec![RawHighlightCandidate {
                title: format!("{}日常互动", policy.content_type.as_str()),
                start_ms: start,
                end_ms: end,
                score: 65.0,
                reason: "密集交流讨论看点".to_string(),
                content_type: Some(policy.content_type.as_str().to_string()),
                confidence: Some(0.7),
                evidence_cue_ids: cue_ids,
            }];
        }
        return Vec::new();
    }

    // 有关键词命中，按命中的字幕范围扩张
    let first_hit = hit_cues[0];
    let last_hit = hit_cues[hit_cues.len() - 1];

    // 向前向后适度容纳上下文（前后各 3 秒）
    let start_ms = (first_hit.start_ms - 3000).max(window.start_ms);
    let end_ms = (last_hit.end_ms + 3000).min(window.end_ms);

    let mut cue_ids = Vec::new();
    for c in cues {
        if c.end_ms >= start_ms && c.start_ms <= end_ms {
            cue_ids.push(c.id.clone());
        }
    }

    let score = (70.0 + (hit_cues.len() as f64 * 5.0)).min(95.0);
    let title = format!(
        "{}高光：{}",
        policy.content_type.as_str(),
        first_hit.text.chars().take(12).collect::<String>()
    );

    vec![RawHighlightCandidate {
        title,
        start_ms,
        end_ms,
        score,
        reason: format!("检测到高光热点关键词，共命中 {} 处情绪点", hit_cues.len()),
        content_type: Some(policy.content_type.as_str().to_string()),
        confidence: Some(0.85),
        evidence_cue_ids: cue_ids,
    }]
}

/// 执行单窗口分析：优先调用 LLM，若未配置或失败则降级到启发式。
pub async fn analyze_window(
    window: &SlidingWindow,
    cues: &[EvidenceCue],
    policy: &AutoClippingPolicy,
    evidence_index: &EvidenceIndex,
    config: &Config,
) -> Vec<RawHighlightCandidate> {
    if cues.is_empty() {
        return Vec::new();
    }

    // 若配置了有效的 LLM 提供商且模型名称非空，尝试 LLM
    if !config.llm.model.trim().is_empty() {
        let transcript = evidence_index.format_for_prompt(cues);
        let prompt = format!(
            "直播内容类型偏好: {}\n当前分析时间窗口: [{} - {}] (全局时间)\n\n字幕证据条目:\n{}\n\n请识别并提取高光候选JSON:",
            policy.content_type.as_str(),
            EvidenceIndex::format_ms(window.start_ms),
            EvidenceIndex::format_ms(window.end_ms),
            transcript
        );

        if let Ok(res_str) = crate::agent::llm_prompt(&config.llm, ANALYSIS_PREAMBLE, &prompt).await {
            if let Ok(parsed) = serde_json::from_str::<LlmAnalysisResponse>(&res_str) {
                let mut candidates = Vec::new();
                for item in parsed.candidates {
                    // 判断 start_seconds 是相对秒数还是绝对秒数
                    let mut s_ms = (item.start_seconds * 1000.0).round() as i64;
                    let mut e_ms = (item.end_seconds * 1000.0).round() as i64;

                    // 若模型输出了相对窗口开头的秒数，转换为全局毫秒
                    if s_ms < window.start_ms && s_ms < (window.end_ms - window.start_ms) {
                        s_ms += window.start_ms;
                        e_ms += window.start_ms;
                    }

                    // 边界约束
                    let s_ms = s_ms.max(window.start_ms);
                    let e_ms = e_ms.min(window.end_ms);

                    if e_ms > s_ms {
                        candidates.push(RawHighlightCandidate {
                            title: item.title,
                            start_ms: s_ms,
                            end_ms: e_ms,
                            score: item.score,
                            reason: item.reason,
                            content_type: Some(policy.content_type.as_str().to_string()),
                            confidence: Some(0.9),
                            evidence_cue_ids: item.evidence_cue_ids,
                        });
                    }
                }
                if !candidates.is_empty() {
                    return candidates;
                }
            }
        }
    }

    // 回退到启发式分析
    heuristic_analyze_window(window, cues, policy)
}

/// 执行整场自动化分析流水线。
pub async fn run_analysis_pipeline(
    db: &Arc<Database>,
    config: &Config,
    run_id: &str,
    platform: &str,
    room_id: &str,
    parent_id: &str,
    live_id: &str,
    manifest_items: &[SourceManifestItem],
    policy: &AutoClippingPolicy,
) -> Result<Vec<AutomationCandidateRow>, String> {
    log::info!("开始构建媒体字幕证据索引: run_id={run_id}");
    let evidence_index = EvidenceIndex::build(db, platform, room_id, manifest_items).await?;
    log::info!(
        "已加载 {} 条有效字幕证据，总时长 {} ms",
        evidence_index.cues.len(),
        evidence_index.total_duration_ms
    );

    let windows = compute_sliding_windows(
        evidence_index.total_duration_ms,
        policy.window_seconds,
        policy.overlap_seconds,
    );
    let total_windows = windows.len() as i64;

    log::info!("切分完成，共 {total_windows} 个滑窗分析单元");
    let _ = db
        .update_automation_run_progress(run_id, 0, total_windows)
        .await;

    let mut all_raw_candidates = Vec::new();

    for (idx, win) in windows.iter().enumerate() {
        let cues = evidence_index.cues_in_range(win.start_ms, win.end_ms);
        let win_candidates =
            analyze_window(win, &cues, policy, &evidence_index, config).await;

        let checkpoints_json = serde_json::to_string(&win_candidates).unwrap_or_default();
        let analysis_row = AutomationAnalysisRow {
            id: format!("ana_{}_{}", run_id, win.index),
            run_id: run_id.to_string(),
            platform: platform.to_string(),
            room_id: room_id.to_string(),
            live_id: live_id.to_string(),
            parent_id: parent_id.to_string(),
            status: "completed".to_string(),
            stage: "analyzed".to_string(),
            processed_until_ms: win.end_ms,
            window_seconds: policy.window_seconds,
            subtitle_json: None,
            checkpoints_json: Some(checkpoints_json),
            input_hash: String::new(),
            task_id: None,
            error_message: None,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        };

        let _ = db.save_automation_analysis(&analysis_row).await;
        all_raw_candidates.extend(win_candidates);

        let _ = db
            .update_automation_run_progress(run_id, (idx + 1) as i64, total_windows)
            .await;
    }

    log::info!(
        "所有滑窗分析完成，累计获取 {} 个原始高光候选，开始跨窗口去重与评分复选",
        all_raw_candidates.len()
    );

    let final_candidates = CandidateRanker::deduplicate_and_rank(
        all_raw_candidates,
        policy,
        run_id,
        platform,
        room_id,
        parent_id,
        live_id,
    );

    log::info!(
        "复选完成，最终保留 {} 个优质高光候选，持久化入库",
        final_candidates.len()
    );

    if !final_candidates.is_empty() {
        db.create_automation_candidates(&final_candidates)
            .await
            .map_err(|e| format!("保存自动化候选失败: {e}"))?;
    }

    let _ = db
        .update_automation_run_status(run_id, "pending", "candidate_review", None, None)
        .await;

    db.notify_change(DataChange::Automation);

    Ok(final_candidates)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_sliding_windows() {
        let windows = compute_sliding_windows(600_000, 300, 30);
        // 600s total:
        // win 0: 0 -> 300,000 (0 -> 300s)
        // step = 270s = 270,000
        // win 1: 270,000 -> 570,000 (270 -> 570s)
        // win 2: 540,000 -> 600,000 (540 -> 600s)
        assert_eq!(windows.len(), 3);
        assert_eq!(windows[0].start_ms, 0);
        assert_eq!(windows[0].end_ms, 300_000);
        assert_eq!(windows[1].start_ms, 270_000);
        assert_eq!(windows[1].end_ms, 570_000);
        assert_eq!(windows[2].start_ms, 540_000);
        assert_eq!(windows[2].end_ms, 600_000);
    }
}
