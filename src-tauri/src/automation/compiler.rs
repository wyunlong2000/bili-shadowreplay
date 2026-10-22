use std::path::Path;
use chrono::{Local, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::evidence::EvidenceCue;
use super::types::{AutomationCandidateRow, AutomationOutputRow, EditPlan, EditPlanClip, EditPlanOverlay};
use crate::database::project::ProjectRow;
use crate::database::video::{MaterialSource, VideoRow};
use crate::events::DataChange;
use crate::project::{Overlay, OverlayContent, OverlayTrack, Timeline, TimelineClip, TimelineSubtitle, TrackKind};

/// 自动化工程与时间轴编译器。
pub struct TimelineCompiler;

impl TimelineCompiler {
    /// 根据流复制关键帧校准规则，从候选与生成的素材编译标准 Timeline V1 工程。
    pub fn compile_timeline_v1(
        candidate: &AutomationCandidateRow,
        material: &VideoRow,
        cues: &[EvidenceCue],
    ) -> (Timeline, EditPlan) {
        let req_start_sec = (candidate.start_ms as f64) / 1000.0;
        let req_duration_sec = ((candidate.end_ms - candidate.start_ms) as f64) / 1000.0;

        // 从 MaterialSource 读取流复制实际录播时间起点 actual_start
        let actual_start_sec = if !material.source.is_empty() {
            match serde_json::from_str::<MaterialSource>(&material.source) {
                Ok(source) => source
                    .ranges
                    .first()
                    .map(|r| r.start)
                    .unwrap_or(req_start_sec),
                Err(_) => req_start_sec,
            }
        } else {
            req_start_sec
        };

        // 关键帧校准：clip.start 为请求开始相对于素材开头的偏移
        let clip_in_sec = (req_start_sec - actual_start_sec).max(0.0);
        let clip_out_sec = clip_in_sec + req_duration_sec;
        let clip_id = format!("clip_{}", Uuid::new_v4().simple());

        let clip = TimelineClip {
            id: clip_id.clone(),
            material_id: material.id,
            start: clip_in_sec,
            end: clip_out_sec,
            volume: 1.0,
        };

        // 依据字幕时间校准：material_time = source_time - actual_start
        let mut subtitles = Vec::new();
        for cue in cues {
            let cue_start_sec = (cue.start_ms as f64) / 1000.0;
            let cue_end_sec = (cue.end_ms as f64) / 1000.0;

            // 过滤超出请求区间的字幕
            if cue_end_sec <= req_start_sec || cue_start_sec >= req_start_sec + req_duration_sec {
                continue;
            }

            let sub_start_sec = (cue_start_sec - actual_start_sec).max(0.0);
            let sub_end_sec = (cue_end_sec - actual_start_sec).max(sub_start_sec);

            subtitles.push(TimelineSubtitle {
                id: format!("sub_{}", Uuid::new_v4().simple()),
                clip_id: clip_id.clone(),
                start: sub_start_sec,
                end: sub_end_sec,
                text: cue.text.clone(),
            });
        }

        let timeline = Timeline {
            version: 1,
            clips: vec![clip],
            subtitles,
            tracks: Vec::new(),
            subtitle_style: None,
            danmu: true,
            audio_tracks: Vec::new(),
        };

        let edit_plan = EditPlan {
            version: 1,
            output_mode: super::types::AutomationOutputMode::Acoustic,
            title: candidate.reason.clone(),
            clips: vec![EditPlanClip {
                id: clip_id,
                live_id: candidate.live_id.clone(),
                source_start_ms: candidate.start_ms,
                source_end_ms: candidate.end_ms,
            }],
            overlays: Vec::new(),
            narrations: Vec::new(),
            target_aspect_ratio: None,
        };

        (timeline, edit_plan)
    }

    /// 编译 SRT 内容并写入素材伴随文件，避免二次触发转写。

    /// 根据流复制关键帧校准与重点提示轨规范，从候选与生成的素材编译标准 Timeline V2 工程。
    pub fn compile_timeline_v2(
        candidate: &AutomationCandidateRow,
        material: &VideoRow,
        cues: &[EvidenceCue],
    ) -> (Timeline, EditPlan) {
        let req_start_sec = (candidate.start_ms as f64) / 1000.0;
        let req_duration_sec = ((candidate.end_ms - candidate.start_ms) as f64) / 1000.0;

        // 从 MaterialSource 读取流复制实际录播时间起点 actual_start
        let actual_start_sec = if !material.source.is_empty() {
            match serde_json::from_str::<MaterialSource>(&material.source) {
                Ok(source) => source
                    .ranges
                    .first()
                    .map(|r| r.start)
                    .unwrap_or(req_start_sec),
                Err(_) => req_start_sec,
            }
        } else {
            req_start_sec
        };

        // 关键帧校准：clip.start 为请求开始相对于素材开头的偏移
        let clip_in_sec = (req_start_sec - actual_start_sec).max(0.0);
        let clip_out_sec = clip_in_sec + req_duration_sec;
        let clip_id = format!("clip_{}", Uuid::new_v4().simple());

        let clip = TimelineClip {
            id: clip_id.clone(),
            material_id: material.id,
            start: clip_in_sec,
            end: clip_out_sec,
            volume: 1.0,
        };

        // 依据字幕时间校准：material_time = source_time - actual_start
        let mut subtitles = Vec::new();
        for cue in cues {
            let cue_start_sec = (cue.start_ms as f64) / 1000.0;
            let cue_end_sec = (cue.end_ms as f64) / 1000.0;

            // 过滤超出请求区间的字幕
            if cue_end_sec <= req_start_sec || cue_start_sec >= req_start_sec + req_duration_sec {
                continue;
            }

            let sub_start_sec = (cue_start_sec - actual_start_sec).max(0.0);
            let sub_end_sec = (cue_end_sec - actual_start_sec).max(sub_start_sec);

            subtitles.push(TimelineSubtitle {
                id: format!("sub_{}", Uuid::new_v4().simple()),
                clip_id: clip_id.clone(),
                start: sub_start_sec,
                end: sub_end_sec,
                text: cue.text.clone(),
            });
        }

        // P1b: 标题与重点提示文字浮层（Title Callout Overlay）
        let title_text = if !candidate.reason.trim().is_empty() {
            let clean = candidate.reason.replace('\n', " ");
            if clean.chars().count() > 24 {
                format!("高光 · {}...", clean.chars().take(22).collect::<String>())
            } else {
                format!("高光 · {}", clean)
            }
        } else {
            "精彩高光瞬间".to_string()
        };

        let overlay_duration = 3.5_f64.min(req_duration_sec.max(1.0));
        let title_element = serde_json::json!({
            "id": 1,
            "kind": "text",
            "content": title_text,
            "x": 960.0,
            "y": 180.0,
            "rotation": 0.0,
            "font": "",
            "fontSize": 48,
            "bold": true,
            "align": "center",
            "fill": "#FFE14D",
            "stroke": "#1A1A1A",
            "strokeWidth": 0.1,
            "background": null,
            "shadow": true,
        });

        let title_overlay = Overlay {
            id: format!("ov_{}", Uuid::new_v4().simple()),
            clip_id: clip_id.clone(),
            start: 0.0,
            duration: overlay_duration,
            content: OverlayContent::Text { element: title_element },
        };

        let overlay_track = OverlayTrack {
            id: format!("tr_{}", Uuid::new_v4().simple()),
            kind: Some(TrackKind::Sticker),
            items: vec![title_overlay],
        };

        let timeline = Timeline {
            version: 2,
            clips: vec![clip],
            subtitles,
            tracks: vec![overlay_track],
            subtitle_style: None,
            danmu: true,
            audio_tracks: Vec::new(),
        };

        let edit_plan = EditPlan {
            version: 2,
            output_mode: super::types::AutomationOutputMode::Acoustic,
            title: candidate.reason.clone(),
            clips: vec![EditPlanClip {
                id: clip_id.clone(),
                live_id: candidate.live_id.clone(),
                source_start_ms: candidate.start_ms,
                source_end_ms: candidate.end_ms,
            }],
            overlays: vec![EditPlanOverlay {
                id: format!("plan_ov_{}", Uuid::new_v4().simple()),
                clip_id,
                start_ms: 0,
                end_ms: (overlay_duration * 1000.0) as i64,
                kind: "title_callout".to_string(),
                text: title_text,
                position_normalized: Some((0.5, 0.167)),
            }],
            narrations: Vec::new(),
            target_aspect_ratio: None,
        };

        (timeline, edit_plan)
    }

    pub async fn write_material_srt(
        output_dir: &Path,
        material_file: &str,
        timeline: &Timeline,
    ) -> Result<(), String> {
        let srt_path = output_dir.join(material_file).with_extension("srt");
        let mut lines = Vec::new();
        for (i, sub) in timeline.subtitles.iter().enumerate() {
            let start_ts = format_srt_time(sub.start);
            let end_ts = format_srt_time(sub.end);
            lines.push(format!("{}\n{} --> {}\n{}\n", i + 1, start_ts, end_ts, sub.text));
        }
        tokio::fs::write(&srt_path, lines.join("\n"))
            .await
            .map_err(|e| format!("写入素材 SRT 失败: {e}"))
    }

    /// 为指定候选自动创建素材并生成 Timeline V1 工程。
    pub async fn process_candidate_acoustic(
        state: &crate::state::State,
        candidate_id: &str,
    ) -> Result<(i64, i64), String> {
        let candidate = state
            .db
            .get_automation_candidate(candidate_id)
            .await
            .map_err(|e| format!("获取候选记录失败: {e}"))?
            .ok_or_else(|| "候选记录不存在".to_string())?;

        let output_dir: std::path::PathBuf = state.config.read().await.output.clone().into();
        let cache_dir: std::path::PathBuf = state.config.read().await.cache.clone().into();

        // 1. 构建 ClipRangeParams
        let req_start_sec = (candidate.start_ms as f64) / 1000.0;
        let req_end_sec = (candidate.end_ms as f64) / 1000.0;

        let params = crate::recorder_manager::ClipRangeParams {
            room_id: candidate.room_id.clone(),
            live_id: candidate.live_id.clone(),
            ranges: vec![crate::ffmpeg::Range {
                start: req_start_sec,
                end: req_end_sec,
            }],
            local_offset: 0,
            title: format!("{}_{}", candidate.content_type, candidate.reason),
            note: format!("自动切片高光（评分: {}）", candidate.score),
            cover: String::new(),
        };

        // 创建临时 ProgressReporter
        let task = state
            .db
            .generate_task(
                "auto_clip_material",
                "processing",
                &serde_json::to_string(&params).unwrap_or_default(),
            )
            .await
            .map_err(|e| format!("创建任务记录失败: {e}"))?;

        let reporter = crate::progress::progress_reporter::ProgressReporter::new(
            state.db.clone(),
            &state.events,
            &task.id,
        )
        .await
        .map_err(|e| format!("创建进度报告器失败: {e}"))?;

        // 2. 调用已有的 create_material（流复制截取）
        let material = crate::handlers::video::create_material(state, &reporter, &params).await?;

        // 3. 构建证据索引并提取落入该区间的字幕
        let manifest_res = super::source::SourceManifest::build(
            &state.db,
            &candidate.platform,
            &candidate.room_id,
            &candidate.parent_id,
            &cache_dir,
        )
        .await
        .unwrap_or(None);

        let manifest_items = manifest_res.map(|m| m.items).unwrap_or_default();
        let evidence_index = super::evidence::EvidenceIndex::build(
            &state.db,
            &candidate.platform,
            &candidate.room_id,
            &manifest_items,
        )
        .await
        .unwrap_or_default();

        let cues = evidence_index.cues_in_range(candidate.start_ms, candidate.end_ms);

        // 4. 编译校准后的 Timeline V1 与 EditPlan
        let (timeline, edit_plan) = Self::compile_timeline_v2(&candidate, &material, &cues);

        // 5. 写入伴随 .srt 文件（避免重复转写）
        let _ = Self::write_material_srt(&output_dir, &material.file, &timeline).await;

        // 6. 添加到 projects 表
        let now = Local::now().to_rfc3339();
        let project = state
            .db
            .add_project(&ProjectRow {
                id: 0,
                room_id: candidate.room_id.clone(),
                platform: candidate.platform.clone(),
                title: material.title.clone(),
                note: material.note.clone(),
                cover: material.cover.clone(),
                timeline: timeline.to_json(),
                profile: String::new(),
                status: 0,
                bvid: String::new(),
                created_at: now.clone(),
                updated_at: now,
            })
            .await
            .map_err(|e| format!("保存自动化工程失败: {e}"))?;

        // 7. 保存自动化产物记录 automation_outputs
        let plan_json = serde_json::to_string(&edit_plan).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(plan_json.as_bytes());
        let plan_hash = format!("{:x}", hasher.finalize());

        let output_row = AutomationOutputRow {
            id: format!("out_{}", Uuid::new_v4().simple()),
            candidate_id: candidate.id.clone(),
            output_mode: "acoustic".to_string(),
            plan_version: 1,
            plan_json,
            plan_hash,
            project_id: Some(project.id),
            material_id: Some(material.id),
            assets_json: "{}".to_string(),
            render_task_id: None,
            status: "success".to_string(),
            error_code: None,
            error_message: None,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        };

        state
            .db
            .save_automation_output(&output_row)
            .await
            .map_err(|e| format!("保存自动化成片记录失败: {e}"))?;

        // 8. 更新候选状态为 completed
        let _ = state
            .db
            .update_candidate_status(&candidate.id, "completed", Some(material.id))
            .await;

        state.db.notify_change(DataChange::Automation);
        state.db.notify_change(DataChange::Videos);
        state.db.notify_change(DataChange::Projects);

        Ok((material.id, project.id))
    }
}

/// 格式化秒数为 SRT 时间戳格式 (hh:mm:ss,ms)。
fn format_srt_time(seconds: f64) -> String {
    let total_ms = (seconds.max(0.0) * 1000.0).round() as u64;
    let ms = total_ms % 1000;
    let total_secs = total_ms / 1000;
    let s = total_secs % 60;
    let m = (total_secs / 60) % 60;
    let h = total_secs / 3600;
    format!("{:02}:{:02}:{:02},{:03}", h, m, s, ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_timeline_keyframe_calibration() {
        // 请求范围：10.0s 到 40.0s (时长 30.0s)
        let candidate = AutomationCandidateRow {
            id: "cand_1".into(),
            run_id: "run_1".into(),
            platform: "bilibili".into(),
            room_id: "123".into(),
            parent_id: "p_1".into(),
            live_id: "l_1".into(),
            start_ms: 10_000,
            end_ms: 40_000,
            score: 88.0,
            signals_json: "{}".into(),
            reason: "极限反杀".into(),
            status: "pending".into(),
            content_type: "gaming".into(),
            confidence: 0.9,
            evidence_json: "{}".into(),
            manual_modified: 0,
            revision: 1,
            clip_task_id: None,
            video_id: None,
            notify_status: "none".into(),
            error_message: None,
            created_at: "".into(),
            updated_at: "".into(),
        };

        // 模拟流复制实际关键帧前导偏移：actual_start = 8.5s
        let material = VideoRow {
            id: 101,
            status: 0,
            room_id: "123".into(),
            created_at: "".into(),
            cover: "".into(),
            file: "clip_101.mp4".into(),
            note: "".into(),
            length: 32,
            size: 1000,
            bvid: "".into(),
            title: "".into(),
            desc: "".into(),
            tags: "".into(),
            area: 0,
            platform: "bilibili".into(),
            source: r#"{"ranges":[{"start":8.5,"end":40.0}],"local_offset":0}"#.into(),
        };

        // 字幕在录播全局时间 12.0s ~ 15.0s
        let cues = vec![EvidenceCue {
            id: "cue_001".into(),
            live_id: "l_1".into(),
            start_ms: 12_000,
            end_ms: 15_000,
            text: "看我操作".into(),
        }];

        let (timeline, plan) = TimelineCompiler::compile_timeline_v1(&candidate, &material, &cues);

        assert_eq!(timeline.version, 1);
        assert_eq!(timeline.clips.len(), 1);

        let clip = &timeline.clips[0];
        // 关键帧校准验证：
        // clip.start 应为 10.0 - 8.5 = 1.5s
        assert!((clip.start - 1.5).abs() < 1e-4);
        // clip.end 应为 1.5 + 30.0 = 31.5s
        assert!((clip.end - 31.5).abs() < 1e-4);

        // 字幕素材相对时间验证：
        // sub.start 应为 12.0 - 8.5 = 3.5s
        // sub.end 应为 15.0 - 8.5 = 6.5s
        assert_eq!(timeline.subtitles.len(), 1);
        let sub = &timeline.subtitles[0];
        assert!((sub.start - 3.5).abs() < 1e-4);
        assert!((sub.end - 6.5).abs() < 1e-4);
        assert_eq!(sub.text, "看我操作");
    }

    #[test]
    fn test_timeline_v2_title_callout_overlay() {
        let candidate = AutomationCandidateRow {
            id: "c_v2".into(),
            run_id: "r_1".into(),
            platform: "bilibili".into(),
            room_id: "123".into(),
            parent_id: "p_1".into(),
            live_id: "l_1".into(),
            start_ms: 10_000,
            end_ms: 30_000,
            score: 88.0,
            reason: "极限反杀连环双杀".into(),
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

        let material = VideoRow {
            id: 102,
            status: 0,
            room_id: "123".into(),
            created_at: "".into(),
            cover: "".into(),
            file: "clip_102.mp4".into(),
            note: "".into(),
            length: 20,
            size: 1000,
            bvid: "".into(),
            title: "".into(),
            desc: "".into(),
            tags: "".into(),
            area: 0,
            platform: "bilibili".into(),
            source: r#"{"ranges":[{"start":10.0,"end":30.0}],"local_offset":0}"#.into(),
        };

        let (timeline, plan) = TimelineCompiler::compile_timeline_v2(&candidate, &material, &[]);

        assert_eq!(timeline.version, 2);
        assert_eq!(timeline.tracks.len(), 1);
        let track = &timeline.tracks[0];
        assert_eq!(track.kind, Some(TrackKind::Sticker));
        assert_eq!(track.items.len(), 1);
        let overlay = &track.items[0];
        assert_eq!(overlay.start, 0.0);
        assert_eq!(overlay.duration, 3.5);

        assert_eq!(plan.version, 2);
        assert_eq!(plan.overlays.len(), 1);
        assert_eq!(plan.overlays[0].kind, "title_callout");
        assert!(plan.overlays[0].text.contains("极限反杀"));
    }
}
