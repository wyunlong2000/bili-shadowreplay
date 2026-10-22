use serde::{Deserialize, Serialize};
use crate::database::Database;
use super::types::SourceManifestItem;

/// 稳定字幕证据条目（毫秒为单位，时间已映射到整场录播全局坐标轴）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceCue {
    /// 全局稳定证据标识（例如 cue_0001）
    pub id: String,
    /// 来源录播切片分段标识
    pub live_id: String,
    /// 相对于整场直播起点的开始时间（毫秒）
    pub start_ms: i64,
    /// 相对于整场直播起点的结束时间（毫秒）
    pub end_ms: i64,
    /// 字幕文本内容
    pub text: String,
}

/// 场次媒体与字幕证据索引库。
#[derive(Debug, Clone, Default)]
pub struct EvidenceIndex {
    /// 按全局毫秒开始时间递增排序的全部字幕证据
    pub cues: Vec<EvidenceCue>,
    /// 录播总时长（毫秒）
    pub total_duration_ms: i64,
}

impl EvidenceIndex {
    /// 优先从 record_summaries 中只读复用已有字幕，构建全局证据索引。
    pub async fn build(
        db: &Database,
        platform: &str,
        room_id: &str,
        manifest_items: &[SourceManifestItem],
    ) -> Result<Self, String> {
        let mut cues = Vec::new();
        let mut total_duration_ms = 0i64;
        let mut cue_counter = 1usize;

        for item in manifest_items {
            total_duration_ms = total_duration_ms.max(item.start_time_ms + item.duration_ms);

            // 优先只读复用已有的 record_summary 字幕缓存
            if let Ok(Some(summary)) = db.get_record_summary(platform, room_id, &item.live_id).await {
                if let Some(ref srt_content) = summary.subtitle_srt {
                    Self::parse_srt_into_cues(
                        srt_content,
                        &item.live_id,
                        item.start_time_ms,
                        &mut cue_counter,
                        &mut cues,
                    );
                }
            }
        }

        // 按全局时间排序
        cues.sort_by_key(|c| c.start_ms);

        Ok(Self {
            cues,
            total_duration_ms,
        })
    }

    /// 解析 SRT 文本并将带有全局时间偏移的条目追加到 cues 列表中。
    pub fn parse_srt_into_cues(
        srt_content: &str,
        live_id: &str,
        start_offset_ms: i64,
        cue_counter: &mut usize,
        out_cues: &mut Vec<EvidenceCue>,
    ) {
        if let Ok(parsed_items) = srtparse::from_str(srt_content) {
            for srt_item in parsed_items {
                let text = srt_item.text.trim().to_string();
                if text.is_empty() {
                    continue;
                }
                let item_start_ms = Self::srt_time_to_millis(&srt_item.start_time);
                let item_end_ms = Self::srt_time_to_millis(&srt_item.end_time);

                let cue = EvidenceCue {
                    id: format!("cue_{:04}", *cue_counter),
                    live_id: live_id.to_string(),
                    start_ms: start_offset_ms + item_start_ms,
                    end_ms: start_offset_ms + item_end_ms,
                    text,
                };
                *cue_counter += 1;
                out_cues.push(cue);
            }
        }
    }

    /// 将 srtparse::Time 转为毫秒。
    pub fn srt_time_to_millis(time: &srtparse::Time) -> i64 {
        ((time.hours * 3600 + time.minutes * 60 + time.seconds) * 1000 + time.milliseconds) as i64
    }

    /// 提取落入指定全局毫秒范围内的全部字幕证据（包含部分重叠）。
    pub fn cues_in_range(&self, start_ms: i64, end_ms: i64) -> Vec<EvidenceCue> {
        self.cues
            .iter()
            .filter(|c| c.end_ms > start_ms && c.start_ms < end_ms)
            .cloned()
            .collect()
    }

    /// 将字幕条目格式化为带证据标识和时间戳的提示文本。
    pub fn format_for_prompt(&self, cues: &[EvidenceCue]) -> String {
        cues.iter()
            .map(|c| {
                format!(
                    "[{}] [{} - {}] {}",
                    c.id,
                    Self::format_ms(c.start_ms),
                    Self::format_ms(c.end_ms),
                    c.text.replace('\n', " ")
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 毫秒时间格式化为 hh:mm:ss 文本。
    pub fn format_ms(ms: i64) -> String {
        let total_secs = (ms.max(0) / 1000) as u64;
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_srt_with_offset() {
        let srt = r#"1
00:00:01,000 --> 00:00:03,500
第一句话

2
00:00:04,000 --> 00:00:06,000
第二句话
"#;
        let mut cues = Vec::new();
        let mut counter = 1;
        EvidenceIndex::parse_srt_into_cues(srt, "live_001", 10_000, &mut counter, &mut cues);

        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].id, "cue_0001");
        assert_eq!(cues[0].start_ms, 11_000);
        assert_eq!(cues[0].end_ms, 13_500);
        assert_eq!(cues[0].text, "第一句话");

        assert_eq!(cues[1].id, "cue_0002");
        assert_eq!(cues[1].start_ms, 14_000);
        assert_eq!(cues[1].end_ms, 16_000);
        assert_eq!(cues[1].text, "第二句话");
    }

    #[test]
    fn test_cues_in_range() {
        let cues = vec![
            EvidenceCue {
                id: "cue_0001".into(),
                live_id: "live_1".into(),
                start_ms: 0,
                end_ms: 5000,
                text: "前导".into(),
            },
            EvidenceCue {
                id: "cue_0002".into(),
                live_id: "live_1".into(),
                start_ms: 10_000,
                end_ms: 15_000,
                text: "中间".into(),
            },
            EvidenceCue {
                id: "cue_0003".into(),
                live_id: "live_1".into(),
                start_ms: 20_000,
                end_ms: 25_000,
                text: "末尾".into(),
            },
        ];
        let index = EvidenceIndex {
            cues,
            total_duration_ms: 30_000,
        };

        let in_range = index.cues_in_range(8000, 18_000);
        assert_eq!(in_range.len(), 1);
        assert_eq!(in_range[0].id, "cue_0002");
    }
}
