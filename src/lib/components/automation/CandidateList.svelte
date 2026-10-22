<!--
  高光候选卡片列表
  展示某运行场次下的所有高光候选，支持点击选中和状态预览
-->
<script lang="ts">
  import type { AutomationCandidateRow, LiveContentType } from "../../interface";
  import { Sparkles, Clock, ChevronRight } from "lucide-svelte";

  interface Props {
    candidates: AutomationCandidateRow[];
    selectedCandidateId: string | null;
    onSelect: (candidate: AutomationCandidateRow) => void;
  }

  let { candidates = [], selectedCandidateId, onSelect }: Props = $props();

  const typeNames: Record<string, string> = {
    gaming: "游戏高光",
    chat: "聊天互动",
    knowledge: "知识分享",
    sports: "体育赛事",
    commerce: "带货直播",
    mixed: "综合混合",
    other: "其他",
  };

  function formatSec(seconds: number) {
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = Math.floor(seconds % 60);
    if (h > 0) {
      return `${h.toString().padStart(2, "0")}:${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
    }
    return `${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
  }
</script>

<div class="h-full flex flex-col">
  {#if candidates.length === 0}
    <div class="flex-1 flex flex-col items-center justify-center p-8 text-fg-muted space-y-2">
      <Sparkles class="h-8 w-8 text-fg-faint" />
      <span class="text-xs">暂无识别出的高光候选</span>
    </div>
  {:else}
    <div class="flex-1 overflow-y-auto divide-y divide-line custom-scrollbar-light">
      {#each candidates as c (c.id)}
        {@const isSelected = c.id === selectedCandidateId}
        {@const durationSec = Math.max(0, Math.floor((c.end_ms - c.start_ms) / 1000))}
        <button
          type="button"
          class="w-full text-left p-3.5 transition-colors flex items-start gap-3 hover:bg-control-hover/60 {isSelected ? 'bg-control' : ''}"
          onclick={() => onSelect(c)}
        >
          <div class="flex-1 min-w-0 space-y-1.5">
            <div class="flex items-center gap-2">
              <span class="rounded bg-accent/10 px-1.5 py-0.5 text-[11px] font-semibold text-accent border border-accent/20">
                {typeNames[c.content_type] || c.content_type}
              </span>
              <span class="rounded bg-amber-500/10 px-1.5 py-0.5 text-[11px] font-medium text-amber-600 dark:text-amber-400 border border-amber-500/20">
                {c.score.toFixed(0)}分
              </span>
              {#if c.manual_modified}
                <span class="text-[10px] text-blue-500">已微调</span>
              {/if}
            </div>

            <!-- 时间段与时长 -->
            <div class="flex items-center gap-1.5 text-xs text-fg-muted font-mono">
              <Clock class="h-3 w-3 shrink-0" />
              <span>{formatSec(c.start_ms / 1000)} - {formatSec(c.end_ms / 1000)}</span>
              <span class="text-[11px] text-fg-faint">({formatSec(durationSec)})</span>
            </div>

            <!-- 理由截断 -->
            <p class="text-xs text-fg-muted line-clamp-2 leading-relaxed">
              {c.reason || "滑窗高光候选片段"}
            </p>
          </div>

          <ChevronRight class="h-4 w-4 shrink-0 text-fg-faint mt-2 {isSelected ? 'text-accent' : ''}" />
        </button>
      {/each}
    </div>
  {/if}
</div>
