<!--
  高光候选详情与微调面板
  展示选区依据、评分理由，支持起止时间微调与一键生成切片工程
-->
<script lang="ts">
  import { onMount } from "svelte";
  import type { AutomationCandidateRow, AutomationOutputRow, LiveContentType } from "../../interface";
  import {
    updateAutomationCandidate,
    generateCandidateProject,
    getAutomationOutputs,
  } from "../../automation";
  import OutputStatus from "./OutputStatus.svelte";
  import {
    Scissors,
    Sparkles,
    Clock,
    Check,
    AlertCircle,
    Loader2,
    Sliders,
    FolderKanban,
  } from "lucide-svelte";

  interface Props {
    candidate: AutomationCandidateRow | null;
    onUpdated?: () => void;
    onOpenProject?: (projectId: number) => void;
  }

  let { candidate, onUpdated, onOpenProject }: Props = $props();

  let outputs = $state<AutomationOutputRow[]>([]);
  let loadingOutputs = $state(false);

  // 微调编辑状态
  let editContentType = $state<LiveContentType>("gaming");
  let editStartSec = $state(0);
  let editEndSec = $state(0);
  let isSaving = $state(false);
  let isGenerating = $state(false);
  let statusMessage = $state<{ type: "success" | "error"; text: string } | null>(null);

  const contentTypes: { value: LiveContentType; label: string }[] = [
    { value: "gaming", label: "游戏高光" },
    { value: "chat", label: "聊天互动" },
    { value: "knowledge", label: "知识分享" },
    { value: "sports", label: "体育赛事" },
    { value: "commerce", label: "带货直播" },
    { value: "mixed", label: "综合混合" },
    { value: "other", label: "其他内容" },
  ];

  $effect(() => {
    if (candidate) {
      editContentType = candidate.content_type;
      editStartSec = Math.floor(candidate.start_ms / 1000);
      editEndSec = Math.ceil(candidate.end_ms / 1000);
      statusMessage = null;
      loadOutputs(candidate.id);
    } else {
      outputs = [];
    }
  });

  async function loadOutputs(candidateId: string) {
    loadingOutputs = true;
    try {
      outputs = await getAutomationOutputs(candidateId);
    } catch (e) {
      console.error("加载候选成片记录失败:", e);
    } finally {
      loadingOutputs = false;
    }
  }

  /**
   * 保存时间微调与分类变更
   */
  async function handleSaveAdjustment() {
    if (!candidate) return;
    if (editEndSec <= editStartSec) {
      statusMessage = { type: "error", text: "结束时间必须大于开始时间" };
      return;
    }
    isSaving = true;
    statusMessage = null;
    try {
      const ok = await updateAutomationCandidate(
        candidate.id,
        candidate.revision,
        editStartSec * 1000,
        editEndSec * 1000,
        editContentType
      );
      if (ok) {
        statusMessage = { type: "success", text: "微调保存成功" };
        onUpdated?.();
      } else {
        statusMessage = { type: "error", text: "版本冲突或记录不存在，请刷新后重试" };
      }
    } catch (e: any) {
      statusMessage = { type: "error", text: "保存失败: " + (e?.message || e) };
    } finally {
      isSaving = false;
    }
  }

  /**
   * 一键从候选片段生成切片工程 (Timeline V1)
   */
  async function handleGenerateProject() {
    if (!candidate) return;
    isGenerating = true;
    statusMessage = null;
    try {
      const res = await generateCandidateProject(candidate.id);
      statusMessage = { type: "success", text: `切片工程已生成 (工程ID: ${res.project_id})` };
      await loadOutputs(candidate.id);
      onUpdated?.();
      if (onOpenProject) {
        onOpenProject(res.project_id);
      }
    } catch (e: any) {
      statusMessage = { type: "error", text: "生成工程失败: " + (e?.message || e) };
    } finally {
      isGenerating = false;
    }
  }

  function formatTime(seconds: number) {
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = Math.floor(seconds % 60);
    if (h > 0) {
      return `${h.toString().padStart(2, "0")}:${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
    }
    return `${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
  }
</script>

{#if !candidate}
  <div class="h-full flex flex-col items-center justify-center p-8 text-fg-muted text-center space-y-2">
    <Scissors class="h-10 w-10 text-fg-faint" />
    <p class="text-sm">在左侧选择一个高光候选查看详情与微调</p>
  </div>
{:else}
  <div class="space-y-5 p-5">
    <!-- 头部摘要 -->
    <div class="flex items-start justify-between gap-4 border-b border-line pb-4">
      <div>
        <div class="flex items-center gap-2">
          <span class="rounded bg-accent/10 px-2 py-0.5 text-xs font-semibold text-accent border border-accent/20">
            {contentTypes.find((c) => c.value === candidate?.content_type)?.label ?? candidate?.content_type}
          </span>
          <span class="rounded bg-amber-500/10 px-2 py-0.5 text-xs font-medium text-amber-600 dark:text-amber-400 border border-amber-500/20">
            评分: {candidate.score.toFixed(1)}
          </span>
          {#if candidate.manual_modified}
            <span class="rounded bg-blue-500/10 px-2 py-0.5 text-[11px] text-blue-500 border border-blue-500/20">
              已手工微调
            </span>
          {/if}
        </div>
        <h3 class="text-base font-semibold text-fg mt-2">
          高光片段 #{candidate.id.slice(0, 8)}
        </h3>
        <p class="text-xs text-fg-muted mt-1">
          所属录播场次: <span class="font-mono">{candidate.live_id}</span>
        </p>
      </div>

      <!-- 成片状态徽标 -->
      <OutputStatus {outputs} {onOpenProject} />
    </div>

    <!-- 推荐理由与证据 -->
    <div class="ui-card p-4 space-y-2 bg-control/30 border-line">
      <div class="flex items-center gap-1.5 text-xs font-medium text-fg">
        <Sparkles class="h-3.5 w-3.5 text-amber-500" />
        <span>识别推荐理由</span>
      </div>
      <p class="text-xs text-fg leading-relaxed">
        {candidate.reason || "模型根据滑窗综合评分识别出的高光片段。"}
      </p>
    </div>

    <!-- 时间选区与边界微调 -->
    <div class="ui-card p-4 space-y-4">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-1.5 text-xs font-medium text-fg">
          <Sliders class="h-3.5 w-3.5 text-accent" />
          <span>选区边界与分类微调</span>
        </div>
        <div class="text-xs text-fg-muted flex items-center gap-1">
          <Clock class="h-3 w-3" />
          <span>总时长: {formatTime(Math.max(0, editEndSec - editStartSec))}</span>
        </div>
      </div>

      <div class="grid grid-cols-1 sm:grid-cols-3 gap-3 text-xs">
        <div>
          <label for="detail-content-type" class="block font-medium mb-1 text-fg-muted">片段类型</label>
          <select
            id="detail-content-type"
            class="ui-input w-full"
            bind:value={editContentType}
          >
            {#each contentTypes as ct}
              <option value={ct.value}>{ct.label}</option>
            {/each}
          </select>
        </div>

        <div>
          <label for="detail-start-sec" class="block font-medium mb-1 text-fg-muted">
            起始时间 (秒 / {formatTime(editStartSec)})
          </label>
          <input
            id="detail-start-sec"
            type="number"
            min="0"
            step="1"
            class="ui-input w-full"
            bind:value={editStartSec}
          />
        </div>

        <div>
          <label for="detail-end-sec" class="block font-medium mb-1 text-fg-muted">
            结束时间 (秒 / {formatTime(editEndSec)})
          </label>
          <input
            id="detail-end-sec"
            type="number"
            min="0"
            step="1"
            class="ui-input w-full"
            bind:value={editEndSec}
          />
        </div>
      </div>

      <div class="flex items-center justify-between pt-2">
        <div>
          {#if statusMessage}
            <div class="text-xs flex items-center gap-1.5 {statusMessage.type === 'success' ? 'text-green-600 dark:text-green-400' : 'text-red-600 dark:text-red-400'}">
              {#if statusMessage.type === 'success'}
                <Check class="h-3.5 w-3.5" />
              {:else}
                <AlertCircle class="h-3.5 w-3.5" />
              {/if}
              <span>{statusMessage.text}</span>
            </div>
          {/if}
        </div>
        <button
          type="button"
          class="ui-btn ui-btn-sm flex items-center gap-1"
          onclick={handleSaveAdjustment}
          disabled={isSaving}
        >
          {#if isSaving}
            <Loader2 class="h-3.5 w-3.5 animate-spin" />
            <span>保存中...</span>
          {:else}
            <Check class="h-3.5 w-3.5" />
            <span>保存微调</span>
          {/if}
        </button>
      </div>
    </div>

    <!-- 工程生成与导出操作 -->
    <div class="ui-card p-4 space-y-3">
      <div class="flex items-center gap-1.5 text-xs font-medium text-fg">
        <FolderKanban class="h-3.5 w-3.5 text-accent" />
        <span>切片工程操作</span>
      </div>
      <p class="text-xs text-fg-muted">
        一键执行关键帧偏移对齐，生成带有免二次转写伴随字幕的 Timeline V1 工程，可直接在切片编辑器中精剪。
      </p>

      <div class="flex items-center gap-3 pt-2">
        <button
          type="button"
          class="ui-btn ui-btn-sm ui-btn-primary flex items-center gap-1.5"
          onclick={handleGenerateProject}
          disabled={isGenerating}
        >
          {#if isGenerating}
            <Loader2 class="h-3.5 w-3.5 animate-spin" />
            <span>生成切片工程中...</span>
          {:else}
            <Scissors class="h-3.5 w-3.5" />
            <span>一键生成切片工程</span>
          {/if}
        </button>

        {#if outputs.some((o) => o.project_id)}
          {#each outputs.filter((o) => o.project_id) as out}
            <button
              type="button"
              class="ui-btn ui-btn-sm flex items-center gap-1"
              onclick={() => onOpenProject?.(out.project_id!)}
            >
              <FolderKanban class="h-3.5 w-3.5 text-accent" />
              <span>打开已生成工程 (#{out.project_id})</span>
            </button>
          {/each}
        {/if}
      </div>
    </div>
  </div>
{/if}
