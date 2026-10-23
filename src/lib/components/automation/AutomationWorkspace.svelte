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
  import type {
    AutomationCandidateRow,
    AutomationRunRow,
    RecorderList,
  } from "../../interface";
  import type { RecordItem } from "../../db";
  import { invoke } from "../../invoker";
  import { createStateRefresh, watchStateChanges } from "../../state-refresh";
  import { usePageVisibility } from "../../page-visibility";
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
  let recordMap = $state<Map<string, { title: string; userName?: string }>>(new Map());

  let loadingRuns = $state(false);
  let loadingCandidates = $state(false);
  let showRunDialog = $state(false);
  let isCancelling = $state(false);

  let selectedRun = $derived(runs.find((r) => r.id === selectedRunId) || null);
  let selectedCandidate = $derived(
    candidates.find((c) => c.id === selectedCandidateId) || null
  );
  let selectedRunRecord = $derived(
    selectedRun?.live_id ? recordMap.get(selectedRun.live_id) : undefined
  );

  /**
   * 将平台英文标识转换为中文显示
   * @param p 平台英文标识（如 bilibili, douyin 等）
   * @returns 平台中文名称
   */
  function platformLabel(p: string): string {
    const map: Record<string, string> = {
      bilibili: "B站",
      douyin: "抖音",
      douyu: "斗鱼",
      huya: "虎牙",
      kuaishou: "快手",
      tiktok: "TikTok",
      twitch: "Twitch",
      youtube: "YouTube",
    };
    return map[p?.toLowerCase()] || p || "";
  }

  /**
   * 将自动化运行状态转换为中文显示
   * @param status 运行状态
   * @returns 中文状态文本
   */
  function statusLabel(status: string): string {
    switch (status) {
      case "pending":
        return "排队中";
      case "processing":
        return "分析中";
      case "success":
        return "已完成";
      case "partial_success":
        return "部分完成";
      case "failed":
        return "失败";
      case "cancelled":
        return "已取消";
      default:
        return status;
    }
  }

  /**
   * 格式化录播场次下拉框的选项文案（包含录播标题）
   * @param r 自动化运行记录
   * @returns 选项格式化文案
   */
  function formatRunOption(r: AutomationRunRow): string {
    const meta = recordMap.get(r.live_id);
    const title = meta?.title?.trim();
    const platform = platformLabel(r.platform);
    const status = statusLabel(r.status);
    const idDisplay = r.live_id || r.id.slice(0, 8);

    if (title) {
      return `【${title}】 ${platform} · ${idDisplay} (${status})`;
    }
    return `${platform} · ${r.room_id} · ${idDisplay} (${status})`;
  }

  /**
   * 刷新运行记录列表以及关联的历史录播标题与主播信息
   */
  async function refreshRuns() {
    loadingRuns = true;
    try {
      const [list, recentRecords, recorderList] = await Promise.all([
        getAutomationRuns(undefined, undefined, 30),
        invoke<RecordItem[]>("get_recent_record", {
          roomId: "",
          offset: 0,
          limit: 100,
        }).catch(() => [] as RecordItem[]),
        invoke<RecorderList>("get_recorder_list").catch(() => null),
      ]);

      const map = new Map<string, { title: string; userName?: string }>();
      const userMap = new Map<string, string>();
      if (recorderList?.recorders) {
        for (const rec of recorderList.recorders) {
          userMap.set(
            rec.room_info.room_id,
            rec.user_info?.user_name || rec.room_info?.room_title || rec.room_info.room_id
          );
        }
      }

      if (recentRecords) {
        for (const item of recentRecords) {
          if (item.live_id) {
            map.set(item.live_id, {
              title: item.title,
              userName: userMap.get(item.room_id),
            });
          }
        }
      }

      // 若有运行记录的 live_id 未在 recentRecords 中命中，按需拉取对应归档信息
      const missingRuns = list.filter((r) => r.live_id && !map.has(r.live_id));
      if (missingRuns.length > 0) {
        await Promise.allSettled(
          missingRuns.map(async (r) => {
            try {
              const archive = await invoke<RecordItem>("get_archive", {
                roomId: r.room_id,
                liveId: r.live_id,
              });
              if (archive && archive.title) {
                map.set(r.live_id, {
                  title: archive.title,
                  userName: userMap.get(r.room_id),
                });
              }
            } catch {
              // 忽略单个未找到的录播
            }
          })
        );
      }

      recordMap = map;
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

  const pageVisibility = usePageVisibility();
  const stateRefresh = createStateRefresh(async () => {
    await refreshRuns();
    if (selectedRunId) {
      await refreshCandidates(selectedRunId);
    }
  });

  onMount(() => {
    return watchStateChanges(stateRefresh, ["automation-changed"], 30_000, pageVisibility);
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
          class="ui-input min-w-[280px] max-w-[420px] text-xs"
          bind:value={selectedRunId}
        >
          {#each runs as r}
            <option value={r.id}>
              {formatRunOption(r)}
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
      <div class="flex items-center gap-3 flex-wrap">
        {#if selectedRunRecord?.title}
          <div class="flex items-center gap-1.5 font-medium text-fg">
            <span class="text-fg-muted">录播标题:</span>
            <span class="font-semibold text-fg truncate max-w-sm" title={selectedRunRecord.title}>
              {selectedRunRecord.title}
            </span>
            {#if selectedRunRecord.userName}
              <span class="text-xs text-fg-muted">({selectedRunRecord.userName})</span>
            {/if}
          </div>
          <span class="text-line">|</span>
        {/if}
        <div class="flex items-center gap-1.5">
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
