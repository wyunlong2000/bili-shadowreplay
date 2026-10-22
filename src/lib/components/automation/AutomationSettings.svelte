<script lang="ts">
  import { onMount } from "svelte";
  import { getAutomationPolicy, saveAutomationPolicy } from "../../automation";
  import type { AutoClippingPolicy, ClipOutputMode, LiveContentType } from "../../interface";

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

  let saving = $state(false);
  let saveStatus = $state<"idle" | "saved" | "error">("idle");
  let errorMessage = $state("");
  let showAdvanced = $state(false);

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
    { value: "both", label: "原声版 + AI 解说版（推荐）" },
    { value: "original", label: "仅原声高光版" },
    { value: "commentary", label: "仅 AI 解说版" },
  ];

  async function load() {
    try {
      const res = await getAutomationPolicy();
      policy = res.policy;
    } catch (e: any) {
      console.error("加载自动化策略失败:", e);
    }
  }

  async function handleSave() {
    saving = true;
    saveStatus = "idle";
    errorMessage = "";
    try {
      await saveAutomationPolicy(policy);
      saveStatus = "saved";
      setTimeout(() => {
        if (saveStatus === "saved") saveStatus = "idle";
      }, 2500);
    } catch (e: any) {
      saveStatus = "error";
      errorMessage = String(e);
    } finally {
      saving = false;
    }
  }

  onMount(load);
</script>

<div class="space-y-4">
  <div class="flex items-center justify-between">
    <div>
      <h3 class="ui-label text-base font-medium">智能高光切片 (自动化)</h3>
      <p class="ui-hint mt-0.5">
        下播后自动利用历史转写与滑窗模型识别精彩片段，生成经关键帧校准的切片工程
      </p>
    </div>
    <label class="ui-switch">
      <input
        type="checkbox"
        bind:checked={policy.enabled}
        onchange={handleSave}
      />
      <span></span>
    </label>
  </div>

  {#if policy.enabled}
    <div class="mt-4 pt-4 border-t border-line space-y-4">
      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div>
          <label class="block text-sm font-medium mb-1">默认直播内容类型</label>
          <select
            class="w-full bg-surface border border-line rounded px-3 py-1.5 text-sm"
            bind:value={policy.content_type}
            onchange={handleSave}
          >
            {#each contentTypes as ct}
              <option value={ct.value}>{ct.label}</option>
            {/each}
          </select>
        </div>

        <div>
          <label class="block text-sm font-medium mb-1">成片生成版本</label>
          <select
            class="w-full bg-surface border border-line rounded px-3 py-1.5 text-sm"
            bind:value={policy.output_mode}
            onchange={handleSave}
          >
            {#each outputModes as om}
              <option value={om.value}>{om.label}</option>
            {/each}
          </select>
        </div>

        <div>
          <label class="block text-sm font-medium mb-1">单场最多候选数</label>
          <input
            type="number"
            min="1"
            max="20"
            class="w-full bg-surface border border-line rounded px-3 py-1.5 text-sm"
            bind:value={policy.max_candidates}
            onchange={handleSave}
          />
        </div>

        <div>
          <label class="block text-sm font-medium mb-1">最低采纳评分 (0 - 100)</label>
          <input
            type="number"
            min="0"
            max="100"
            step="5"
            class="w-full bg-surface border border-line rounded px-3 py-1.5 text-sm"
            bind:value={policy.min_score}
            onchange={handleSave}
          />
        </div>
      </div>

      <div class="flex items-center justify-between pt-2">
        <label class="flex items-center gap-2 cursor-pointer text-sm">
          <input
            type="checkbox"
            class="rounded border-line"
            bind:checked={policy.auto_classify}
            onchange={handleSave}
          />
          <span>自动识别混合内容类型</span>
        </label>

        <button
          type="button"
          class="text-xs text-primary hover:underline"
          onclick={() => (showAdvanced = !showAdvanced)}
        >
          {showAdvanced ? "收起高级参数" : "展开高级切片参数"}
        </button>
      </div>

      {#if showAdvanced}
        <div class="bg-surface/50 p-3 rounded border border-line grid grid-cols-2 gap-4 text-xs">
          <div>
            <label class="block font-medium mb-1">滑窗大小 (秒)</label>
            <input
              type="number"
              min="60"
              max="1800"
              step="30"
              class="w-full bg-surface border border-line rounded px-2 py-1"
              bind:value={policy.window_seconds}
              onchange={handleSave}
            />
            <span class="text-secondary opacity-75">推荐 300 秒</span>
          </div>
          <div>
            <label class="block font-medium mb-1">窗口重叠 (秒)</label>
            <input
              type="number"
              min="10"
              max="300"
              step="10"
              class="w-full bg-surface border border-line rounded px-2 py-1"
              bind:value={policy.overlap_seconds}
              onchange={handleSave}
            />
            <span class="text-secondary opacity-75">推荐 30 秒</span>
          </div>
        </div>
      {/if}

      {#if saveStatus === "saved"}
        <p class="text-xs text-emerald-500">策略已自动保存</p>
      {:else if saveStatus === "error"}
        <p class="text-xs text-rose-500">保存失败: {errorMessage}</p>
      {/if}
    </div>
  {/if}
</div>
