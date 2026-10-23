<script lang="ts">
  interface Props {
    activeUrl?: string;
    label?: string;
    count?: number | null;
    /** 仅展示图标；折叠时截断标签并作为悬停提示展示。 */
    collapsed?: boolean;
    dot?: boolean;
    icon?: import("svelte").Snippet<[any]>;
    onActiveChange?: (label: string) => void;
  }

  let {
    activeUrl = "总览",
    label = "",
    count = undefined,
    collapsed = false,
    dot = false,
    icon,
    onActiveChange,
  }: Props = $props();
</script>

<!-- 图标在展开与折叠时固定对齐位置，折叠时仅隐藏文字和数量角标。 -->
<button
  type="button"
  onclick={() => onActiveChange?.(label)}
  title={collapsed ? label : undefined}
  class="relative flex h-8 w-full items-center gap-2.5 overflow-hidden whitespace-nowrap rounded-[8px] px-2.5 text-[13px] transition-colors {activeUrl ===
  label
    ? 'bg-accent-soft font-medium text-accent-fg'
    : 'text-fg-secondary hover:bg-control-hover hover:text-fg'}"
>
  {@render icon?.({})}
  {#if !collapsed}
    <span class="truncate">{label}</span>
    {#if count !== undefined && count !== null && count > 0}
      <span class="ml-auto text-[11px] text-fg-muted tabular-nums">{count}</span>
    {/if}
  {/if}
  {#if dot}
    <span
      class="h-1.5 w-1.5 shrink-0 rounded-full bg-danger {collapsed
        ? 'absolute left-[23px] top-[7px]'
        : 'ml-auto'}"
    ></span>
  {/if}
</button>
