<!--
  自动化切片工作区
  包含场次运行状态、滑窗高光候选列表与微调详情面板
-->
<script lang="ts">
  import { onMount } from "svelte";
  import {
    getAutomationRuns,
    getAutomationCandidates,
    cancelAutomationRun,
  } from "../../automation";
  import type { AutomationCandidateRow, AutomationRunRow } from "../../interface";
  import { invoke } from "../../invoker";
  import { createStateRefresh, watchStateChanges } from "../../state-refresh";
  import CandidateList from "./CandidateList.svelte";
  import CandidateDetail from "./CandidateDetail.svelte";
  import RunAutomationDialog from "./RunAutomationDialog.svelte";
  import {
    Sparkles,
    Play,
    RefreshCw,
    XCircle,
    Loader2,
    Calendar,
    Radio,
    Clock,
  } from "lucide-svelte";

  interface Props {
    onOpenProject?: (projectId: number) => void;
  }

  let { onOpenProject }: Props = $props();

  let runs = $state<AutomationRunRow[]>([]);
  let selectedRunId = $state<string | null>(null);
  let candidates = $state<AutomationCandidateRow[]>([]);
  let selectedCandidateId = $state<string | null>(null);

  let loadingRuns = $state(false);
  let loadingCandidates = $state(false);
  let showRunDialog = $state(false);
  let isCancelling = $state(false);

  let selectedRun = $derived(runs.find((r) => r.id === selectedRunId) || null);
  let selectedCandidate = $derived(
    candidates.find((c) => c.id === selectedCandidateId) || null
  );

  /**
   * 刷新运行记录列表
   */
  async function refreshRuns() {
    loadingRuns = true;
    try {
      const list = await getAutomationRuns(undefined, undefined, 30);
      runs = list;
      if (list.length > 0) {
        if (!selectedRunId || !list.some((r) => r.id === selectedRunId)) {
          selectedRunId = list[0].id;
        }
      } else {
        selectedRunId = null;
      }
    } catch (e) {
      console.error("加载自动化运行记录失败:", e);
    } finally {
      loadingRuns = false;
    }
  }

  /**
   * 刷新指定运行记录下的候选片段
   */
  async function refreshCandidates(runId: string | null) {
    if (!runId) {
      candidates = [];
      selectedCandidateId = null;
      return;
    }
    loadingCandidates = true;
    try {
      const list = await getAutomationCandidates(runId);
      candidates = list;
      if (list.length > 0) {
        if (!selectedCandidateId || !list.some((c) => c.id === selectedCandidateId)) {
          selectedCandidateId = list[0].id;
        }
      } else {
        selectedCandidateId = null;
      }
    } catch (e) {
      console.error("加载高光候选失败:", e);
    } finally {
      loadingCandidates = false;
    }
  }

  $effect(() => {
    if (selectedRunId) {
      refreshCandidates(selectedRunId);
    }
  });

  const stateRefresh = createStateRefresh(async () => {
    await refreshRuns();
    if (selectedRunId) {
      await refreshCandidates(selectedRunId);
    }
  });

  onMount(() => {
    refreshRuns();
    return watchStateChanges(stateRefresh, ["automation-changed"]);
  });

  async function handleCancelRun() {
    if (!selectedRunId) return;
    isCancelling = true;
    try {
      await cancelAutomationRun(selectedRunId);
      await refreshRuns();
    } catch (e) {
      console.error("取消任务失败:", e);
    } finally {
      isCancelling = false;
    }
  }

  function handleOpenProject(projectId: number) {
    if (onOpenProject) {
      onOpenProject(projectId);
    } else {
      invoke("open_project", { projectId }).catch((e) =>
        console.error("打开工程失败:", e)
      );
    }
  }
</script>

<div class="h-full flex flex-col space-y-4">
  <!-- 顶栏：场次切换与手动触发 -->
  <div class="ui-card p-3 flex flex-wrap items-center justify-between gap-3">
    <div class="flex items-center gap-3">
      <div class="flex items-center gap-1.5 text-xs text-fg font-medium">
        <Radio class="h-4 w-4 text-accent" />
        <span>录播场次:</span>
      </div>

      {#if runs.length === 0}
        <span class="text-xs text-fg-muted">暂无已运行的分析记录</span>
      {:else}
        <select
          class="ui-input min-w-[260px] text-xs font-mono"
          bind:value={selectedRunId}
        >
          {#each runs as r}
            <option value={r.id}>
              {r.platform} · {r.room_id} · {r.live_id || r.id.slice(0, 8)} ({r.status})
            </option>
          {/each}
        </select>
      {/if}

      <button
        type="button"
        class="ui-btn ui-btn-sm"
        title="刷新"
        onclick={() => stateRefresh.run()}
        disabled={loadingRuns || loadingCandidates}
      >
        <RefreshCw class="h-3.5 w-3.5 {loadingRuns || loadingCandidates ? 'animate-spin' : ''}" />
      </button>
    </div>

    <button
      type="button"
      class="ui-btn ui-btn-sm ui-btn-primary flex items-center gap-1.5"
      onclick={() => (showRunDialog = true)}
    >
      <Play class="h-3.5 w-3.5" />
      <span>手动分析录播</span>
    </button>
  </div>

  <!-- 任务运行状态进度卡片 (如果正在运行或失败) -->
  {#if selectedRun}
    <div class="ui-card p-3 bg-control/40 border-line flex flex-wrap items-center justify-between gap-3 text-xs">
      <div class="flex items-center gap-3">
        <span class="font-medium text-fg">场次状态:</span>
        {#if selectedRun.status === "processing"}
          <span class="flex items-center gap-1 text-blue-500">
            <Loader2 class="h-3.5 w-3.5 animate-spin" />
            正在分析 ({selectedRun.stage} 阶段)
          </span>
          {#if selectedRun.total_windows > 0}
            <span class="text-fg-muted">
              滑窗进度: {selectedRun.completed_windows} / {selectedRun.total_windows}
            </span>
          {/if}
        {:else if selectedRun.status === "success" || selectedRun.status === "partial_success"}
          <span class="text-green-600 dark:text-green-400 font-medium">
            已完成 · 识别出 {candidates.length} 个高光片段
          </span>
        {:else if selectedRun.status === "failed"}
          <span class="text-red-500 font-medium">
            分析失败: {selectedRun.error_message || selectedRun.error_code || "未知错误"}
          </span>
        {:else if selectedRun.status === "cancelled"}
          <span class="text-fg-muted">已取消</span>
        {:else}
          <span class="text-amber-500">{selectedRun.status}</span>
        {/if}
      </div>

      {#if selectedRun.status === "processing" || selectedRun.status === "pending"}
        <button
          type="button"
          class="ui-btn ui-btn-sm text-red-500 hover:text-red-600 flex items-center gap-1"
          onclick={handleCancelRun}
          disabled={isCancelling}
        >
          <XCircle class="h-3.5 w-3.5" />
          <span>取消分析</span>
        </button>
      {/if}
    </div>
  {/if}

  <!-- 两栏主界面：左侧候选列表，右侧微调详情 -->
  <div class="flex-1 min-h-[460px] grid grid-cols-1 md:grid-cols-12 gap-4 overflow-hidden">
    <!-- 左侧列表 -->
    <div class="md:col-span-5 ui-card flex flex-col overflow-hidden">
      <div class="border-b border-line px-3.5 py-2.5 flex items-center justify-between">
        <div class="flex items-center gap-1.5 text-xs font-semibold text-fg">
          <Sparkles class="h-3.5 w-3.5 text-amber-500" />
          <span>高光候选片段</span>
        </div>
        <span class="text-[11px] text-fg-muted">共 {candidates.length} 条</span>
      </div>
      <div class="flex-1 overflow-hidden">
        <CandidateList
          {candidates}
          {selectedCandidateId}
          onSelect={(c) => (selectedCandidateId = c.id)}
        />
      </div>
    </div>

    <!-- 右侧详情 -->
    <div class="md:col-span-7 ui-card flex flex-col overflow-hidden">
      <div class="flex-1 overflow-y-auto custom-scrollbar-light">
        <CandidateDetail
          candidate={selectedCandidate}
          onUpdated={() => {
            if (selectedRunId) refreshCandidates(selectedRunId);
          }}
          {onOpenProject}
        />
      </div>
    </div>
  </div>
</div>

<!-- 触发分析弹窗 -->
<RunAutomationDialog
  open={showRunDialog}
  onClose={() => (showRunDialog = false)}
  onSuccess={(runId) => {
    refreshRuns().then(() => {
      selectedRunId = runId;
    });
  }}
/>
