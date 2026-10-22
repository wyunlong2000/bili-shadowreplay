//! 后端结构化字幕与样式 ASS 编译器。
//!
//! 将字幕片段、样式与控制字符编译为 1920×1080 标准 V4+ ASS 脚本，
//! 严格对齐前端 `src/lib/subtitle-ass.ts` 的坐标、转义与样式字段规范。

use serde::{Deserialize, Serialize};

/// 基础 ASS 字幕条目（秒为单位）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssCue {
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub text: String,
    pub style_name: Option<String>,
    pub layer: Option<i32>,
}

/// 标准 V4+ 样式定义（1920×1080 坐标）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssStyle {
    pub name: String,
    pub font_name: String,
    pub font_size: f64,
    pub primary_colour: String,
    pub secondary_colour: String,
    pub outline_colour: String,
    pub back_colour: String,
    pub bold: i32,
    pub italic: i32,
    pub underline: i32,
    pub strike_out: i32,
    pub scale_x: f64,
    pub scale_y: f64,
    pub spacing: f64,
    pub angle: f64,
    pub border_style: i32,
    pub outline: f64,
    pub shadow: f64,
    pub alignment: i32,
    pub margin_l: i32,
    pub margin_r: i32,
    pub margin_v: i32,
    pub encoding: i32,
}

impl Default for AssStyle {
    /// 默认 1080p 标准白字样式（对应前端 DEFAULT_SUBTITLE_STYLE 换算后数值）。
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            font_name: "Arial".to_string(),
            font_size: 67.5,
            primary_colour: "&H00FFFFFF".to_string(),
            secondary_colour: "&H00FFFFFF".to_string(),
            outline_colour: "&H00000000".to_string(),
            back_colour: "&H80000000".to_string(),
            bold: 0,
            italic: 0,
            underline: 0,
            strike_out: 0,
            scale_x: 100.0,
            scale_y: 100.0,
            spacing: 0.0,
            angle: 0.0,
            border_style: 1,
            outline: 7.5,
            shadow: 0.0,
            alignment: 2, // 底部居中
            margin_l: 100,
            margin_r: 100,
            margin_v: 75,
            encoding: 1,
        }
    }
}

impl AssStyle {
    /// 格式化为标准 Style: 行。
    pub fn format_row(&self) -> String {
        format!(
            "Style: {},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            self.name,
            self.font_name,
            self.font_size,
            self.primary_colour,
            self.secondary_colour,
            self.outline_colour,
            self.back_colour,
            self.bold,
            self.italic,
            self.underline,
            self.strike_out,
            self.scale_x,
            self.scale_y,
            self.spacing,
            self.angle,
            self.border_style,
            self.outline,
            self.shadow,
            self.alignment,
            self.margin_l,
            self.margin_r,
            self.margin_v,
            self.encoding
        )
    }
}

/// 对普通文本转义，防止 ASS 控制字符被意外执行。
pub fn literal_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => {
                result.push('\\');
                result.push('\u{2060}'); // Word joiner 阻断控制转义
            }
            '{' => {
                result.push('\\');
                result.push('{');
            }
            '}' => {
                result.push('\\');
                result.push('}');
            }
            '\r' => {}
            '\n' => {
                result.push_str("\\N");
            }
            // 过滤危险控制字符
            '\u{0000}'..='\u{0008}' | '\u{000b}'..='\u{000c}' | '\u{000e}'..='\u{001f}' | '\u{007f}' => {}
            other => result.push(other),
        }
    }
    result
}

/// 秒数转 ASS 时间戳格式 H:MM:SS.CC (厘秒)。
pub fn ass_time_from_seconds(seconds: f64) -> String {
    let cs = (seconds * 100.0).max(0.0).round() as i64;
    let hours = cs / 360_000;
    let minutes = (cs / 6_000) % 60;
    let secs = (cs / 100) % 60;
    let rem_cs = cs % 100;
    format!("{hours}:{minutes:02}:{secs:02}.{rem_cs:02}")
}

/// 构建完整的 1080p ASS 脚本。
pub fn build_ass_document(cues: &[AssCue], styles: &[AssStyle]) -> String {
    let style_rows = styles
        .iter()
        .map(|s| s.format_row())
        .collect::<Vec<_>>()
        .join("\n");

    let mut events = Vec::new();
    let mut sorted_cues = cues.to_vec();
    sorted_cues.sort_by(|a, b| {
        a.start_seconds
            .partial_cmp(&b.start_seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for cue in sorted_cues {
        if cue.start_seconds < 0.0 || cue.end_seconds <= cue.start_seconds {
            continue;
        }
        let start = ass_time_from_seconds(cue.start_seconds);
        let end = ass_time_from_seconds(cue.end_seconds);
        let style = cue.style_name.as_deref().unwrap_or("Default");
        let layer = cue.layer.unwrap_or(0);
        let text = literal_text(&cue.text);
        events.push(format!(
            "Dialogue: {layer},{start},{end},{style},,0,0,0,,{text}"
        ));
    }

    format!(
        "[Script Info]\nScriptType: v4.00+\nPlayResX: 1920\nPlayResY: 1080\nWrapStyle: 0\nScaledBorderAndShadow: yes\nYCbCr Matrix: None\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n{style_rows}\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n{}\n",
        events.join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_text_escaping() {
        let raw = "测试 \\N 控制符 {test} 与换行\n第二行";
        let escaped = literal_text(raw);
        assert_eq!(escaped, "测试 \\\u{2060}N 控制符 \\{test\\} 与换行\\N第二行");
    }

    #[test]
    fn test_ass_time_formatting() {
        assert_eq!(ass_time_from_seconds(0.0), "0:00:00.00");
        assert_eq!(ass_time_from_seconds(1.5), "0:00:01.50");
        assert_eq!(ass_time_from_seconds(65.123), "0:01:05.12");
        assert_eq!(ass_time_from_seconds(3661.05), "1:01:01.05");
    }

    #[test]
    fn test_build_ass_document() {
        let cues = vec![
            AssCue {
                start_seconds: 1.0,
                end_seconds: 3.5,
                text: "第一句".to_string(),
                style_name: None,
                layer: None,
            },
            AssCue {
                start_seconds: 4.0,
                end_seconds: 6.0,
                text: "第二句".to_string(),
                style_name: Some("Default".to_string()),
                layer: Some(1),
            },
        ];
        let doc = build_ass_document(&cues, &[AssStyle::default()]);

        assert!(doc.contains("[Script Info]"));
        assert!(doc.contains("PlayResX: 1920"));
        assert!(doc.contains("PlayResY: 1080"));
        assert!(doc.contains("Style: Default,Arial,67.5,&H00FFFFFF,&H00FFFFFF,&H00000000,&H80000000,0,0,0,0,100,100,0,0,1,7.5,0,2,100,100,75,1"));
        assert!(doc.contains("Dialogue: 0,0:00:01.00,0:00:03.50,Default,,0,0,0,,第一句"));
        assert!(doc.contains("Dialogue: 1,0:00:04.00,0:00:06.00,Default,,0,0,0,,第二句"));
    }
}
