use std::path::Path;
use sha2::{Digest, Sha256};

use super::types::SourceManifestItem;
use crate::database::Database;

/// 场次来源清单与输入指纹结构。
#[derive(Debug, Clone)]
pub struct SourceManifest {
    pub items: Vec<SourceManifestItem>,
    pub total_duration_ms: i64,
    pub input_hash: String,
    pub all_sealed: bool,
}

impl SourceManifest {
    /// 检查指定场次的录制段落清单并构建时间映射与指纹。
    ///
    /// 优先检测播放列表的 `#EXT-X-ENDLIST` 封口标记；
    /// 若缺少标记但文件已停止写入（超过 30 秒无修改），自动静默补全封口标记以放行历史录播。
    pub async fn build(
        db: &Database,
        platform: &str,
        room_id: &str,
        parent_id: &str,
        cache_dir: &Path,
    ) -> Result<Option<SourceManifest>, String> {
        let records = db
            .get_archives_by_parent_id_and_platform(platform, room_id, parent_id)
            .await
            .map_err(|e| format!("查询录播分段失败: {e}"))?;

        if records.is_empty() {
            return Ok(None);
        }

        let mut items = Vec::new();
        let mut total_duration_ms = 0i64;
        let mut all_sealed = true;
        let mut hasher = Sha256::new();

        for record in records {
            let seg_dir = cache_dir.join(platform).join(room_id).join(&record.live_id);
            let playlist_path = seg_dir.join("playlist.m3u8");

            let (sealed, duration_ms) = if playlist_path.is_file() {
                let bytes = std::fs::read(&playlist_path).unwrap_or_default();
                let has_endlist = bytes.windows(14).any(|w| w == b"#EXT-X-ENDLIST");
                let dur = (record.length * 1000.0).max(0.0) as i64;

                let sealed = if has_endlist {
                    true
                } else {
                    // 若播放列表缺少结束标记，检查文件是否已停止写入（超过 30 秒无修改）。
                    // 若已停止写入且内容非空，静默补全 #EXT-X-ENDLIST 放行，避免历史录播永久阻塞。
                    let is_inactive = match std::fs::metadata(&playlist_path).and_then(|m| m.modified()) {
                        Ok(mtime) => match std::time::SystemTime::now().duration_since(mtime) {
                            Ok(elapsed) => elapsed >= std::time::Duration::from_secs(30),
                            Err(_) => false,
                        },
                        Err(_) => false,
                    };

                    if is_inactive && !bytes.is_empty() {
                        let mut new_bytes = bytes.clone();
                        if !new_bytes.ends_with(b"\n") {
                            new_bytes.push(b'\n');
                        }
                        new_bytes.extend_from_slice(b"#EXT-X-ENDLIST\n");
                        if let Err(e) = std::fs::write(&playlist_path, &new_bytes) {
                            log::warn!("静默补全播放列表封口标记失败: {:?}, err={}", playlist_path, e);
                            false
                        } else {
                            log::info!("录播分段已停止写入，已静默补全封口标记: {:?}", playlist_path);
                            true
                        }
                    } else {
                        false
                    }
                };

                (sealed, dur)
            } else {
                (false, (record.length * 1000.0).max(0.0) as i64)
            };

            if !sealed {
                all_sealed = false;
            }

            hasher.update(record.live_id.as_bytes());
            hasher.update(record.size.to_le_bytes());
            hasher.update(duration_ms.to_le_bytes());
            hasher.update(&[if sealed { 1 } else { 0 }]);

            items.push(SourceManifestItem {
                live_id: record.live_id.clone(),
                start_time_ms: total_duration_ms,
                duration_ms,
                file_path: playlist_path.to_string_lossy().to_string(),
                sealed,
            });

            total_duration_ms += duration_ms;
        }

        let input_hash = format!("{:x}", hasher.finalize());

        Ok(Some(SourceManifest {
            items,
            total_duration_ms,
            input_hash,
            all_sealed,
        }))
    }
}
