<script lang="ts">
  import { createStateRefresh, watchStateChanges } from "../lib/state-refresh";
  import { usePageVisibility } from "../lib/page-visibility";
  import { formatSize } from "../lib/size-format";

  import { get_static_url, invoke } from "../lib/invoker";
  import type { RecorderList, DiskInfo } from "../lib/interface";
  import type { RecordItem } from "../lib/db";
  import { onMount } from "svelte";
  import PageTitlebar from "../lib/components/PageTitlebar.svelte";
  import { scale } from "svelte/transition";
  import {
    ChevronRight,
    Film,
    HardDrive,
    Play,
    Plus,
    RefreshCw,
    Trash2,
    Users,
  } from "lucide-svelte";

  let summary: RecorderList = $state({
    count: 0,
    recorders: [],
  });

  let disk_info: DiskInfo = $state({
    disk: "",
    total: 0,
    free: 0,
  });

  let total = $state(0);
  let online = $state(0);
  let disk_usage = $state(0);
  let account_count = $state(0);
  let total_length = $state(0);
  let today_record_count = $state(0);
  let recent_records: RecordItem[] = $state([]);
  let activeDropdown = $state<string | null>(null);
  let loading = $state(false);
  let offset = 0;
  let hasMore = $state(true);
  let hasNewRecords = $state(false);
  let avatarFailed: Record<string, boolean> = $state({});
  const RECORDS_PER_PAGE = 5;

  const pageVisibility = usePageVisibility();
  const stateRefresh = createStateRefresh(update_summary);

  /** 当前在线直播间列表。 */
  let liveRooms = $derived(summary.recorders.filter((r) => r.room_info.status));

  /** 当前正在缓存的直播间数量（开播且已开启自动录制）。 */
  let recordingRoomsCount = $derived(
    summary.recorders.filter((r) => r.room_info.status && r.enabled).length,
  );

  /** 磁盘已使用百分比。 */
  let diskUsagePercent = $derived(
    disk_info.total > 0
      ? Math.min(
          100,
          Math.max(
            0,
            Math.round(
              ((disk_info.total - disk_info.free) / disk_info.total) * 100,
            ),
          ),
        )
      : 0,
  );

  /**
   * 刷新总览核心统计数据。
   */
  async function update_summary() {
    summary = (await invoke("get_recorder_list")) as RecorderList;
    total = summary.count;
    online = summary.recorders.filter((r) => r.room_info.status).length;

    disk_usage = await get_archive_disk_usage();
    disk_info = await invoke("get_disk_info");
    account_count = await get_account_count();
    total_length = await get_total_length();
    today_record_count = await get_today_record_count();

    // 检查是否有新增录制记录
    if (recent_records.length > 0) {
      const latestRecords = (await invoke("get_recent_record", {
        roomId: "",
        offset: 0,
        limit: 1,
      })) as RecordItem[];

      if (
        latestRecords.length > 0 &&
        (!recent_records[0] ||
          latestRecords[0].live_id !== recent_records[0].live_id)
      ) {
        hasNewRecords = true;
      }
    } else {
      await loadMoreRecords();
    }
  }

  /**
   * 分页加载或刷新最近录播记录。
   */
  async function loadMoreRecords() {
    if (loading || (!hasMore && !hasNewRecords)) return;

    loading = true;
    const newRecords = (await invoke("get_recent_record", {
      roomId: "",
      offset: hasNewRecords ? 0 : offset,
      limit: RECORDS_PER_PAGE,
    })) as RecordItem[];

    for (const record of newRecords) {
      record.cover = await get_static_url(
        "cache",
        `${record.platform}/${record.room_id}/${record.live_id}/cover.jpg`,
      );
    }

    if (hasNewRecords) {
      recent_records = newRecords;
      offset = newRecords.length;
      hasNewRecords = false;
      hasMore = true;
    } else {
      if (newRecords.length < RECORDS_PER_PAGE) {
        hasMore = false;
      }
      recent_records = [...recent_records, ...newRecords];
      offset += newRecords.length;
    }

    loading = false;
  }

  /**
   * 监听录播列表滚动事件，触底自动追加下一页。
   */
  function handleScroll(event: Event) {
    const target = event.target as HTMLElement;
    if (target.scrollTop === 0 && hasNewRecords) {
      loadMoreRecords();
      return;
    }

    const bottom =
      target.scrollHeight - target.scrollTop - target.clientHeight < 50;
    if (bottom && !hasNewRecords) {
      loadMoreRecords();
    }
  }

  onMount(() =>
    watchStateChanges(
      stateRefresh,
      ["recorders-changed", "records-changed"],
      30_000,
      pageVisibility,
    ),
  );

  /**
   * 查询归档录播占用的磁盘大小（字节）。
   */
  async function get_archive_disk_usage() {
    return (await invoke("get_archive_disk_usage")) as number;
  }

  /**
   * 获取累计录制总时长（秒）。
   */
  async function get_total_length(): Promise<number> {
    return await invoke("get_total_length");
  }

  /**
   * 获取今日录播场数。
   */
  async function get_today_record_count(): Promise<number> {
    return await invoke("get_today_record_count");
  }

  /**
   * 获取已配置的平台账号数。
   */
  async function get_account_count(): Promise<number> {
    return await invoke("get_account_count");
  }

  /**
   * 将秒数格式化为 HH:MM:SS。
   */
  function format_time(time: number) {
    time = Math.round(time);
    const hours = Math.floor(time / 3600);
    const minutes = Math.floor((time % 3600) / 60);
    const seconds = time % 60;
    return `${hours.toString().padStart(2, "0")}:${minutes
      .toString()
      .padStart(2, "0")}:${seconds.toString().padStart(2, "0")}`;
  }

  /**
   * 将秒数转换为整数小时数展示。
   */
  function format_hours(time: number) {
    return Math.floor(time / 3600);
  }

  /**
   * 格式化本地日期时间。
   */
  function format_date(date: string) {
    return new Date(date).toLocaleString();
  }

  /**
   * 切换指定录播卡片的删除确认下拉浮层。
   */
  function toggleDropdown(id: string) {
    if (activeDropdown === id) {
      activeDropdown = null;
    } else {
      activeDropdown = id;
    }
  }

  /**
   * 点击外部空白区域自动关闭删除下拉框。
   */
  function handleClickOutside(event: MouseEvent) {
    if (
      activeDropdown !== null &&
      !(event.target as HTMLElement).closest(".dropdown-container")
    ) {
      activeDropdown = null;
    }
  }

  /**
   * 删除指定录播归档并更新本地统计数据。
   */
  async function deleteRecord(record: RecordItem) {
    try {
      await invoke("delete_archive", {
        platform: record.platform,
        roomId: record.room_id,
        liveId: record.live_id,
      });

      recent_records = recent_records.filter(
        (r) => r.live_id !== record.live_id,
      );

      disk_usage -= record.size;
      total_length -= record.length;
      if (
        new Date(record.created_at).toDateString() === new Date().toDateString()
      ) {
        today_record_count--;
      }
    } catch (error) {
      alert(error);
    }
  }

  /**
   * 从头重新拉取最近录播列表。
   */
  async function refreshRecords() {
    offset = 0;
    hasMore = true;
    recent_records = [];
    await loadMoreRecords();
  }

  /**
   * 跳转至指定页面。
   */
  function navigatePage(page: string) {
    window.dispatchEvent(new CustomEvent("navigate-page", { detail: page }));
  }

  /**
   * 快捷打开添加直播间弹窗（联动跳转直播间页）。
   */
  function handleAddRoom() {
    navigatePage("直播间");
    window.dispatchEvent(new CustomEvent("open-add-room"));
  }
</script>

<svelte:window onclick={handleClickOutside} />

<PageTitlebar title="总览" />

<div
  class="flex-1 overflow-y-auto bg-canvas p-6 custom-scrollbar-light"
  onscroll={handleScroll}
>
  <div class="space-y-6 max-w-[1400px] mx-auto">
    <!-- 页头标题与快捷添加直播间入口 -->
    <div class="flex items-center justify-between gap-4 flex-wrap">
      <div>
        <h1 class="text-[21px] font-semibold tracking-tight text-fg">总览</h1>
        <p class="mt-1 text-[12px] text-fg-muted">
          掌握录制状态，继续处理最近的内容。
        </p>
      </div>
      <button
        type="button"
        class="ui-btn ui-btn-primary"
        onclick={handleAddRoom}
      >
        <Plus class="h-4 w-4" />
        <span>添加直播间</span>
      </button>
    </div>

    <!-- 顶部三联核心指标条 -->
    <div
      class="ui-card grid grid-cols-1 md:grid-cols-[1.1fr_1fr_1fr] py-4 divide-y md:divide-y-0 md:divide-x divide-line"
      aria-label="录制概况"
    >
      <!-- 直播间概况 -->
      <div class="px-5 py-2 md:py-0">
        <div class="text-[12px] text-fg-muted">直播间</div>
        <div
          class="my-1.5 flex items-baseline gap-1.5 text-[26px] font-semibold text-fg tabular-nums tracking-tight"
        >
          <span>{online}</span>
          <small class="text-[13px] font-normal text-fg-muted"
            >/ {total} 个在线</small
          >
        </div>
        <div
          class="flex items-center gap-1.5 text-[11px] {recordingRoomsCount > 0
            ? 'text-success-fg'
            : 'text-fg-muted'}"
        >
          <span
            class="h-1.5 w-1.5 rounded-full {recordingRoomsCount > 0
              ? 'bg-success'
              : 'bg-fg-muted/60'}"
          ></span>
          <span>正在缓存 {recordingRoomsCount} 个直播间</span>
        </div>
      </div>

      <!-- 总缓存时长 -->
      <div class="px-5 py-2 md:py-0">
        <div class="text-[12px] text-fg-muted">总缓存时长</div>
        <div
          class="my-1.5 flex items-baseline gap-1.5 text-[26px] font-semibold text-fg tabular-nums tracking-tight"
        >
          <span>{format_hours(total_length)}</span>
          <small class="text-[13px] font-normal text-fg-muted">小时</small>
        </div>
        <div class="text-[11px] text-fg-muted">累计录制时长（{format_time(total_length)}）</div>
      </div>

      <!-- 今日录播场数 -->
      <div class="px-5 py-2 md:py-0">
        <div class="text-[12px] text-fg-muted">今日录播</div>
        <div
          class="my-1.5 flex items-baseline gap-1.5 text-[26px] font-semibold text-fg tabular-nums tracking-tight"
        >
          <span>{today_record_count}</span>
          <small class="text-[13px] font-normal text-fg-muted">场</small>
        </div>
        <div class="text-[11px] text-fg-muted">按录制开始时间统计</div>
      </div>
    </div>

    <!-- 下方主副双栏布局 -->
    <div class="grid grid-cols-1 lg:grid-cols-[minmax(0,1fr)_220px] gap-5 items-start">
      <!-- 左主栏：最近录播 + 直播中的房间 -->
      <div class="min-w-0 space-y-6">
        <!-- 最近录播列表 -->
        <div>
          <div class="flex items-center justify-between gap-2 mb-3">
            <h2 class="text-[14px] font-semibold text-fg">最近录播</h2>
            <div class="flex items-center gap-2">
              <button
                type="button"
                class="text-[12px] text-accent-fg hover:underline cursor-pointer"
                onclick={() => navigatePage("录播")}
              >
                全部录播
              </button>
              <button
                type="button"
                class="ui-btn ui-btn-ghost ui-btn-sm ui-btn-icon"
                title="刷新列表"
                onclick={refreshRecords}
              >
                <RefreshCw class="h-3.5 w-3.5" />
              </button>
            </div>
          </div>

          <div class="ui-card divide-y divide-line overflow-hidden">
            {#if hasNewRecords}
              <div class="p-2 border-b border-line bg-accent-soft text-center">
                <button
                  type="button"
                  class="text-[11px] font-medium text-accent-fg hover:underline cursor-pointer"
                  onclick={loadMoreRecords}
                >
                  记录有更新 • 点击刷新
                </button>
              </div>
            {/if}

            {#each recent_records as record (record.live_id)}
              <article
                class="grid grid-cols-[68px_minmax(0,1fr)_auto] gap-3 items-center p-3 hover:bg-control-hover/40 transition-colors"
              >
                <!-- 封面缩略图 -->
                {#if record.cover}
                  <img
                    src={record.cover}
                    alt=""
                    class="h-11 w-[68px] rounded-[6px] border border-line object-cover bg-sunken"
                  />
                {:else}
                  <div
                    class="flex h-11 w-[68px] items-center justify-center rounded-[6px] bg-sunken text-fg-muted"
                  >
                    <Film class="h-5 w-5 opacity-60" />
                  </div>
                {/if}

                <!-- 录播元信息 -->
                <div class="min-w-0">
                  <h3 class="truncate text-[13px] font-medium text-fg leading-snug">
                    {record.title}
                  </h3>
                  <div class="mt-1 flex flex-wrap items-center gap-2 text-[11px] text-fg-muted">
                    <span>{record.platform}</span>
                    <span>•</span>
                    <span>#{record.room_id}</span>
                    <span>•</span>
                    <span>{format_date(record.created_at)}</span>
                  </div>
                  <div class="mt-0.5 flex flex-wrap items-center gap-2 text-[11px] text-fg-faint">
                    <span>时长 {format_time(record.length)}</span>
                    <span>•</span>
                    <span>大小 {formatSize(record.size)}</span>
                  </div>
                </div>

                <!-- 回看与删除操作 -->
                <div class="flex items-center gap-1 shrink-0">
                  <button
                    type="button"
                    class="ui-btn ui-btn-ghost ui-btn-sm ui-btn-icon"
                    title="回看直播"
                    onclick={() => {
                      invoke("open_live", {
                        platform: record.platform,
                        roomId: record.room_id,
                        liveId: record.live_id,
                      });
                    }}
                  >
                    <Play class="h-3.5 w-3.5" />
                  </button>
                  <div class="relative dropdown-container">
                    <button
                      type="button"
                      class="ui-btn ui-btn-ghost ui-btn-sm ui-btn-icon"
                      title="删除记录"
                      onclick={(e) => {
                        e.stopPropagation();
                        toggleDropdown(record.live_id);
                      }}
                    >
                      <Trash2 class="h-3.5 w-3.5 text-danger" />
                    </button>
                    {#if activeDropdown === record.live_id}
                      <div
                        class="ui-menu absolute right-0 z-50 mt-1 w-48"
                        style="transform-origin: top right;"
                        in:scale={{ duration: 100, start: 0.95 }}
                        out:scale={{ duration: 100, start: 0.95 }}
                      >
                        <div class="border-b border-line px-2 pb-2 pt-1.5">
                          <h3 class="text-[12px] font-medium text-fg">确认删除</h3>
                          <p class="ui-hint mt-0.5">此操作无法撤销</p>
                        </div>
                        <div class="flex gap-1.5 p-1 pt-1.5">
                          <button
                            type="button"
                            class="ui-btn ui-btn-sm flex-1"
                            onclick={() => (activeDropdown = null)}
                          >
                            取消
                          </button>
                          <button
                            type="button"
                            class="ui-btn ui-btn-sm ui-btn-danger flex-1"
                            onclick={() => {
                              deleteRecord(record);
                              activeDropdown = null;
                            }}
                          >
                            删除
                          </button>
                        </div>
                      </div>
                    {/if}
                  </div>
                </div>
              </article>
            {/each}

            {#if loading}
              <div class="flex justify-center py-4">
                <div
                  class="h-5 w-5 animate-spin rounded-full border-2 border-accent border-t-transparent"
                ></div>
              </div>
            {/if}

            {#if recent_records.length === 0 && !loading}
              <div class="py-8 text-center text-[12px] text-fg-muted">
                暂无最近录播记录
              </div>
            {/if}

            {#if !hasMore && recent_records.length > 0}
              <div class="py-3 text-center text-[11px] text-fg-faint border-t border-line">
                没有更多记录了
              </div>
            {/if}
          </div>
        </div>

        <!-- 直播中的房间快捷卡片区 -->
        <div>
          <div class="flex items-center justify-between gap-2 mb-3">
            <h2 class="text-[14px] font-semibold text-fg">直播中的房间</h2>
            <button
              type="button"
              class="text-[12px] text-accent-fg hover:underline cursor-pointer"
              onclick={() => navigatePage("直播间")}
            >
              查看全部
            </button>
          </div>

          {#if liveRooms.length > 0}
            <div class="grid grid-cols-1 sm:grid-cols-2 gap-2.5">
              {#each liveRooms as room (`${room.room_info.platform}:${room.room_info.room_id}`)}
                {@const roomKey = `${room.room_info.platform}:${room.room_info.room_id}`}
                <button
                  type="button"
                  class="flex items-center gap-2.5 p-2.5 rounded-[8px] border border-line bg-surface hover:bg-control-hover text-left transition-colors cursor-pointer"
                  onclick={() => navigatePage("直播间")}
                >
                  {#if room.user_info.user_avatar && !avatarFailed[roomKey]}
                    <img
                      src={room.user_info.user_avatar}
                      alt=""
                      referrerpolicy="no-referrer"
                      onerror={() => {
                        avatarFailed[roomKey] = true;
                      }}
                      class="h-8 w-8 rounded-full object-cover shrink-0 border border-line bg-sunken"
                    />
                  {:else}
                    <div
                      class="h-8 w-8 rounded-full bg-accent-soft text-accent-fg flex items-center justify-center text-[12px] font-semibold shrink-0"
                    >
                      {room.user_info.user_name.slice(0, 1)}
                    </div>
                  {/if}
                  <div class="min-w-0 flex-1">
                    <div class="truncate text-[12px] font-medium text-fg">
                      {room.user_info.user_name}
                    </div>
                    <div class="truncate text-[11px] text-fg-muted">
                      {room.room_info.platform} · {room.room_info.room_title || "正在直播"}
                    </div>
                  </div>
                  <span
                    class="h-2 w-2 rounded-full bg-success shrink-0"
                    title="直播中"
                  ></span>
                </button>
              {/each}
            </div>
          {:else}
            <div class="ui-card p-6 text-center text-[12px] text-fg-muted">
              当前暂无正在直播的房间
            </div>
          {/if}
        </div>
      </div>

      <!-- 右侧栏：存储空间与账号入口 -->
      <aside class="ui-card p-4 space-y-3.5" aria-label="存储与账号">
        <div class="flex items-center gap-2 text-fg">
          <HardDrive class="h-4 w-4 text-accent-fg" />
          <h2 class="text-[13px] font-semibold">存储空间</h2>
        </div>
        <div>
          <div class="text-[22px] font-semibold text-fg tabular-nums leading-tight">
            {formatSize(disk_usage)}
          </div>
          <div class="mt-0.5 text-[11px] text-fg-muted">录播缓存占用</div>
        </div>

        <div>
          <div class="h-1.5 w-full rounded-full bg-sunken overflow-hidden">
            <div
              class="h-full rounded-full bg-accent transition-all duration-300"
              style:width="{diskUsagePercent}%"
            ></div>
          </div>
          <div class="mt-2 flex justify-between text-[11px] text-fg-muted">
            <span>磁盘已使用 {diskUsagePercent}%</span>
          </div>
          <div class="mt-1 flex justify-between text-[11px] text-fg-muted">
            <span>剩余空间</span>
            <span class="tabular-nums font-medium text-fg">{formatSize(disk_info.free)}</span>
          </div>
        </div>

        <button
          type="button"
          class="w-full flex items-center gap-2 pt-3 border-t border-line text-[12px] text-fg-muted hover:text-fg text-left transition-colors cursor-pointer"
          onclick={() => navigatePage("账号")}
        >
          <Users class="h-4 w-4 shrink-0" />
          <span class="truncate">
            已连接 <strong class="font-semibold text-fg">{account_count}</strong> 个账号
          </span>
          <ChevronRight class="h-4 w-4 ml-auto shrink-0 text-fg-faint" />
        </button>
      </aside>
    </div>
  </div>
</div>
