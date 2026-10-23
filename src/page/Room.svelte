<script lang="ts">
  import { createStateRefresh, watchStateChanges } from "../lib/state-refresh";
  import { usePageVisibility } from "../lib/page-visibility";
  import { formatSize } from "../lib/size-format";
  import { liveRoomUrl } from "../lib/platform";
  import { invoke, open, onOpenUrl, get_static_url, get } from "../lib/invoker";
  import { message } from "@tauri-apps/plugin-dialog";
  import { fade, scale } from "svelte/transition";
  import type { RecorderList, RecorderInfo } from "../lib/interface";
  import type { RecordItem } from "../lib/db";
  import {
    Ellipsis,
    FileVideo,
    Globe,
    History,
    Play,
    Plus,
    Search,
    Sparkles,
    Trash2,
    X,
  } from "lucide-svelte";
  import PlatformIconTabs from "../lib/components/PlatformIconTabs.svelte";
  import GenerateWholeClipModal from "../lib/components/GenerateWholeClipModal.svelte";
  import RoomAutomationDialog from "../lib/components/automation/RoomAutomationDialog.svelte";
  import PageTitlebar from "../lib/components/PageTitlebar.svelte";
  import { getAutomationPolicy } from "../lib/automation";
  import { registerCommands } from "../lib/stores/commands";
  import { onMount } from "svelte";

  interface Props {
    room_count?: number;
  }

  let { room_count = $bindable(0) }: Props = $props();
  let room_active = $state(0);
  let room_inactive = $state(0);
  let room_unknown = $state(0);
  let room_unconfirmed = $state(0);

  type DetectionStatus = "live" | "offline" | "unknown" | "unconfirmed";

  let selectedAutomationRoom: RecorderInfo | null = $state(null);
  let showAutomationDialog = $state(false);
  let openMenuRoomKey: string | null = $state(null);
  let searchInput: HTMLInputElement | null = $state(null);
  let autoPoliciesSeq = 0;

  let summary: RecorderList = $state({
    count: 0,
    recorders: [],
  });

  /** 房间自动切片开启状态缓存表。 */
  let roomAutoPolicies: Map<string, boolean> = $state(new Map());

  // 平台筛选
  const platformFilters = [
    { id: "all", label: "全部平台" },
    { id: "bilibili", label: "B 站" },
    { id: "douyin", label: "抖音" },
    { id: "huya", label: "虎牙" },
    { id: "douyu", label: "斗鱼" },
    { id: "kuaishou", label: "快手" },
    { id: "tiktok", label: "TikTok" },
    { id: "twitch", label: "Twitch" },
    { id: "youtube", label: "YouTube" },
  ];
  let selectedPlatformFilter = $state("all");
  let searchQuery = $state("");

  /** 根据平台筛选与搜索词过滤直播间列表。 */
  let filteredRecorders = $derived(
    summary.recorders.filter((room) => {
      const matchesPlatform =
        selectedPlatformFilter === "all" ||
        room.room_info.platform === selectedPlatformFilter;
      if (!matchesPlatform) return false;

      const query = searchQuery.trim().toLowerCase();
      if (!query) return true;

      return (
        room.room_info.room_title.toLowerCase().includes(query) ||
        room.user_info.user_name.toLowerCase().includes(query) ||
        room.room_info.room_id.toString().toLowerCase().includes(query)
      );
    }),
  );

  /** 返回主播当前可展示的检测状态。 */
  function getDetectionStatus(room: RecorderInfo): DetectionStatus {
    if (room.room_info.platform !== "douyin") {
      return room.room_info.status ? "live" : "offline";
    }
    switch (room.detection_status) {
      case "live":
      case "offline":
      case "unknown":
      case "unconfirmed":
        return room.detection_status;
      default:
        return room.room_info.status ? "live" : "unconfirmed";
    }
  }

  /** 将检测状态转换为卡片上的中文标签。 */
  function getDetectionLabel(status: DetectionStatus) {
    switch (status) {
      case "live":
        return "直播中";
      case "offline":
        return "未直播";
      case "unconfirmed":
        return "等待首次检测";
      default:
        return "监测状态未知";
    }
  }

  /** 返回监测异常原因和下次重试时间。 */
  function getDetectionErrorMessage(room: RecorderInfo) {
    const errorLabels: Record<string, string> = {
      network: "网络异常",
      rate_limited: "请求受限",
      request_rejected: "接口拒绝请求",
      account_invalid: "账号无效",
      verification: "需要验证",
      parse: "响应格式异常",
    };
    const error =
      errorLabels[room.detection_error_kind] || "最近一次检测未成功";
    const retryAt =
      room.next_retry_at > 0
        ? `；下次重试：${new Date(room.next_retry_at * 1000).toLocaleString()}`
        : "";
    return `${error}${retryAt}`;
  }

  function default_avatar(platform: string) {
    const avatarMap: Record<string, string> = {
      bilibili: "/imgs/bilibili_avatar.png",
      douyin: "/imgs/douyin.png",
      douyu: "/imgs/douyu.svg",
      huya: "/imgs/huya_avatar.png",
      kuaishou: "/imgs/kuaishou.svg",
      tiktok: "/imgs/tiktok.png",
      twitch: "/imgs/twitch.svg",
      youtube: "/imgs/youtube.svg",
    };
    return avatarMap[platform] || "/imgs/huya_avatar.png";
  }

  function default_cover(platform: string) {
    const coverMap: Record<string, string> = {
      bilibili: "/imgs/bilibili.png",
      douyin: "/imgs/douyin.png",
      douyu: "/imgs/douyu.svg",
      huya: "/imgs/huya.png",
      kuaishou: "/imgs/kuaishou.svg",
      tiktok: "/imgs/tiktok.png",
      twitch: "/imgs/twitch.svg",
      youtube: "/imgs/youtube.svg",
    };
    return coverMap[platform] || "/imgs/huya.png";
  }

  const IMAGE_FETCH_TIMEOUT_MS = 10_000;
  const MAX_IMAGE_CACHE_ENTRIES = 200;
  let avatar_cache: Map<string, string> = new Map();
  let image_cache: Map<string, string> = new Map();
  let inflight_images: Map<string, Promise<string | null>> = new Map();

  /**
   * 写入图片 ObjectURL 缓存并在超过上限时释放最旧条目，防止长会话内存泄漏。
   */
  function setImageCacheEntry(
    cache: Map<string, string>,
    key: string,
    objectUrl: string,
  ) {
    if (cache.has(key)) {
      const prev = cache.get(key);
      cache.delete(key);
      if (prev && prev !== objectUrl) {
        URL.revokeObjectURL(prev);
      }
    } else if (cache.size >= MAX_IMAGE_CACHE_ENTRIES) {
      const oldestKey = cache.keys().next().value;
      if (oldestKey !== undefined) {
        const oldestUrl = cache.get(oldestKey);
        cache.delete(oldestKey);
        if (oldestUrl) {
          URL.revokeObjectURL(oldestUrl);
        }
      }
    }
    cache.set(key, objectUrl);
  }

  /**
   * 带超时控制地通过代理获取远程图片并转为 ObjectURL，失败或超时返回 null。
   */
  async function fetchImageObjectUrl(
    cache: Map<string, string>,
    cacheKey: string,
    url: string,
  ): Promise<string | null> {
    if (!url) return null;
    const cached = cache.get(cacheKey);
    if (cached) return cached;

    const inflightKey = `${cache === avatar_cache ? "avatar" : "cover"}:${cacheKey}:${url}`;
    const existing = inflight_images.get(inflightKey);
    if (existing) return existing;

    const task = (async (): Promise<string | null> => {
      let timeoutId: ReturnType<typeof setTimeout> | null = null;
      try {
        const timeoutPromise = new Promise<never>((_, reject) => {
          timeoutId = setTimeout(
            () => reject(new Error("图片加载超时")),
            IMAGE_FETCH_TIMEOUT_MS,
          );
        });
        const response = await Promise.race([get(url), timeoutPromise]);
        if (!response || !response.ok) {
          return null;
        }
        const blob = await Promise.race([response.blob(), timeoutPromise]);
        if (!blob || blob.size === 0) {
          return null;
        }
        const objectUrl = URL.createObjectURL(blob);
        setImageCacheEntry(cache, cacheKey, objectUrl);
        return objectUrl;
      } catch (error) {
        console.warn("加载直播间图片失败，已回退默认图:", error);
        return null;
      } finally {
        if (timeoutId !== null) {
          clearTimeout(timeoutId);
        }
        inflight_images.delete(inflightKey);
      }
    })();

    inflight_images.set(inflightKey, task);
    return task;
  }

  /**
   * 异步加载单个直播间的头像与封面，完成后按需更新对应卡片，不阻塞列表主刷新。
   */
  async function hydrateRoomImages(
    platform: string,
    roomId: string,
    userId: string,
    rawAvatarUrl: string,
    rawCoverUrl: string,
  ) {
    const avatarKey = userId
      ? `${platform}:${userId}:${rawAvatarUrl}`
      : `${platform}:${roomId}:${rawAvatarUrl}`;
    const avatarPromise = rawAvatarUrl
      ? fetchImageObjectUrl(avatar_cache, avatarKey, rawAvatarUrl)
      : Promise.resolve(null);
    const coverPromise = rawCoverUrl
      ? fetchImageObjectUrl(image_cache, rawCoverUrl, rawCoverUrl)
      : Promise.resolve(null);

    const [avatarUrl, coverUrl] = await Promise.all([
      avatarPromise,
      coverPromise,
    ]);
    if (!avatarUrl && !coverUrl) return;

    const target = summary.recorders.find(
      (r) =>
        r.room_info.platform === platform && r.room_info.room_id === roomId,
    );
    if (!target) return;

    if (avatarUrl) {
      target.user_info.user_avatar = avatarUrl;
    }
    if (coverUrl) {
      target.room_info.room_cover = coverUrl;
    } else if (!rawCoverUrl && avatarUrl) {
      target.room_info.room_cover = avatarUrl;
    }
  }

  /**
   * 批量加载各直播间的自动切片配置开关状态。
   */
  async function updateAutoPolicies(recorders: RecorderInfo[]) {
    const seq = ++autoPoliciesSeq;
    const map = new Map<string, boolean>();
    await Promise.all(
      recorders.map(async (r) => {
        try {
          const res = await getAutomationPolicy(
            r.room_info.platform,
            r.room_info.room_id,
          );
          if (res?.policy?.enabled) {
            map.set(`${r.room_info.platform}:${r.room_info.room_id}`, true);
          }
        } catch {
          // 忽略单个策略获取异常
        }
      }),
    );
    if (seq === autoPoliciesSeq) {
      roomAutoPolicies = map;
    }
  }

  const pageVisibility = usePageVisibility();
  const stateRefresh = createStateRefresh(update_summary);

  /**
   * 刷新直播间列表及统计信息。
   */
  async function update_summary() {
    let new_summary = (await invoke("get_recorder_list")) as RecorderList;
    room_count = new_summary.count;
    room_active = new_summary.recorders.filter(
      (room) => getDetectionStatus(room) === "live",
    ).length;
    room_inactive = new_summary.recorders.filter(
      (room) => getDetectionStatus(room) === "offline",
    ).length;
    room_unknown = new_summary.recorders.filter(
      (room) => getDetectionStatus(room) === "unknown",
    ).length;
    room_unconfirmed = new_summary.recorders.filter(
      (room) => getDetectionStatus(room) === "unconfirmed",
    ).length;

    // 按当前检测状态排序
    new_summary.recorders.sort((a, b) => {
      const statusOrder: DetectionStatus[] = [
        "live",
        "unknown",
        "unconfirmed",
        "offline",
      ];
      return (
        statusOrder.indexOf(getDetectionStatus(a)) -
        statusOrder.indexOf(getDetectionStatus(b))
      );
    });

    // 填充已缓存图片或默认图，未命中缓存的图片转入后台异步非阻塞加载
    const pendingHydrations: Array<() => void> = [];
    for (const room of new_summary.recorders) {
      const platform = room.room_info.platform;
      const roomId = room.room_info.room_id;
      const userId = room.user_info.user_id;
      const rawAvatar = room.user_info.user_avatar;
      const rawCover = room.room_info.room_cover;

      const avatarKey = userId
        ? `${platform}:${userId}:${rawAvatar}`
        : `${platform}:${roomId}:${rawAvatar}`;
      const cachedAvatar = rawAvatar ? avatar_cache.get(avatarKey) : null;
      const cachedCover = rawCover ? image_cache.get(rawCover) : null;

      room.user_info.user_avatar = cachedAvatar || default_avatar(platform);
      room.room_info.room_cover =
        cachedCover || (cachedAvatar ? cachedAvatar : default_cover(platform));

      if (
        (rawAvatar && !cachedAvatar) ||
        (rawCover && !cachedCover) ||
        (!rawCover && rawAvatar && !cachedAvatar)
      ) {
        pendingHydrations.push(() =>
          hydrateRoomImages(platform, roomId, userId, rawAvatar, rawCover),
        );
      }
    }

    summary = new_summary;

    for (const hydrate of pendingHydrations) {
      hydrate();
    }

    void updateAutoPolicies(new_summary.recorders);
  }

  onMount(() =>
    watchStateChanges(
      stateRefresh,
      ["recorders-changed"],
      30_000,
      pageVisibility,
    ),
  );

  let deleteModal = $state(false);
  let deleteRoom: RecorderInfo | null = $state(null);

  let addModal = $state(false);
  let addRoom = $state("");
  let selectedPlatform = $state("bilibili");

  let archiveModal = $state(false);
  let archiveRoom: RecorderInfo | null = $state(null);
  let archives: RecordItem[] = $state([]);
  let currentPage = $state(0);
  const pageSize = 20;
  let hasMore = $state(true);
  let isLoading = $state(false);
  let loadError = $state("");

  /**
   * 打开指定房间的历史录播归档弹窗。
   */
  async function showArchives(roomId: string) {
    currentPage = 0;
    archives = [];
    hasMore = true;
    loadError = "";
    await updateArchives();
    archiveModal = true;
  }

  /**
   * 分页拉取录播归档记录。
   */
  async function updateArchives() {
    if (isLoading || !hasMore || !archiveRoom) return;

    isLoading = true;
    try {
      let new_archives = (await invoke("get_archives", {
        roomId: archiveRoom.room_info.room_id,
        offset: currentPage * pageSize,
        limit: pageSize,
      })) as RecordItem[];

      for (const archive of new_archives) {
        archive.cover = await get_static_url(
          "cache",
          `${archive.platform}/${archive.room_id}/${archive.live_id}/cover.jpg`,
        );
      }

      if (currentPage === 0) {
        archives = new_archives;
      } else {
        archives = [...archives, ...new_archives];
      }

      hasMore = new_archives.length === pageSize;
      archives.sort((a, b) => {
        return (
          new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
        );
      });

      currentPage++;
    } catch (error) {
      console.error("加载录播归档失败:", error);
      loadError = "加载失败，请重试";
    } finally {
      isLoading = false;
    }
  }

  async function loadMoreArchives() {
    await updateArchives();
  }

  function handleArchiveScroll(event: Event) {
    const target = event.target as HTMLElement;
    const { scrollTop, scrollHeight, clientHeight } = target;

    if (
      scrollHeight - scrollTop - clientHeight < 100 &&
      hasMore &&
      !isLoading
    ) {
      loadMoreArchives();
    }
  }

  function format_ts(ts_string: string) {
    const date = new Date(ts_string);
    return date.toLocaleString();
  }

  function format_duration(duration: number) {
    duration = Math.round(duration);
    const hours = Math.floor(duration / 3600)
      .toString()
      .padStart(2, "0");
    const minutes = Math.floor((duration % 3600) / 60)
      .toString()
      .padStart(2, "0");
    const seconds = (duration % 60).toString().padStart(2, "0");
    return `${hours}:${minutes}:${seconds}`;
  }

  function calc_bitrate(size: number, duration: number) {
    if (duration <= 0) return "0";
    return ((size * 8) / duration / 1024).toFixed(0);
  }

  /**
   * 切换自动录制使能状态。
   */
  function toggleEnabled(room: RecorderInfo) {
    invoke("set_enable", {
      roomId: room.room_info.room_id,
      platform: room.room_info.platform,
      enabled: !room.enabled,
    });
  }

  /**
   * 打开主播空间或主页。
   */
  function openUserUrl(room: RecorderInfo) {
    if (room.room_info.platform === "bilibili") {
      open("https://space.bilibili.com/" + room.user_info.user_id);
    } else if (room.room_info.platform === "douyin") {
      open("https://www.douyin.com/user/" + room.user_info.user_id);
    } else if (room.room_info.platform === "douyu") {
      openLiveUrl(room);
    } else if (room.room_info.platform === "kuaishou") {
      if (room.user_info.user_id) {
        open("https://www.kuaishou.com/profile/" + room.user_info.user_id);
      } else {
        openLiveUrl(room);
      }
    } else if (room.room_info.platform === "tiktok") {
      if (room.user_info.user_id) {
        const handle = room.user_info.user_id.startsWith("@")
          ? room.user_info.user_id
          : `@${room.user_info.user_id}`;
        open(`https://www.tiktok.com/${handle}`);
      } else {
        openLiveUrl(room);
      }
    } else if (room.room_info.platform === "twitch") {
      openLiveUrl(room);
    } else if (room.room_info.platform === "youtube") {
      if (room.user_info.user_id) {
        open(`https://www.youtube.com/channel/${room.user_info.user_id}`);
      } else {
        openLiveUrl(room);
      }
    }
  }

  /**
   * 打开网页端直播间页面。
   */
  function openLiveUrl(room: RecorderInfo) {
    if (
      room.room_info.platform === "douyin" &&
      room.identifier_type === "profile"
    ) {
      if (room.room_resolved && room.web_rid) {
        open("https://live.douyin.com/" + room.web_rid);
      } else {
        openUserUrl(room);
      }
      return;
    }
    const url = liveRoomUrl(room.room_info.platform, room.room_info.room_id);
    if (url) open(url);
  }

  /**
   * 添加新录制直播间。
   */
  function addNewRecorder(room_id: string, platform: string) {
    const room_input = room_id.trim();
    const douyu_room_id =
      platform === "douyu"
        ? new URLSearchParams(
            room_input.split("?")[1]?.split("#")[0] || "",
          ).get("rid")
        : null;
    room_id =
      platform === "youtube" || platform === "douyin"
        ? room_input
        : room_input.split(/[?#]/)[0];
    if (douyu_room_id && /^\d+$/.test(douyu_room_id)) {
      room_id = douyu_room_id;
    }
    switch (platform) {
      case "bilibili":
        if (room_id.includes("https://") || room_id.includes("bsr://")) {
          room_id = room_id.split("/").pop() || "";
        }
        break;
      case "douyin":
        break;
      case "douyu":
        if (room_id.includes("://")) {
          room_id =
            room_id.split(/[?#]/)[0].replace(/\/+$/, "").split("/").pop() || "";
        }
        break;
      case "huya":
        if (room_id.includes("https://") || room_id.includes("bsr://")) {
          room_id = room_id.split("/").pop() || "";
        }
        break;
      case "kuaishou":
        if (room_id.includes("https://") || room_id.includes("bsr://")) {
          room_id = room_id.split("/").pop() || "";
        }
        break;
      case "tiktok":
        if (room_id.includes("https://") || room_id.includes("bsr://")) {
          room_id = "@" + room_id.split("@").pop()?.split("/")[0];
        }
        break;
      case "twitch":
        if (room_id.includes("https://") || room_id.includes("bsr://")) {
          room_id = room_id
            .replace(/^(?:https?:|bsr:)\/\/(?:(?:www|m)\.)?twitch\.tv\//, "")
            .split("/")[0];
        }
        room_id = room_id.replace(/^@/, "");
        break;
      case "youtube":
        if (room_id.startsWith("bsr://")) {
          room_id = room_id.replace("bsr://", "https://");
        }
        break;
    }

    invoke("add_recorder", {
      roomId: room_id,
      platform: platform,
      extra: "",
    })
      .then(() => {
        addModal = false;
        addRoom = "";
      })
      .catch(async (e) => {
        await message(e);
      });
  }

  let generateWholeClipModal = $state(false);
  let generateWholeClipArchive: RecordItem | null = $state(null);

  /**
   * 打开整场切片导出弹窗。
   */
  async function openGenerateWholeClipModal(archive: RecordItem) {
    generateWholeClipModal = true;
    generateWholeClipArchive = archive;
  }

  function handleWholeClipGenerated() {
    generateWholeClipModal = false;
    generateWholeClipArchive = null;
  }

  onMount(() => {
    // 监听深层链接
    void onOpenUrl((urls: string[]) => {
      console.log("收到 Deep Link:", urls);
      if (urls.length > 0) {
        const url = urls[0];
        let platform = "";

        if (url.startsWith("bsr://live.bilibili.com/")) platform = "bilibili";
        if (url.startsWith("bsr://live.douyin.com/")) platform = "douyin";
        if (
          url.startsWith("bsr://www.douyu.com/") ||
          url.startsWith("bsr://live.douyu.com/")
        )
          platform = "douyu";
        if (url.startsWith("bsr://live.kuaishou.com/")) platform = "kuaishou";
        if (url.startsWith("bsr://live.tiktok.com/")) platform = "tiktok";
        if (
          url.startsWith("bsr://www.twitch.tv/") ||
          url.startsWith("bsr://m.twitch.tv/") ||
          url.startsWith("bsr://twitch.tv/")
        )
          platform = "twitch";
        if (
          url.startsWith("bsr://www.youtube.com/") ||
          url.startsWith("bsr://youtube.com/") ||
          url.startsWith("bsr://youtu.be/")
        )
          platform = "youtube";

        if (url && platform) {
          addModal = true;
          addRoom = url;
          selectedPlatform = platform;
        }
      }
    });

    // 监听来自其他页面（如总览页）的添加直播间联动请求
    const handleOpenAdd = () => {
      addModal = true;
    };
    window.addEventListener("open-add-room", handleOpenAdd);

    // 监听全局点击以关闭卡片更多操作菜单
    const handleDocClick = () => {
      openMenuRoomKey = null;
    };
    window.addEventListener("click", handleDocClick);

    return () => {
      window.removeEventListener("open-add-room", handleOpenAdd);
      window.removeEventListener("click", handleDocClick);
    };
  });

  onMount(() =>
    registerCommands([
      {
        id: "room:add",
        group: "action",
        title: "添加直播间",
        detail: "直播间",
        icon: Plus,
        keywords: ["add room", "new room", "record"],
        page: "直播间",
        run: () => (addModal = true),
      },
      {
        id: "room:search",
        group: "action",
        title: "搜索直播间",
        detail: "直播间",
        icon: Search,
        keywords: ["search room", "find", "filter"],
        page: "直播间",
        run: () => searchInput?.focus({ preventScroll: true }),
      },
    ]),
  );
</script>

<PageTitlebar title="直播间" />

<div class="flex-1 overflow-y-auto bg-canvas p-6 custom-scrollbar-light">
  <div class="space-y-5 max-w-[1400px] mx-auto">
    <!-- 页头：标题、四态检测摘要与添加直播间按钮 -->
    <div class="flex items-center justify-between gap-4 flex-wrap">
      <div>
        <h1 class="text-[21px] font-semibold tracking-tight text-fg">直播间</h1>
        <p class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-[12px] text-fg-muted">
          <span class="flex items-center gap-1.5 text-success-fg">
            <span class="h-1.5 w-1.5 rounded-full bg-success"></span>
            <span>{room_active} 直播中</span>
          </span>
          <span>•</span>
          <span>{room_inactive} 未直播</span>
          {#if room_unknown > 0}
            <span>•</span>
            <span class="text-warning-fg">{room_unknown} 状态未知</span>
          {/if}
          {#if room_unconfirmed > 0}
            <span>•</span>
            <span>{room_unconfirmed} 待检测</span>
          {/if}
        </p>
      </div>
      <button
        type="button"
        class="ui-btn ui-btn-primary"
        onclick={() => (addModal = true)}
      >
        <Plus class="h-4 w-4" />
        <span>添加直播间</span>
      </button>
    </div>

    <!-- 顶部工具栏：平台筛选胶囊栏与搜索框 -->
    <div class="flex flex-wrap items-center justify-between gap-3 border-b border-line pb-3">
      <!-- 平台筛选胶囊栏 -->
      <div class="flex flex-wrap gap-1" role="tablist" aria-label="平台筛选">
        {#each platformFilters as filter}
          <button
            type="button"
            class="px-2.5 py-1 text-[12px] rounded-[6px] transition-colors cursor-pointer {selectedPlatformFilter === filter.id
              ? 'bg-accent-soft text-accent-fg font-medium'
              : 'text-fg-muted hover:text-fg hover:bg-control-hover'}"
            onclick={() => (selectedPlatformFilter = filter.id)}
          >
            {filter.label}
          </button>
        {/each}
      </div>

      <!-- 搜索输入框 -->
      <div class="relative w-full sm:w-64">
        <Search class="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-fg-muted" />
        <input
          bind:this={searchInput}
          type="search"
          bind:value={searchQuery}
          placeholder="搜索主播、标题或房间号"
          class="ui-input w-full pl-8"
          aria-label="搜索主播、标题或房间号"
        />
      </div>
    </div>

    <!-- 直播间卡片网格 -->
    {#if filteredRecorders.length > 0}
      <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
        {#each filteredRecorders as room (`${room.room_info.platform}:${room.room_info.room_id}`)}
          {@const detectionStatus = getDetectionStatus(room)}
          <article class="ui-card relative flex flex-col overflow-hidden hover:border-line-strong transition-colors">
            <!-- 封面媒体区域 -->
            <div class="relative h-40 w-full overflow-hidden bg-sunken">
              <img
                src={room.room_info.room_cover}
                alt="cover"
                class="h-full w-full object-cover transition-opacity {detectionStatus === 'offline'
                  ? 'opacity-70'
                  : ''}"
              />

              <!-- 四态检测徽章（左上角） -->
              <div class="absolute top-2 left-2 flex items-center">
                {#if detectionStatus === "live"}
                  <span class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-[5px] text-[11px] font-medium bg-surface/90 text-success-fg backdrop-blur shadow-sm">
                    <span class="h-1.5 w-1.5 rounded-full bg-success"></span>
                    <span>{getDetectionLabel(detectionStatus)}</span>
                  </span>
                {:else if detectionStatus === "offline"}
                  <span class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-[5px] text-[11px] font-medium bg-surface/90 text-fg-muted backdrop-blur shadow-sm">
                    <span class="h-1.5 w-1.5 rounded-full bg-fg-muted/60"></span>
                    <span>{getDetectionLabel(detectionStatus)}</span>
                  </span>
                {:else if detectionStatus === "unconfirmed"}
                  <span class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-[5px] text-[11px] font-medium bg-surface/90 text-fg-muted backdrop-blur shadow-sm">
                    <span class="h-1.5 w-1.5 rounded-full bg-fg-muted/60"></span>
                    <span>{getDetectionLabel(detectionStatus)}</span>
                  </span>
                {:else}
                  <span
                    class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-[5px] text-[11px] font-medium bg-warning-soft text-warning-fg backdrop-blur shadow-sm"
                    title={getDetectionErrorMessage(room)}
                  >
                    <span class="h-1.5 w-1.5 rounded-full bg-warning"></span>
                    <span>{getDetectionLabel(detectionStatus)}</span>
                  </span>
                {/if}
              </div>

              <!-- 房间号水印（左下角） -->
              <div class="absolute bottom-2 left-2 flex items-center">
                <span class="font-mono text-[10px] text-white/90 bg-black/60 px-1.5 py-0.5 rounded-[4px] backdrop-blur-sm tracking-tight">
                  {room.room_info.platform.toUpperCase()}#{room.room_info.room_id}
                </span>
              </div>

              <!-- 正在录制中提示（右下角） -->
              {#if room.recording}
                <div class="absolute bottom-2 right-2 flex items-center">
                  <span class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-[4px] bg-danger text-white text-[10px] font-medium shadow-sm">
                    <span class="h-1.5 w-1.5 rounded-full bg-white"></span>
                    <span>录制中</span>
                  </span>
                </div>
              {/if}

              <!-- 更多操作菜单按钮（右上角） -->
              <div class="absolute top-2 right-2">
                <button
                  type="button"
                  class="flex h-7 w-7 items-center justify-center rounded-[6px] bg-surface/85 text-fg-secondary hover:bg-surface hover:text-fg shadow-sm backdrop-blur transition-colors cursor-pointer"
                  onclick={(e) => {
                    e.stopPropagation();
                    const key = `${room.room_info.platform}:${room.room_info.room_id}`;
                    openMenuRoomKey = openMenuRoomKey === key ? null : key;
                  }}
                  aria-label="更多操作"
                >
                  <Ellipsis class="h-4 w-4" />
                </button>

                {#if openMenuRoomKey === `${room.room_info.platform}:${room.room_info.room_id}`}
                  <div
                    class="ui-menu absolute right-0 top-8 z-30 min-w-[145px]"
                    role="menu"
                    tabindex="-1"
                    onclick={(e) => e.stopPropagation()}
                    onkeydown={(e) => e.stopPropagation()}
                  >
                    <button
                      type="button"
                      class="ui-menu-item"
                      onclick={() => {
                        openMenuRoomKey = null;
                        selectedAutomationRoom = room;
                        showAutomationDialog = true;
                      }}
                    >
                      <Sparkles class="h-3.5 w-3.5 text-accent-fg" />
                      <span>自动切片设置</span>
                    </button>
                    <button
                      type="button"
                      class="ui-menu-item"
                      onclick={() => {
                        openMenuRoomKey = null;
                        openLiveUrl(room);
                      }}
                    >
                      <Globe class="h-3.5 w-3.5 text-fg-muted" />
                      <span>打开网页直播间</span>
                    </button>
                    <button
                      type="button"
                      class="ui-menu-item text-danger hover:bg-danger-soft hover:text-danger-fg"
                      onclick={() => {
                        openMenuRoomKey = null;
                        deleteRoom = room;
                        deleteModal = true;
                      }}
                    >
                      <Trash2 class="h-3.5 w-3.5" />
                      <span>移除直播间</span>
                    </button>
                  </div>
                {/if}
              </div>
            </div>

            <!-- 卡片内容区域 -->
            <div class="p-3.5 flex-1 flex flex-col justify-between">
              <div>
                <!-- 标题行与自动切片标签 -->
                <div class="flex items-center justify-between gap-2">
                  <h3
                    class="truncate text-[13px] font-semibold text-fg"
                    title={room.room_info.room_title}
                  >
                    {room.room_info.room_title || "未命名直播间"}
                  </h3>
                  {#if roomAutoPolicies.get(`${room.room_info.platform}:${room.room_info.room_id}`)}
                    <span class="ui-badge ui-badge-accent gap-1 shrink-0">
                      <Sparkles class="h-3 w-3" />
                      <span>自动切片</span>
                    </span>
                  {/if}
                </div>

                <!-- 主播信息与平台标签 -->
                <div class="flex items-center gap-2 text-[12px] text-fg-muted mt-2">
                  <button
                    type="button"
                    class="flex items-center gap-1.5 hover:text-fg transition-colors truncate text-left cursor-pointer"
                    onclick={() => openUserUrl(room)}
                  >
                    <img
                      src={room.user_info.user_avatar}
                      alt=""
                      class="h-5 w-5 rounded-full object-cover shrink-0 border border-line bg-sunken"
                    />
                    <span class="truncate font-medium">{room.user_info.user_name}</span>
                  </button>
                  <span class="ml-auto text-[11px] text-fg-faint shrink-0">
                    {room.room_info.platform}
                  </span>
                </div>

                <!-- 异常状态提示 -->
                {#if detectionStatus === "unknown"}
                  <p
                    class="mt-2 break-words text-[11px] text-warning-fg"
                    title={getDetectionErrorMessage(room)}
                  >
                    {getDetectionErrorMessage(room)}
                  </p>
                {/if}
              </div>

              <!-- 卡片底栏：快捷操作按钮与自动录制开关 -->
              <div class="flex flex-wrap items-center justify-between gap-2 border-t border-line pt-2.5 mt-3">
                <div class="flex items-center gap-1.5">
                  <button
                    type="button"
                    class="ui-btn ui-btn-ghost ui-btn-sm"
                    onclick={() => {
                      invoke("open_live", {
                        platform: room.room_info.platform,
                        roomId: room.room_info.room_id,
                        liveId: room.live_id,
                      });
                    }}
                  >
                    预览
                  </button>
                  <button
                    type="button"
                    class="ui-btn ui-btn-ghost ui-btn-sm"
                    onclick={() => {
                      archiveRoom = room;
                      showArchives(room.room_info.room_id);
                    }}
                  >
                    录播
                  </button>
                </div>
                <label class="flex items-center gap-2 cursor-pointer text-[12px] text-fg-muted select-none">
                  <span>自动录制</span>
                  <span class="ui-switch">
                    <input
                      type="checkbox"
                      checked={room.enabled}
                      onchange={() => toggleEnabled(room)}
                    />
                    <span></span>
                  </span>
                </label>
              </div>
            </div>
          </article>
        {/each}
      </div>
    {:else}
      <!-- 搜索或平台筛选无匹配结果时的空状态视图 -->
      <div class="ui-card py-16 text-center text-fg-muted">
        <Search class="mx-auto h-8 w-8 opacity-60 mb-3" />
        <h3 class="text-[14px] font-medium text-fg mb-1">没有匹配的直播间</h3>
        <p class="text-[12px]">试试其他名称，或切换直播平台。</p>
      </div>
    {/if}
  </div>
</div>

<!-- 移除直播间确认弹窗 -->
{#if deleteModal}
  <div
    class="ui-modal-backdrop"
    role="presentation"
    transition:fade={{ duration: 150 }}
    onclick={(e) => {
      if (e.target === e.currentTarget) deleteModal = false;
    }}
    onkeydown={(e) => {
      if (e.key === "Escape") deleteModal = false;
    }}
  >
    <div
      class="ui-modal w-[360px]"
      transition:scale={{ duration: 150, start: 0.95 }}
    >
      <div class="ui-modal-header">
        <span>移除直播间</span>
        <button
          type="button"
          class="ui-btn ui-btn-ghost ui-btn-sm ui-btn-icon"
          onclick={() => (deleteModal = false)}
        >
          <X class="h-4 w-4" />
        </button>
      </div>
      <div class="ui-modal-body text-center py-5">
        <p class="text-[14px] font-medium text-fg">确认移除此直播间？</p>
        <p class="mt-1 text-[12px] text-fg-muted">
          此操作将移除直播间及其所有关联的录制记录，且无法撤销。
        </p>
      </div>
      <div class="ui-modal-footer">
        <button
          type="button"
          class="ui-btn"
          onclick={() => (deleteModal = false)}
        >
          取消
        </button>
        <button
          type="button"
          class="ui-btn ui-btn-danger"
          onclick={async () => {
            if (!deleteRoom) return;
            try {
              await invoke("remove_recorder", {
                roomId: deleteRoom.room_info.room_id,
                platform: deleteRoom.room_info.platform,
              });
              deleteModal = false;
            } catch (e) {
              await message(String(e));
            }
          }}
        >
          移除
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- 添加直播间弹窗 -->
{#if addModal}
  <div
    class="ui-modal-backdrop"
    role="presentation"
    transition:fade={{ duration: 150 }}
    onclick={(e) => {
      if (e.target === e.currentTarget) addModal = false;
    }}
    onkeydown={(e) => {
      if (e.key === "Escape") addModal = false;
    }}
  >
    <div
      class="ui-modal w-[440px]"
      transition:scale={{ duration: 150, start: 0.95 }}
    >
      <div class="ui-modal-header">
        <span>添加直播间</span>
        <button
          type="button"
          class="ui-btn ui-btn-ghost ui-btn-sm ui-btn-icon"
          onclick={() => (addModal = false)}
        >
          <X class="h-4 w-4" />
        </button>
      </div>
      <div class="ui-modal-body space-y-4">
        <PlatformIconTabs
          bind:value={selectedPlatform}
          platforms={[
            { id: "bilibili", label: "哔哩哔哩" },
            { id: "douyin", label: "抖音" },
            { id: "douyu", label: "斗鱼" },
            { id: "huya", label: "虎牙" },
            { id: "kuaishou", label: "快手" },
            { id: "tiktok", label: "TikTok" },
            { id: "twitch", label: "Twitch" },
            { id: "youtube", label: "YouTube" },
          ]}
        />
        <div class="space-y-1.5">
          <label for="room_id_input" class="text-[12px] font-medium text-fg-secondary">
            {selectedPlatform === "douyin"
              ? "主播主页、分享链接或直播间链接"
              : "直播间 ID 或链接"}
          </label>
          <input
            id="room_id_input"
            type="text"
            bind:value={addRoom}
            class="ui-input w-full"
            placeholder={selectedPlatform === "douyin"
              ? "请输入抖音主页、分享链接、sec_uid 或直播间链接"
              : "请输入直播间 ID 或链接"}
          />
        </div>
      </div>
      <div class="ui-modal-footer">
        <button
          type="button"
          class="ui-btn"
          onclick={() => (addModal = false)}
        >
          取消
        </button>
        <button
          type="button"
          class="ui-btn ui-btn-primary"
          disabled={!addRoom}
          onclick={() => {
            addNewRecorder(addRoom, selectedPlatform);
          }}
        >
          确认添加
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- 历史录播归档弹窗 -->
{#if archiveModal}
  <div
    class="ui-modal-backdrop"
    role="presentation"
    transition:fade={{ duration: 150 }}
    onclick={(e) => {
      if (e.target === e.currentTarget) archiveModal = false;
    }}
    onkeydown={(e) => {
      if (e.key === "Escape") archiveModal = false;
    }}
  >
    <div
      class="ui-modal w-[900px] max-w-[95vw] h-[80vh]"
      transition:scale={{ duration: 150, start: 0.95 }}
    >
      <div class="ui-modal-header">
        <div class="flex items-center gap-2">
          <span>直播间记录</span>
          <span class="text-[12px] font-normal text-fg-muted">
            {archiveRoom?.user_info.user_name} · #{archiveRoom?.room_info.room_id}
          </span>
        </div>
        <button
          type="button"
          class="ui-btn ui-btn-ghost ui-btn-sm ui-btn-icon"
          onclick={() => (archiveModal = false)}
        >
          <X class="h-4 w-4" />
        </button>
      </div>
      <div
        class="ui-modal-body p-4 custom-scrollbar-light overflow-y-auto"
        onscroll={handleArchiveScroll}
      >
        <table class="w-full text-left text-[12px]">
          <thead class="border-b border-line text-fg-muted font-medium">
            <tr>
              <th class="px-3 py-2.5">直播时间</th>
              <th class="px-3 py-2.5">标题</th>
              <th class="px-3 py-2.5">时长</th>
              <th class="px-3 py-2.5">大小</th>
              <th class="px-3 py-2.5">码率</th>
              <th class="px-3 py-2.5 text-right">操作</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-line">
            {#each archives as archive (archive.parent_id + ":" + archive.live_id)}
              <tr class="hover:bg-control-hover/40 transition-colors">
                <td class="px-3 py-2.5 whitespace-nowrap">
                  <div>{format_ts(archive.created_at).split(" ")[0]}</div>
                  <div class="text-[11px] text-fg-faint">
                    {format_ts(archive.created_at).split(" ")[1]}
                  </div>
                </td>
                <td class="px-3 py-2.5">
                  <div class="flex items-center gap-2.5">
                    <img
                      src={archive.cover}
                      alt=""
                      class="h-8 w-12 rounded object-cover border border-line bg-sunken shrink-0"
                    />
                    <span class="font-medium text-fg line-clamp-1">
                      {archive.title}
                    </span>
                  </div>
                </td>
                <td class="px-3 py-2.5 whitespace-nowrap tabular-nums text-fg-secondary">
                  {format_duration(archive.length)}
                </td>
                <td class="px-3 py-2.5 whitespace-nowrap tabular-nums text-fg-secondary">
                  {formatSize(archive.size)}
                </td>
                <td class="px-3 py-2.5 whitespace-nowrap tabular-nums text-fg-muted">
                  {calc_bitrate(archive.size, archive.length)} Kbps
                </td>
                <td class="px-3 py-2.5 text-right whitespace-nowrap">
                  <div class="inline-flex items-center gap-1">
                    <button
                      type="button"
                      class="ui-btn ui-btn-ghost ui-btn-sm ui-btn-icon"
                      title="预览录播"
                      onclick={() => {
                        invoke("open_live", {
                          platform: archiveRoom?.room_info.platform,
                          roomId: archiveRoom?.room_info.room_id,
                          liveId: archive.live_id,
                        });
                      }}
                    >
                      <Play class="h-3.5 w-3.5" />
                    </button>
                    <button
                      type="button"
                      class="ui-btn ui-btn-ghost ui-btn-sm ui-btn-icon"
                      title="生成整场切片"
                      onclick={() => openGenerateWholeClipModal(archive)}
                    >
                      <FileVideo class="h-3.5 w-3.5" />
                    </button>
                    <button
                      type="button"
                      class="ui-btn ui-btn-ghost ui-btn-sm ui-btn-icon text-danger hover:bg-danger-soft"
                      title="删除记录"
                      onclick={() => {
                        invoke("delete_archive", {
                          platform: archiveRoom?.room_info.platform,
                          roomId: archiveRoom?.room_info.room_id,
                          liveId: archive.live_id,
                        })
                          .then(async () => {
                            currentPage = 0;
                            archives = [];
                            hasMore = true;
                            loadError = "";
                            await updateArchives();
                          })
                          .catch((e) => alert(e));
                      }}
                    >
                      <Trash2 class="h-3.5 w-3.5" />
                    </button>
                  </div>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>

        <!-- 加载与空状态 -->
        <div class="mt-4 flex justify-center py-2">
          {#if loadError}
            <div class="text-danger text-[12px]">
              {loadError}
              <button
                type="button"
                class="ml-2 text-accent-fg hover:underline cursor-pointer"
                onclick={() => {
                  loadError = "";
                  loadMoreArchives();
                }}
              >
                重试
              </button>
            </div>
          {:else if isLoading}
            <div class="text-[12px] text-fg-muted flex items-center gap-2">
              <div class="h-4 w-4 animate-spin rounded-full border-2 border-accent border-t-transparent"></div>
              <span>加载中...</span>
            </div>
          {:else if archives.length === 0}
            <div class="text-[12px] text-fg-muted">暂无录制记录</div>
          {/if}
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- 整场切片弹窗 -->
{#if generateWholeClipArchive}
  <GenerateWholeClipModal
    bind:showModal={generateWholeClipModal}
    archive={generateWholeClipArchive}
    roomId={generateWholeClipArchive.room_id}
    onGenerated={handleWholeClipGenerated}
  />
{/if}

<!-- 房间自动切片设置弹窗 -->
{#if selectedAutomationRoom}
  <RoomAutomationDialog
    platform={selectedAutomationRoom.room_info.platform}
    roomId={selectedAutomationRoom.room_info.room_id}
    roomTitle={selectedAutomationRoom.room_info.room_title}
    open={showAutomationDialog}
    onClose={() => {
      showAutomationDialog = false;
      // 保存后重新获取自动策略开关
      if (selectedAutomationRoom) {
        void updateAutoPolicies(summary.recorders);
      }
    }}
  />
{/if}
