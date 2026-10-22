<!--
  手动发起场次高光分析弹窗
  允许用户选择或输入场次信息，启动全自动高光识别流水线
-->
<script lang="ts">
  import { startAutomationAnalysis } from "../../automation";
  import { X, Play, Loader2, AlertCircle } from "lucide-svelte";

  interface Props {
    open: boolean;
    platform?: string;
    roomId?: string;
    parentId?: string;
    liveId?: string;
    onClose: () => void;
    onSuccess?: (runId: string) => void;
  }

  let {
    open,
    platform = "bilibili",
    roomId = "",
    parentId = "",
    liveId = "",
    onClose,
    onSuccess,
  }: Props = $props();

  let inputPlatform = $state("");
  let inputRoomId = $state("");
  let inputParentId = $state("");
  let inputLiveId = $state("");

  let loading = $state(false);
  let error = $state("");

  $effect(() => {
    if (open) {
      inputPlatform = platform;
      inputRoomId = roomId;
      inputParentId = parentId;
      inputLiveId = liveId;
      error = "";
    }
  });

  async function handleSubmit() {
    if (!inputPlatform || !inputRoomId || !inputParentId || !inputLiveId) {
      error = "请完整填写平台、房间号、录播目录和场次标识";
      return;
    }
    loading = true;
    error = "";
    try {
      const runId = await startAutomationAnalysis(
        inputPlatform.trim(),
        inputRoomId.trim(),
        inputParentId.trim(),
        inputLiveId.trim()
      );
      onSuccess?.(runId);
      onClose();
    } catch (e: any) {
      error = "启动高光分析失败: " + (e?.message || e);
    } finally {
      loading = false;
    }
  }
</script>

{#if open}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4 backdrop-blur-sm">
    <div class="w-full max-w-md rounded-xl bg-surface border border-line p-6 shadow-2xl space-y-4">
      <!-- 弹窗标题 -->
      <div class="flex items-center justify-between border-b border-line pb-3">
        <div class="flex items-center gap-2">
          <Play class="h-4 w-4 text-accent" />
          <h2 class="text-base font-semibold text-fg">发起智能高光分析</h2>
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
        系统将校验该录播是否已完成封口，读取历史字幕证据并按 300 秒滑窗执行多维评分，识别出精彩片段。
      </p>

      {#if error}
        <div class="rounded-lg bg-red-500/10 border border-red-500/20 p-3 text-xs text-red-600 dark:text-red-400 flex items-start gap-2">
          <AlertCircle class="h-4 w-4 shrink-0 mt-0.5" />
          <span>{error}</span>
        </div>
      {/if}

      <!-- 表单字段 -->
      <div class="space-y-3 text-xs">
        <div>
          <label for="auto-run-platform" class="block font-medium mb-1 text-fg">直播平台</label>
          <input
            id="auto-run-platform"
            type="text"
            class="ui-input w-full"
            placeholder="例如 bilibili / douyin"
            bind:value={inputPlatform}
          />
        </div>

        <div>
          <label for="auto-run-roomid" class="block font-medium mb-1 text-fg">房间号</label>
          <input
            id="auto-run-roomid"
            type="text"
            class="ui-input w-full"
            placeholder="直播间真实房间号"
            bind:value={inputRoomId}
          />
        </div>

        <div>
          <label for="auto-run-parentid" class="block font-medium mb-1 text-fg">录播录制目录 (Parent ID)</label>
          <input
            id="auto-run-parentid"
            type="text"
            class="ui-input w-full font-mono text-[11px]"
            placeholder="如 record_20261022_..."
            bind:value={inputParentId}
          />
        </div>

        <div>
          <label for="auto-run-liveid" class="block font-medium mb-1 text-fg">场次标识 (Live ID)</label>
          <input
            id="auto-run-liveid"
            type="text"
            class="ui-input w-full font-mono text-[11px]"
            placeholder="如 live_20261022_..."
            bind:value={inputLiveId}
          />
        </div>
      </div>

      <!-- 底部操作 -->
      <div class="flex items-center justify-end gap-2 pt-3 border-t border-line">
        <button
          type="button"
          class="ui-btn ui-btn-sm"
          onclick={onClose}
          disabled={loading}
        >
          取消
        </button>
        <button
          type="button"
          class="ui-btn ui-btn-sm ui-btn-primary flex items-center gap-1.5"
          onclick={handleSubmit}
          disabled={loading}
        >
          {#if loading}
            <Loader2 class="h-3.5 w-3.5 animate-spin" />
            <span>提交中...</span>
          {:else}
            <Play class="h-3.5 w-3.5" />
            <span>开始分析</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}
