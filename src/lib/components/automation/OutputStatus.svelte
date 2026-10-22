<!--
  自动化切片成片状态徽标组件
  展示原声版与解说版成片进度与打开工程入口
-->
<script lang="ts">
  import type { AutomationOutputRow } from "../../interface";
  import { CheckCircle2, Clock, AlertCircle, Loader2, ExternalLink } from "lucide-svelte";

  interface Props {
    outputs: AutomationOutputRow[];
    onOpenProject?: (projectId: number) => void;
  }

  let { outputs = [], onOpenProject }: Props = $props();

  let acousticOutput = $derived(outputs.find((o) => o.output_mode === "original"));
  let commentaryOutput = $derived(outputs.find((o) => o.output_mode === "commentary"));

  function getStatusBadge(status?: string) {
    switch (status) {
      case "completed":
      case "success":
        return { label: "已就绪", color: "bg-green-500/10 text-green-600 dark:text-green-400 border-green-500/20" };
      case "processing":
      case "synthesizing":
      case "rendering":
        return { label: "处理中", color: "bg-blue-500/10 text-blue-600 dark:text-blue-400 border-blue-500/20" };
      case "failed":
        return { label: "失败", color: "bg-red-500/10 text-red-600 dark:text-red-400 border-red-500/20" };
      case "pending":
        return { label: "排队中", color: "bg-amber-500/10 text-amber-600 dark:text-amber-400 border-amber-500/20" };
      default:
        return { label: "未生成", color: "bg-control text-fg-muted border-line" };
    }
  }
</script>

<div class="flex flex-wrap items-center gap-2 text-xs">
  <!-- 原声版状态 -->
  <div class="flex items-center gap-1.5 rounded border px-2 py-1 {getStatusBadge(acousticOutput?.status).color}">
    {#if acousticOutput?.status === "completed" || acousticOutput?.status === "success"}
      <CheckCircle2 class="h-3.5 w-3.5" />
    {:else if acousticOutput?.status === "processing"}
      <Loader2 class="h-3.5 w-3.5 animate-spin" />
    {:else if acousticOutput?.status === "failed"}
      <AlertCircle class="h-3.5 w-3.5" />
    {:else}
      <Clock class="h-3.5 w-3.5" />
    {/if}
    <span>原声版: {getStatusBadge(acousticOutput?.status).label}</span>
    {#if acousticOutput?.project_id && onOpenProject}
      <button
        type="button"
        class="ml-1 hover:underline inline-flex items-center gap-0.5 text-accent"
        onclick={() => onOpenProject(acousticOutput!.project_id!)}
        title="打开原声切片工程"
      >
        <ExternalLink class="h-3 w-3" />
      </button>
    {/if}
  </div>

  <!-- 解说版状态 -->
  <div class="flex items-center gap-1.5 rounded border px-2 py-1 {getStatusBadge(commentaryOutput?.status).color}">
    {#if commentaryOutput?.status === "completed" || commentaryOutput?.status === "success"}
      <CheckCircle2 class="h-3.5 w-3.5" />
    {:else if commentaryOutput?.status === "processing" || commentaryOutput?.status === "synthesizing" || commentaryOutput?.status === "rendering"}
      <Loader2 class="h-3.5 w-3.5 animate-spin" />
    {:else if commentaryOutput?.status === "failed"}
      <AlertCircle class="h-3.5 w-3.5" />
    {:else}
      <Clock class="h-3.5 w-3.5" />
    {/if}
    <span>解说版: {getStatusBadge(commentaryOutput?.status).label}</span>
    {#if commentaryOutput?.project_id && onOpenProject}
      <button
        type="button"
        class="ml-1 hover:underline inline-flex items-center gap-0.5 text-accent"
        onclick={() => onOpenProject(commentaryOutput!.project_id!)}
        title="打开解说切片工程"
      >
        <ExternalLink class="h-3 w-3" />
      </button>
    {/if}
  </div>
</div>
