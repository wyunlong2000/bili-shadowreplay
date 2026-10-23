<!--
  手动发起场次高光分析弹窗
  允许用户直观选择已有历史录播场次，一键启动全自动高光识别流水线
-->
<script lang="ts">
  import { startAutomationAnalysis } from "../../automation";
  import { invoke } from "../../invoker";
  import { formatTimecode } from "../../time-format";
  import { formatSize } from "../../size-format";
  import type { RecordItem } from "../../db";
  import type { RecorderList } from "../../interface";
  import {
    X,
    Play,
    Loader2,
    AlertCircle,
    Sparkles,
    Calendar,
    Clock,
    HardDrive,
    Radio,
  } from "lucide-svelte";

  interface Props {
    open: boolean;
    archive?: RecordItem | null;
    platform?: string;
    roomId?: string;
    parentId?: string;
    liveId?: string;
    onClose: () => void;
    onSuccess?: (runId: string) => void;
  }

  let {
    open,
    archive = null,
    platform = "",
    roomId = "",
    parentId = "",
    liveId = "",
    onClose,
    onSuccess,
  }: Props = $props();

  let records = $state<RecordItem[]>([]);
  let recorders = $state<Map<string, string>>(new Map());
  let loadingRecords = $state(false);
  let selectedLiveId = $state("");
  let submitting = $state(false);
  let error = $state("");

  /** 根据选中的 live_id 或传入的 archive 计算当前目标录播 */
  let currentTarget = $derived.by(() => {
    if (archive) return archive;
    if (selectedLiveId) {
      return records.find((r) => r.live_id === selectedLiveId) || null;
    }
    return null;
  });

  /** 格式化平台名称为中文展示 */
  function platformLabel(p: string): string {
    const map: Record<string, string> = {
      bilibili: "哔哩哔哩",
      douyin: "抖音",
      douyu: "斗鱼",
      huya: "虎牙",
      kuaishou: "快手",
      tiktok: "TikTok",
      twitch: "Twitch",
      youtube: "YouTube",
    };
    return map[p.toLowerCase()] || p;
  }

  /** 获取房间对应的主播昵称或房间标题 */
  function getRoomDisplayName(rId: string): string {
    return recorders.get(rId) || rId;
  }

  /** 弹窗打开时加载录播列表与主播信息 */
  $effect(() => {
    if (open) {
      error = "";
      if (archive) {
        selectedLiveId = archive.live_id;
      } else if (liveId) {
        selectedLiveId = liveId;
      }
      loadRecordOptions();
    }
  });

  /** 拉取历史录播与主播映射 */
  async function loadRecordOptions() {
    loadingRecords = true;
    try {
      const [recent, list] = await Promise.all([
        invoke<RecordItem[]>("get_recent_record", {
          roomId: "",
          offset: 0,
          limit: 50,
        }),
        invoke<RecorderList>("get_recorder_list").catch(() => null),
      ]);

      records = recent || [];
      if (list && list.recorders) {
        const map = new Map<string, string>();
        for (const r of list.recorders) {
          map.set(
            r.room_info.room_id,
            r.user_info?.user_name || r.room_info?.room_title || r.room_info.room_id
          );
        }
        recorders = map;
      }

      if (!archive && records.length > 0 && !selectedLiveId) {
        selectedLiveId = records[0].live_id;
      }
    } catch (e: any) {
      console.error("加载历史录播失败:", e);
      error = "加载历史录播失败: " + (e?.message || e);
    } finally {
      loadingRecords = false;
    }
  }

  /** 提交自动化分析任务 */
  async function handleSubmit() {
    const target = currentTarget;
    if (!target) {
      error = "请选择需要分析的录播场次";
      return;
    }

    if (!target.parent_id || !target.live_id) {
      error = "所选录播缺少场次标识信息，无法构建时间索引";
      return;
    }

    submitting = true;
    error = "";
    try {
      const runId = await startAutomationAnalysis(
        target.platform,
        target.room_id,
        target.parent_id,
        target.live_id
      );
      onSuccess?.(runId);
      onClose();
    } catch (e: any) {
      error = "启动高光分析失败: " + (e?.message || e);
    } finally {
      submitting = false;
    }
  }
</script>

{#if open}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4 backdrop-blur-sm">
    <div class="w-full max-w-lg rounded-xl bg-surface border border-line p-6 shadow-2xl space-y-4">
      <!-- 弹窗标题 -->
      <div class="flex items-center justify-between border-b border-line pb-3">
        <div class="flex items-center gap-2">
          <Sparkles class="h-4 w-4 text-accent-fg" />
          <h2 class="text-base font-semibold text-fg">发起智能高光切片分析</h2>
        </div>
        <button
          type="button"
          class="rounded p-1 text-fg-muted hover:bg-control hover:text-fg transition-colors"
          onclick={onClose}
        >
          <X class="h-4 w-4" />
        </button>
      </div>

      <p class="text-xs text-fg-muted leading-relaxed">
        系统将对所选录播场次建立时间索引，提取台词字幕与弹幕高能点，按 300 秒滑窗执行多维评分并自动识别高光精彩片段。
      </p>

      {#if error}
        <div class="rounded-lg bg-red-500/10 border border-red-500/20 p-3 text-xs text-red-600 dark:text-red-400 flex items-start gap-2">
          <AlertCircle class="h-4 w-4 shrink-0 mt-0.5" />
          <span>{error}</span>
        </div>
      {/if}

      <!-- 录播场次选择区域 -->
      <div class="space-y-3">
        {#if !archive}
          <div>
            <label for="auto-run-select" class="block text-xs font-medium mb-1.5 text-fg">选择历史录播场次</label>
            {#if loadingRecords}
              <div class="flex items-center gap-2 p-3 text-xs text-fg-muted bg-sunken rounded-lg border border-line">
                <Loader2 class="h-4 w-4 animate-spin text-accent-fg" />
                <span>正在获取已完成录播列表...</span>
              </div>
            {:else if records.length === 0}
              <div class="p-4 text-xs text-center text-fg-muted bg-sunken rounded-lg border border-line">
                暂无已录制完成的录播记录。请先在直播间完成至少一场直播录制。
              </div>
            {:else}
              <select
                id="auto-run-select"
                class="ui-input w-full text-xs"
                bind:value={selectedLiveId}
              >
                {#each records as r (r.live_id)}
                  <option value={r.live_id}>
                    [{platformLabel(r.platform)}] {getRoomDisplayName(r.room_id)} · {r.title || "未命名录播"} ({new Date(r.created_at).toLocaleDateString()} {formatTimecode(r.length, 0)})
                  </option>
                {/each}
              </select>
            {/if}
          </div>
        {/if}

        <!-- 选中录播详情预览卡片 -->
        {#if currentTarget}
          <div class="rounded-lg border border-line bg-sunken/60 p-3.5 space-y-2.5">
            <div class="flex items-start justify-between gap-2">
              <div class="space-y-1 min-w-0 flex-1">
                <div class="flex items-center gap-1.5 flex-wrap">
                  <span class="inline-flex items-center px-1.5 py-0.5 rounded text-[11px] font-medium bg-accent-soft text-accent-fg">
                    {platformLabel(currentTarget.platform)}
                  </span>
                  <span class="text-xs font-semibold text-fg truncate" title={currentTarget.title}>
                    {currentTarget.title || "未命名录播"}
                  </span>
                </div>
                <div class="flex items-center gap-1 text-[11px] text-fg-muted">
                  <Radio class="h-3 w-3 text-fg-faint" />
                  <span class="truncate">{getRoomDisplayName(currentTarget.room_id)}</span>
                </div>
              </div>
            </div>

            <div class="grid grid-cols-3 gap-2 pt-2 border-t border-line/60 text-[11px] text-fg-secondary">
              <div class="flex items-center gap-1">
                <Clock class="h-3 w-3 text-fg-faint" />
                <span class="tabular-nums">{formatTimecode(currentTarget.length, 0)}</span>
              </div>
              <div class="flex items-center gap-1">
                <HardDrive class="h-3 w-3 text-fg-faint" />
                <span class="tabular-nums">{formatSize(currentTarget.size)}</span>
              </div>
              <div class="flex items-center gap-1">
                <Calendar class="h-3 w-3 text-fg-faint" />
                <span class="truncate">{new Date(currentTarget.created_at).toLocaleDateString()}</span>
              </div>
            </div>
          </div>
        {/if}
      </div>

      <!-- 底部操作按钮 -->
      <div class="flex items-center justify-end gap-2 pt-3 border-t border-line">
        <button
          type="button"
          class="ui-btn ui-btn-sm"
          onclick={onClose}
          disabled={submitting}
        >
          取消
        </button>
        <button
          type="button"
          class="ui-btn ui-btn-sm ui-btn-primary flex items-center gap-1.5"
          onclick={handleSubmit}
          disabled={submitting || !currentTarget}
        >
          {#if submitting}
            <Loader2 class="h-3.5 w-3.5 animate-spin" />
            <span>提交分析中...</span>
          {:else}
            <Play class="h-3.5 w-3.5" />
            <span>开始分析</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}
