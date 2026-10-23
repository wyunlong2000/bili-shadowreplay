<script module lang="ts">
  export const SIDEBAR_WIDTH = 192;
  /** 折叠状态下侧边栏固定宽度（图标居中排列）。 */
  export const SIDEBAR_RAIL_WIDTH = 52;
</script>

<script lang="ts">
  import { Command } from "lucide-svelte";
  import { PALETTE_SHORTCUT } from "../command-palette";
  import { navGroups } from "../pages";
  import { openCommandPalette } from "../stores/commands";
  import { hasNewVersion } from "../stores/version";
  import {
    MAC_TRAFFIC_LIGHTS,
    TRAFFIC_LIGHTS_INSET,
    windowDrag,
    windowState,
  } from "../window-chrome";
  import SidebarItem from "./SidebarItem.svelte";
  import SidebarToggle from "./SidebarToggle.svelte";

  interface Props {
    activeUrl?: string;
    collapsed?: boolean;
    counts?: Record<string, number | null | undefined>;
    onActiveChange?: (label: string) => void;
    onToggleCollapsed?: () => void;
  }

  let {
    activeUrl = $bindable("总览"),
    collapsed = false,
    counts = {},
    onActiveChange,
    onToggleCollapsed,
  }: Props = $props();

  // macOS 下红绿灯避让间距
  let lightsInset = $derived(MAC_TRAFFIC_LIGHTS && !$windowState.fullscreen);

  /**
   * 切换当前激活页面。
   * @param label 目标页面名称
   */
  function navigate(label: string) {
    activeUrl = label;
    onActiveChange?.(label);
  }

  // 常规顶部导航分组与置底应用管理分组
  let topGroups = $derived(navGroups.filter((g) => !g.isTail));
  let tailGroup = $derived(navGroups.find((g) => g.isTail));
</script>

<div
  class="flex shrink-0 flex-col overflow-hidden bg-chrome transition-[width] duration-200"
  style:width="{collapsed ? SIDEBAR_RAIL_WIDTH : SIDEBAR_WIDTH}px"
>
  <!-- 侧边栏标题栏区域，适配 macOS 红绿灯与窗口拖拽 -->
  <div
    class="flex h-10 shrink-0 items-center gap-2 overflow-hidden pr-3"
    style:padding-left="{lightsInset ? TRAFFIC_LIGHTS_INSET : 12}px"
    use:windowDrag
  >
    {#if !(MAC_TRAFFIC_LIGHTS && collapsed)}
      <SidebarToggle {collapsed} onToggle={() => onToggleCollapsed?.()} />
    {/if}
    {#if !MAC_TRAFFIC_LIGHTS && !collapsed}
      <img src="/icon.svg" alt="" class="h-5 w-5 shrink-0" />
      <span class="truncate text-[13px] font-semibold text-fg">
        ShadowReplay
      </span>
    {/if}
  </div>

  <!-- 分割线从标题行下方开始延伸 -->
  <div class="flex min-h-0 flex-1 flex-col border-r border-line px-2 pb-3 pt-1">
    <div class="flex flex-col gap-2.5 overflow-y-auto overflow-x-hidden custom-scrollbar-light">
      {#each topGroups as group (group.title)}
        <nav class="space-y-0.5">
          {#if !collapsed}
            <div class="select-none px-2.5 pb-1 pt-1 text-[11px] font-medium text-fg-muted">
              {group.title}
            </div>
          {/if}
          {#each group.pages as page (page.label)}
            <SidebarItem
              label={page.label}
              {activeUrl}
              {collapsed}
              count={counts[page.label]}
              onActiveChange={navigate}
              dot={page.label === "关于" && $hasNewVersion}
            >
              {#snippet icon()}
                <page.icon class="h-4 w-4 shrink-0" />
              {/snippet}
            </SidebarItem>
          {/each}
        </nav>
      {/each}
    </div>

    <!-- 置底分组：应用管理 -->
    {#if tailGroup}
      <nav class="mt-auto border-t border-line pt-2 space-y-0.5">
        {#each tailGroup.pages as page (page.label)}
          <SidebarItem
            label={page.label}
            {activeUrl}
            {collapsed}
            count={counts[page.label]}
            onActiveChange={navigate}
            dot={page.label === "关于" && $hasNewVersion}
          >
            {#snippet icon()}
              <page.icon class="h-4 w-4 shrink-0" />
            {/snippet}
          </SidebarItem>
        {/each}
      </nav>
    {/if}

    <!-- 命令面板入口，折叠时仅展示图标 -->
    <button
      type="button"
      class="mt-1 flex h-8 w-full items-center gap-2.5 overflow-hidden whitespace-nowrap rounded-[8px] px-2.5 text-[13px] text-fg-muted transition-colors hover:bg-control-hover hover:text-fg"
      title="命令面板（{PALETTE_SHORTCUT}）"
      onclick={openCommandPalette}
    >
      <Command class="h-4 w-4 shrink-0" />
      {#if !collapsed}
        <span>命令面板</span>
        <kbd class="ml-auto font-sans text-[10px] text-fg-faint">{PALETTE_SHORTCUT}</kbd>
      {/if}
    </button>
  </div>
</div>

<style>
  :global(img.text-\[\#0A84FF\]) {
    filter: invert(48%) sepia(85%) saturate(2229%) hue-rotate(198deg)
      brightness(100%) contrast(101%);
  }

  :global(img.text-gray-700) {
    filter: invert(23%) sepia(10%) saturate(532%) hue-rotate(182deg)
      brightness(94%) contrast(90%);
  }

  :global(.dark img.text-\[\#0A84FF\]) {
    filter: invert(48%) sepia(85%) saturate(2229%) hue-rotate(198deg)
      brightness(100%) contrast(101%);
  }
</style>
