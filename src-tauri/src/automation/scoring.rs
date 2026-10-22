use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::policy::AutoClippingPolicy;
use super::types::{AutomationCandidateRow, CandidateEvidence};

/// 原始模型或启发式分析返回的高光片段描述。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawHighlightCandidate {
    pub title: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub score: f64,
    pub reason: String,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub evidence_cue_ids: Vec<String>,
}

/// 候选打分、跨窗口去重合并与复选排序器。
pub struct CandidateRanker;

impl CandidateRanker {
    /// 对来自各个滑窗的候选进行时间重叠去重、分数过滤与截断排序。
    pub fn deduplicate_and_rank(
        mut raw_candidates: Vec<RawHighlightCandidate>,
        policy: &AutoClippingPolicy,
        run_id: &str,
        platform: &str,
        room_id: &str,
        parent_id: &str,
        live_id: &str,
    ) -> Vec<AutomationCandidateRow> {
        // 1. 过滤掉不合法区间和低于最低阈值的候选
        raw_candidates.retain(|c| {
            c.start_ms >= 0 && c.end_ms > c.start_ms && c.score >= policy.min_score
        });

        // 2. 按开始时间升序排列以方便区间去重合并
        raw_candidates.sort_by_key(|c| c.start_ms);

        let mut merged: Vec<RawHighlightCandidate> = Vec::new();

        for candidate in raw_candidates {
            if let Some(last) = merged.last_mut() {
                // 计算重叠交集
                let overlap_start = last.start_ms.max(candidate.start_ms);
                let overlap_end = last.end_ms.min(candidate.end_ms);
                let overlap_dur = (overlap_end - overlap_start).max(0);

                let min_dur =
                    (last.end_ms - last.start_ms).min(candidate.end_ms - candidate.start_ms);

                // 若交集超过较短片段的 40% 或起止相距很近（<= 15秒），判定为同一高光事件
                if (min_dur > 0 && overlap_dur as f64 / min_dur as f64 >= 0.4)
                    || (candidate.start_ms - last.start_ms).abs() <= 15_000
                {
                    // 合并证据 ID
                    for cue in candidate.evidence_cue_ids {
                        if !last.evidence_cue_ids.contains(&cue) {
                            last.evidence_cue_ids.push(cue);
                        }
                    }
                    // 保留更高评分者的元数据
                    if candidate.score > last.score {
                        last.score = candidate.score;
                        last.title = candidate.title;
                        last.reason = candidate.reason;
                        if candidate.content_type.is_some() {
                            last.content_type = candidate.content_type;
                        }
                    }
                    // 扩展起止边界为合集
                    last.start_ms = last.start_ms.min(candidate.start_ms);
                    last.end_ms = last.end_ms.max(candidate.end_ms);
                    continue;
                }
            }
            merged.push(candidate);
        }

        // 3. 按最终分数降序排序
        merged.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // 4. 截取至最大候选数量
        let limit = policy.max_candidates.max(1);
        let selected = merged.into_iter().take(limit);

        let now = Utc::now().to_rfc3339();

        // 5. 转换为数据库持久化结构 AutomationCandidateRow
        selected
            .map(|c| {
                let id = format!("cand_{}", Uuid::new_v4().simple());
                let content_type = c
                    .content_type
                    .unwrap_or_else(|| policy.content_type.as_str().to_string());
                let evidence = CandidateEvidence {
                    cue_ids: c.evidence_cue_ids,
                    frame_timestamps_ms: Vec::new(),
                    danmaku_peaks: Vec::new(),
                    notes: c.reason.clone(),
                };
                let evidence_json =
                    serde_json::to_string(&evidence).unwrap_or_else(|_| "{}".to_string());

                AutomationCandidateRow {
                    id,
                    run_id: run_id.to_string(),
                    platform: platform.to_string(),
                    room_id: room_id.to_string(),
                    parent_id: parent_id.to_string(),
                    live_id: live_id.to_string(),
                    start_ms: c.start_ms,
                    end_ms: c.end_ms,
                    score: c.score,
                    signals_json: "{}".to_string(),
                    reason: c.reason,
                    status: "pending".to_string(),
                    content_type,
                    confidence: c.confidence.unwrap_or(0.85),
                    evidence_json,
                    manual_modified: 0,
                    revision: 1,
                    clip_task_id: None,
                    video_id: None,
                    notify_status: "none".to_string(),
                    error_message: None,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation::types::{ClipOutputMode, LiveContentType};

    #[test]
    fn test_deduplicate_and_rank() {
        let policy = AutoClippingPolicy {
            enabled: true,
            content_type: LiveContentType::Gaming,
            auto_classify: true,
            output_mode: ClipOutputMode::Both,
            window_seconds: 300,
            overlap_seconds: 30,
            max_candidates: 2,
            min_score: 60.0,
        };

        let raw = vec![
            RawHighlightCandidate {
                title: "精彩反杀低分".into(),
                start_ms: 10_000,
                end_ms: 40_000,
                score: 65.0,
                reason: "初次识别".into(),
                content_type: None,
                confidence: None,
                evidence_cue_ids: vec!["cue_0001".into()],
            },
            RawHighlightCandidate {
                title: "精彩反杀高分".into(),
                start_ms: 12_000,
                end_ms: 45_000,
                score: 90.0,
                reason: "二次滑窗强化".into(),
                content_type: None,
                confidence: None,
                evidence_cue_ids: vec!["cue_0002".into()],
            },
            RawHighlightCandidate {
                title: "低分淘汰片段".into(),
                start_ms: 60_000,
                end_ms: 90_000,
                score: 50.0, // 低于 60.0
                reason: "淘汰".into(),
                content_type: None,
                confidence: None,
                evidence_cue_ids: vec![],
            },
            RawHighlightCandidate {
                title: "独立看点".into(),
                start_ms: 120_000,
                end_ms: 150_000,
                score: 80.0,
                reason: "团灭敌方".into(),
                content_type: None,
                confidence: None,
                evidence_cue_ids: vec!["cue_0005".into()],
            },
        ];

        let ranked = CandidateRanker::deduplicate_and_rank(
            raw, &policy, "run_1", "bilibili", "123", "p_1", "l_1",
        );

        // 应该只有 2 个（因为 max_candidates = 2，且两个反杀已合并，低分淘汰被过滤）
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].score, 90.0);
        assert_eq!(ranked[0].title, "精彩反杀高分");
        assert_eq!(ranked[0].start_ms, 10_000);
        assert_eq!(ranked[0].end_ms, 45_000);

        let evidence: CandidateEvidence =
            serde_json::from_str(&ranked[0].evidence_json).unwrap();
        assert_eq!(evidence.cue_ids, vec!["cue_0001", "cue_0002"]);

        assert_eq!(ranked[1].score, 80.0);
        assert_eq!(ranked[1].title, "独立看点");
    }
}
