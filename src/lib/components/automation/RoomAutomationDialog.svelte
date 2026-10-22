<script lang="ts">
  import { onMount } from "svelte";
  import { getAutomationPolicy, saveAutomationPolicy } from "../../automation";
  import type { AutoClippingPolicy, ClipOutputMode, LiveContentType } from "../../interface";

  interface Props {
    platform: string;
    roomId: string;
    roomTitle?: string;
    open: boolean;
    onClose: () => void;
  }

  let { platform, roomId, roomTitle, open, onClose }: Props = $props();

  let loading = $state(true);
  let saving = $state(false);
  let isOverride = $state(false);
  let revision = $state(0);
  let error = $state("");

  let policy: AutoClippingPolicy = $state({
    enabled: false,
    content_type: "gaming",
    auto_classify: true,
    output_mode: "both",
    window_seconds: 300,
    overlap_seconds: 30,
    max_candidates: 3,
    min_score: 60.0,
  });

  const contentTypes: { value: LiveContentType; label: string }[] = [
    { value: "gaming", label: "游戏高光" },
    { value: "chat", label: "聊天互动" },
    { value: "knowledge", label: "知识分享" },
    { value: "sports", label: "体育赛事" },
    { value: "commerce", label: "带货直播" },
    { value: "mixed", label: "综合混合" },
    { value: "other", label: "其他内容" },
  ];

  const outputModes: { value: ClipOutputMode; label: string }[] = [
    { value: "both", label: "原声版 + AI 解说版" },
    { value: "original", label: "仅原声高光版" },
    { value: "commentary", label: "仅 AI 解说版" },
  ];

  async function load() {
    if (!open) return;
    loading = true;
    error = "";
    try {
      const res = await getAutomationPolicy(platform, roomId);
      policy = res.policy;
      isOverride = !res.inherited;
      revision = res.revision;
    } catch (e: any) {
      error = "获取房间策略失败: " + e;
    } finally {
      loading = false;
    }
  }

  async function handleSave() {
    saving = true;
    error = "";
    try {
      if (!isOverride) {
        // 如果取消覆盖，可以保存一个继承或默认配置
        await saveAutomationPolicy(policy, platform, roomId, revision);
      } else {
        await saveAutomationPolicy(policy, platform, roomId, revision);
      }
      onClose();
    } catch (e: any) {
      error = "保存失败: " + e;
    } finally {
      saving = false;
    }
  }

  $effect(() => {
    if (open) {
      load();
    }
  });
</script>

{#if open}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4">
    <div class="w-full max-w-lg rounded-xl bg-surface border border-line p-6 shadow-2xl space-y-4">
      <div class="flex items-center justify-between border-b border-line pb-3">
        <div>
          <h2 class="text-base font-semibold">房间自动切片设置</h2>
          <p class="text-xs text-secondary mt-0.5">
            {platform} · 房间 {roomId} {roomTitle ? `(${roomTitle})` : ""}
          </p>
        </div>
        <button
          type="button"
          class="text-secondary hover:text-primary text-sm px-2 py-1"
          onclick={onClose}
        >
          ✕
        </button>
      </div>

      {#if loading}
        <div class="py-8 text-center text-sm text-secondary">加载设置中...</div>
      {:else}
        {#if error}
          <div class="p-3 text-xs bg-rose-500/10 text-rose-500 rounded border border-rose-500/20">
            {error}
          </div>
        {/if}

        <div class="space-y-4 text-sm">
          <div class="flex items-center justify-between p-3 rounded bg-surface/50 border border-line">
            <div>
              <span class="font-medium">自定义此房间策略</span>
              <p class="text-xs text-secondary mt-0.5">
                {isOverride ? "当前使用独立房间策略" : "当前继承全局默认策略"}
              </p>
            </div>
            <label class="ui-switch">
              <input
                type="checkbox"
                bind:checked={isOverride}
              />
              <span></span>
            </label>
          </div>

          {#if isOverride}
            <div class="space-y-3 pt-2">
              <div class="flex items-center justify-between">
                <span>启用此房间自动切片</span>
                <label class="ui-switch">
                  <input
                    type="checkbox"
                    bind:checked={policy.enabled}
                  />
                  <span></span>
                </label>
              </div>

              <div>
                <label class="block text-xs font-medium mb-1">主要内容类型</label>
                <select
                  class="w-full bg-surface border border-line rounded px-3 py-1.5 text-xs"
                  bind:value={policy.content_type}
                >
                  {#each contentTypes as ct}
                    <option value={ct.value}>{ct.label}</option>
                  {/each}
                </select>
              </div>

              <div>
                <label class="block text-xs font-medium mb-1">输出成片版本</label>
                <select
                  class="w-full bg-surface border border-line rounded px-3 py-1.5 text-xs"
                  bind:value={policy.output_mode}
                >
                  {#each outputModes as om}
                    <option value={om.value}>{om.label}</option>
                  {/each}
                </select>
              </div>

              <div class="grid grid-cols-2 gap-3">
                <div>
                  <label class="block text-xs font-medium mb-1">单场最大候选</label>
                  <input
                    type="number"
                    min="1"
                    max="20"
                    class="w-full bg-surface border border-line rounded px-3 py-1.5 text-xs"
                    bind:value={policy.max_candidates}
                  />
                </div>
                <div>
                  <label class="block text-xs font-medium mb-1">最低采纳分数</label>
                  <input
                    type="number"
                    min="0"
                    max="100"
                    step="5"
                    class="w-full bg-surface border border-line rounded px-3 py-1.5 text-xs"
                    bind:value={policy.min_score}
                  />
                </div>
              </div>
            </div>
          {/if}
        </div>

        <div class="flex items-center justify-end gap-2 pt-4 border-t border-line">
          <button
            type="button"
            class="px-4 py-1.5 text-xs rounded border border-line hover:bg-surface/80"
            onclick={onClose}
          >
            取消
          </button>
          <button
            type="button"
            class="px-4 py-1.5 text-xs rounded bg-primary text-white font-medium hover:opacity-90 disabled:opacity-50"
            disabled={saving}
            onclick={handleSave}
          >
            {saving ? "保存中..." : "保存设置"}
          </button>
        </div>
      {/if}
    </div>
  </div>
{/if}
