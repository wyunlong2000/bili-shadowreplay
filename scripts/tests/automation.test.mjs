import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import vm from "node:vm";
import ts from "typescript";

async function loadTimelineModule() {
  const source = await readFile(new URL("../../src/lib/project-timeline.ts", import.meta.url), "utf8");
  const { outputText } = ts.transpileModule(source, {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
  });
  const module = new vm.SourceTextModule(outputText, { context: vm.createContext({ crypto: globalThis.crypto }) });
  await module.link(() => { throw new Error("Timeline module has unexpected external dependency"); });
  await module.evaluate();
  return module.namespace;
}

const api = await loadTimelineModule();
const plain = (value) => JSON.parse(JSON.stringify(value));

test("Timeline V2 preserves title callout overlays and narration audio tracks on parse", () => {
  const v2Json = JSON.stringify({
    version: 2,
    clips: [
      { id: "clip_1", material_id: 101, start: 1.5, end: 31.5, volume: 1.0 },
    ],
    subtitles: [
      { id: "sub_1", clip_id: "clip_1", start: 3.5, end: 6.5, text: "极限反杀" },
    ],
    tracks: [
      {
        id: "tr_1",
        kind: "sticker",
        items: [
          {
            id: "ov_1",
            clip_id: "clip_1",
            start: 0.0,
            duration: 3.5,
            kind: "text",
            element: {
              id: 1,
              kind: "text",
              content: "高光 · 极限反杀连环双杀",
              x: 960,
              y: 180,
              rotation: 0,
              font: "",
              fontSize: 48,
              bold: true,
              align: "center",
              fill: "#FFE14D",
              stroke: "#1A1A1A",
              strokeWidth: 0.1,
              background: null,
              shadow: true,
            },
          },
        ],
      },
    ],
    audio_tracks: [
      {
        id: "atr_1",
        items: [
          {
            id: "aud_1",
            clip_id: "clip_1",
            start: 2.0,
            duration: 3.2,
            kind: "file",
            audio_id: 501,
            source_start: 0.0,
            volume: 1.0,
            fade_in: 0.1,
            fade_out: 0.1,
          },
        ],
      },
    ],
  });

  const parsed = api.parseTimeline(v2Json);
  assert.equal(parsed.version, 2);
  assert.equal(parsed.clips.length, 1);
  assert.equal(parsed.clips[0].start, 1.5);
  assert.equal(parsed.clips[0].end, 31.5);

  // 标题提示浮层验证
  assert.equal(parsed.tracks.length, 1);
  assert.equal(parsed.tracks[0].items.length, 1);
  const overlay = parsed.tracks[0].items[0];
  assert.equal(overlay.kind, "text");
  assert.equal(overlay.start, 0.0);
  assert.equal(overlay.duration, 3.5);
  assert.equal(overlay.element.fill, "#FFE14D");
  assert.equal(overlay.element.content, "高光 · 极限反杀连环双杀");

  // 解说音频轨道验证
  assert.equal(parsed.audio_tracks.length, 1);
  assert.equal(parsed.audio_tracks[0].items.length, 1);
  const audio = parsed.audio_tracks[0].items[0];
  assert.equal(audio.kind, "file");
  assert.equal(audio.audio_id, 501);
  assert.equal(audio.duration, 3.2);
});

test("Keyframe offset calibration maps requested time to material-relative time accurately", () => {
  // 请求范围：[10.0s, 40.0s], 时长 30.0s
  // 关键帧流复制实际起始时间 actual_start = 8.5s
  const reqStart = 10.0;
  const reqDuration = 30.0;
  const actualStart = 8.5;

  const clipStart = Math.max(0.0, reqStart - actualStart);
  const clipEnd = clipStart + reqDuration;

  assert.equal(clipStart, 1.5);
  assert.equal(clipEnd, 31.5);

  // 证据字幕转换
  const cueSourceStart = 12.0;
  const cueSourceEnd = 15.0;

  const subStart = Math.max(0.0, cueSourceStart - actualStart);
  const subEnd = Math.max(subStart, cueSourceEnd - actualStart);

  assert.equal(subStart, 3.5);
  assert.equal(subEnd, 6.5);
});

test("Cross-cut narration timing projects accurately across split clips (L-cut support)", () => {
  // 一个持续 20 秒的 clip，解说在第 2 秒触发，持续 8 秒
  const baseTimeline = {
    ...api.emptyTimeline(),
    version: 2,
    clips: [{ id: "c1", material_id: 1, start: 0, end: 20 }],
    audio_tracks: [
      {
        id: "at1",
        items: [
          {
            id: "nar1",
            clip_id: "c1",
            start: 2.0,
            duration: 8.0,
            kind: "file",
            audio_id: 10,
            source_start: 0,
            volume: 1.0,
          },
        ],
      },
    ],
  };

  assert.equal(api.timelineDuration(baseTimeline), 20);

  // 在第 5 秒进行分割
  const { timeline: splitTimeline, clipId: newClipId } = api.splitClip(baseTimeline, 5);
  assert.equal(splitTimeline.clips.length, 2);
  assert.equal(splitTimeline.clips[0].start, 0);
  assert.equal(splitTimeline.clips[0].end, 5);
  assert.equal(splitTimeline.clips[1].id, newClipId);
  assert.equal(splitTimeline.clips[1].start, 5);
  assert.equal(splitTimeline.clips[1].end, 20);

  // 解说的触发起点是 2.0 < 5.0，因此归属在左侧 clip，但其持续时间 8 秒仍自然跨越分割线
  const track = splitTimeline.audio_tracks[0];
  assert.equal(track.items.length, 1);
  assert.equal(track.items[0].clip_id, "c1");
  assert.equal(track.items[0].start, 2.0);
  assert.equal(track.items[0].duration, 8.0);
});

test("Candidate IoU deduplication and top-k filtering logic", () => {
  function timeIoU(aStart, aEnd, bStart, bEnd) {
    const interStart = Math.max(aStart, bStart);
    const interEnd = Math.min(aEnd, bEnd);
    const inter = Math.max(0, interEnd - interStart);
    const union = (aEnd - aStart) + (bEnd - bStart) - inter;
    return union > 0 ? inter / union : 0;
  }

  // 候选 1 与候选 2 重叠率高（80%）
  const iouHigh = timeIoU(10, 40, 15, 42); // inter = 25, union = 30 + 27 - 25 = 32, iou = 25/32 ≈ 0.78 > 0.4
  assert.ok(iouHigh > 0.7);

  // 候选 1 与候选 3 无重叠
  const iouZero = timeIoU(10, 40, 50, 70);
  assert.equal(iouZero, 0);

  // 模拟去重：高分保留，低分去重
  const rawList = [
    { id: "c1", score: 85.0, start: 10, end: 40 },
    { id: "c2", score: 65.0, start: 15, end: 42 }, // 重叠，得分低，应被剔除
    { id: "c3", score: 90.0, start: 50, end: 70 }, // 不重叠，高分保留
    { id: "c4", score: 40.0, start: 80, end: 100 }, // 分数低于 60，过滤
  ];

  const sorted = rawList.filter((c) => c.score >= 60.0).sort((a, b) => b.score - a.score);
  const accepted = [];
  for (const c of sorted) {
    const duplicate = accepted.some((acc) => timeIoU(acc.start, acc.end, c.start, c.end) >= 0.4);
    if (!duplicate) {
      accepted.push(c);
    }
  }

  assert.equal(accepted.length, 2);
  assert.equal(accepted[0].id, "c3"); // 90分
  assert.equal(accepted[1].id, "c1"); // 85分
});
