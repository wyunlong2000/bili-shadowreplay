<script lang="ts">
  import { createStateRefresh, watchStateChanges } from "../lib/state-refresh";
  import { usePageVisibility } from "../lib/page-visibility";
  import { formatSize } from "../lib/size-format";
  import { liveRoomUrl } from "../lib/platform";
  import { invoke, open, onOpenUrl, get_static_url, get } from "../lib/invoker";
  import { message } from "@tauri-apps/plugin-dialog";
  import { fade, scale } from "svelte/transition";
  import { Dropdown, DropdownItem } from "flowbite-svelte";
  import type { RecorderList, RecorderInfo } from "../lib/interface";
  import type { RecordItem } from "../lib/db";
  import {
    Ellipsis,
    Play,
    Plus,
    FileVideo,
    Search,
    Trash2,
    X,
    History,
    PlayIcon,
    Globe,
  } from "lucide-svelte";
  import BilibiliIcon from "../lib/components/BilibiliIcon.svelte";
  import DouyinIcon from "../lib/components/DouyinIcon.svelte";
  import DouyuIcon from "../lib/components/DouyuIcon.svelte";
  import KuaishouIcon from "../lib/components/KuaishouIcon.svelte";
  import HuyaIcon from "../lib/components/HuyaIcon.svelte";
  import TikTokIcon from "../lib/components/TikTokIcon.svelte";
  import TwitchIcon from "../lib/components/TwitchIcon.svelte";
  import YouTubeIcon from "../lib/components/YouTubeIcon.svelte";
  import PlatformIconTabs from "../lib/components/PlatformIconTabs.svelte";
  import AutoRecordIcon from "../lib/components/AutoRecordIcon.svelte";
  import GenerateWholeClipModal from "../lib/components/GenerateWholeClipModal.svelte";
  import RoomAutomationDialog from "../lib/components/automation/RoomAutomationDialog.svelte";
  import { onMount } from "svelte";
  import { registerCommands } from "../lib/stores/commands";

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

  let summary: RecorderList = $state({
    count: 0,
    recorders: [],
  });

  let searchQuery = $state("");
  let filteredRecorders = $derived(summary.recorders.filter((room) => {
    const query = searchQuery.toLowerCase();
    return (
      room.room_info.room_title.toLowerCase().includes(query) ||
      room.user_info.user_name.toLowerCase().includes(query) ||
      room.room_info.room_id.toString().includes(query)
    );
  }));

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
        return "直播进行中";
      case "offline":
        return "直播未开始";
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
    const avatarMap = {
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
    const coverMap = {
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
    const avatarKey = userId ? `${platform}:${userId}:${rawAvatarUrl}` : `${platform}:${roomId}:${rawAvatarUrl}`;
    const avatarPromise = rawAvatarUrl
      ? fetchImageObjectUrl(avatar_cache, avatarKey, rawAvatarUrl)
      : Promise.resolve(null);
    const coverPromise = rawCoverUrl
      ? fetchImageObjectUrl(image_cache, rawCoverUrl, rawCoverUrl)
      : Promise.resolve(null);

    const [avatarUrl, coverUrl] = await Promise.all([avatarPromise, coverPromise]);
    if (!avatarUrl && !coverUrl) return;

    const target = summary.recorders.find(
      (r) => r.room_info.platform === platform && r.room_info.room_id === roomId,
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

  const pageVisibility = usePageVisibility();
  const stateRefresh = createStateRefresh(update_summary);

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

    // 按当前检测状态排序。
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

    // 先同步填充已缓存图片或默认图，未命中缓存的图片转入后台异步加载，不阻塞列表更新
    const pendingHydrations: Array<() => void> = [];
    for (const room of new_summary.recorders) {
      const platform = room.room_info.platform;
      const roomId = room.room_info.room_id;
      const userId = room.user_info.user_id;
      const rawAvatarUrl = room.user_info.user_avatar;
      const rawCoverUrl = room.room_info.room_cover;
      const avatarKey = userId
        ? `${platform}:${userId}:${rawAvatarUrl}`
        : `${platform}:${roomId}:${rawAvatarUrl}`;

      const cachedAvatar = rawAvatarUrl ? avatar_cache.get(avatarKey) : undefined;
      room.user_info.user_avatar = cachedAvatar || default_avatar(platform);

      const cachedCover = rawCoverUrl ? image_cache.get(rawCoverUrl) : undefined;
      if (cachedCover) {
        room.room_info.room_cover = cachedCover;
      } else if (!rawCoverUrl && cachedAvatar) {
        room.room_info.room_cover = cachedAvatar;
      } else {
        room.room_info.room_cover = default_cover(platform);
      }

      if ((rawAvatarUrl && !cachedAvatar) || (rawCoverUrl && !cachedCover)) {
        pendingHydrations.push(() => {
          void hydrateRoomImages(
            platform,
            roomId,
            userId,
            rawAvatarUrl,
            rawCoverUrl,
          );
        });
      }
    }

    summary = new_summary;
    for (const startHydration of pendingHydrations) {
      startHydration();
    }
  }
  onMount(() =>
    watchStateChanges(stateRefresh, ["recorders-changed"], 30_000, pageVisibility),
  );

  let searchInput: HTMLInputElement | null = $state(null);

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

  // modals
  let deleteModal = $state(false);
  let deleteRoom = $state(null);

  let addModal = $state(false);
  let addRoom = $state("");
  let selectedPlatform = $state("bilibili");

  let archiveModal = $state(false);
  let archiveRoom: RecorderInfo = $state(null);
  let archives: RecordItem[] = $state([]);

  // 分页相关状态
  let currentPage = $state(0);
  let pageSize = 20;
  let hasMore = $state(true);
  let isLoading = $state(false);
  let loadError = $state("");

  async function showArchives(room_id: string) {
    // 重置分页状态
    currentPage = 0;
    archives = [];
    hasMore = true;
    isLoading = false;
    loadError = "";

    updateArchives();
    archiveModal = true;
    console.log(archives);
  }

  async function updateArchives() {
    if (isLoading || !hasMore) return;

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

      // 如果是第一页，直接替换；否则追加数据
      if (currentPage === 0) {
        archives = new_archives;
      } else {
        archives = [...archives, ...new_archives];
      }

      // 检查是否还有更多数据
      hasMore = new_archives.length === pageSize;

      // 按时间排序
      archives.sort((a, b) => {
        return (
          new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
        );
      });

      // 更新总数（如果后端支持的话）
      // totalCount = await invoke("get_archives_count", { roomId: archiveRoom.room_id });

      currentPage++;
    } catch (error) {
      console.error("Failed to load archives:", error);
      loadError = "加载失败，请重试";
    } finally {
      isLoading = false;
    }
  }

  async function loadMoreArchives() {
    await updateArchives();
  }

  function handleScroll(event: Event) {
    const target = event.target as HTMLElement;
    const { scrollTop, scrollHeight, clientHeight } = target;

    // 当滚动到距离底部100px时，自动加载更多
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
    return ((size * 8) / duration / 1024).toFixed(0);
  }

  function handleModalClickOutside(event) {
    // 检查点击是否在任何modal内部
    const clickedElement = event.target;

    // 检查是否点击了按钮，如果是则不关闭modal
    if (clickedElement.closest("button")) {
      return;
    }

    // 按层级顺序检查modal，优先处理最上层的modal
    // 如果点击在最上层modal内部，则不处理任何modal关闭

    // 最上层：generateWholeClipModal (handled by component)
    if (generateWholeClipModal) {
      return; // Let the component handle its own modal closing
    }

    // 第二层：archiveModal
    if (archiveModal) {
      const archiveModalEl = document.querySelector(".archive-modal");
      if (archiveModalEl) {
        if (archiveModalEl.contains(clickedElement)) {
          // 点击在archiveModal内部，不关闭任何modal
          return;
        } else {
          // 点击在archiveModal外部，关闭它
          archiveModal = false;
          return;
        }
      }
    }

    // 第三层：addModal
    if (addModal) {
      const addModalEl = document.querySelector(".add-modal");
      if (addModalEl) {
        if (addModalEl.contains(clickedElement)) {
          // 点击在addModal内部，不关闭任何modal
          return;
        } else {
          // 点击在addModal外部，关闭它
          addModal = false;
          return;
        }
      }
    }

    // 第四层：deleteModal
    if (deleteModal) {
      const deleteModalEl = document.querySelector(".delete-modal");
      if (deleteModalEl) {
        if (deleteModalEl.contains(clickedElement)) {
          // 点击在deleteModal内部，不关闭任何modal
          return;
        } else {
          // 点击在deleteModal外部，关闭它
          deleteModal = false;
          return;
        }
      }
    }
  }

  // Function to toggle auto-record state
  function toggleEnabled(room: RecorderInfo) {
    invoke("set_enable", {
      roomId: room.room_info.room_id,
      platform: room.room_info.platform,
      enabled: !room.enabled,
    });
  }

  function openUserUrl(room: RecorderInfo) {
    if (room.room_info.platform === "bilibili") {
      open("https://space.bilibili.com/" + room.user_info.user_id);
    } else if (room.room_info.platform === "douyin") {
      console.log(room.user_info);
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

  function openLiveUrl(room: RecorderInfo) {
    if (room.room_info.platform === "douyin" && room.identifier_type === "profile") {
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

  function addNewRecorder(room_id: string, platform: string) {
    const room_input = room_id.trim();
    const douyu_room_id =
      platform === "douyu"
        ? new URLSearchParams(room_input.split("?")[1]?.split("#")[0] || "").get("rid")
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
        // room_id might be a link, extract the room_id from the link
        // example: https://live.bilibili.com/1234567890
        if (room_id.includes("https://") || room_id.includes("bsr://")) {
          room_id = room_id.split("/").pop();
        }
        break;
      case "douyin":
        // 保留完整链接，由后端区分主页、分享链接和直播间链接。
        break;
      case "douyu":
        // Douyu accepts numeric room IDs and vanity paths.
        // Keep only the path segment so the backend can resolve a vanity path.
        if (room_id.includes("://")) {
          room_id = room_id.split(/[?#]/)[0].replace(/\/+$/, "").split("/").pop() || "";
        }
        break;
      case "huya":
        // room_id might be a link, extract the room_id from the link
        // example: https://www.huya.com/1234567890
        if (room_id.includes("https://") || room_id.includes("bsr://")) {
          room_id = room_id.split("/").pop();
        }
        break;
      case "kuaishou":
        // room_id might be a link, extract the room_id from the link
        // example: https://live.kuaishou.com/u/1234567890
        if (room_id.includes("https://") || room_id.includes("bsr://")) {
          room_id = room_id.split("/").pop();
        }
        break;
      case "tiktok":
        // room_id might be a link, extract the room_id from the link
        // example: https://www.tiktok.com/@1234567890/live -> @1234567890
        if (room_id.includes("https://") || room_id.includes("bsr://")) {
          room_id = "@" + room_id.split("@").pop().split("/")[0];
        }
        break;
      case "twitch":
        // Twitch accepts a channel login or https://www.twitch.tv/<login>.
        if (room_id.includes("https://") || room_id.includes("bsr://")) {
          room_id = room_id
            .replace(/^(?:https?:|bsr:)\/\/(?:(?:www|m)\.)?twitch\.tv\//, "")
            .split("/")[0];
        }
        room_id = room_id.replace(/^@/, "");
        break;
      case "youtube":
        // Keep the URL intact. The recorder resolves a channel to its current
        // live video on each poll, while a watch URL pins one broadcast.
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
  let generateWholeClipArchive: RecordItem = $state(null);

  async function openGenerateWholeClipModal(archive: RecordItem) {
    generateWholeClipModal = true;
    generateWholeClipArchive = archive;
  }

  function handleWholeClipGenerated() {
    generateWholeClipModal = false;
    generateWholeClipArchive = null;
  }

  onMount(async () => {
    await onOpenUrl((urls: string[]) => {
      console.log("Received Deep Link:", urls);
      if (urls.length > 0) {
        const url = urls[0];
        // extract platform and room_id from url
        // url example:
        // bsr://live.bilibili.com/167537?live_from=85001&spm_id_from=333.1365.live_users.item.click
        // bsr://live.douyin.com/200525029536

        let platform = "";

        if (url.startsWith("bsr://live.bilibili.com/")) {
          platform = "bilibili";
        }

        if (url.startsWith("bsr://live.douyin.com/")) {
          platform = "douyin";
        }

        if (url.startsWith("bsr://www.douyu.com/") || url.startsWith("bsr://live.douyu.com/")) {
          platform = "douyu";
        }

        if (url.startsWith("bsr://live.kuaishou.com/")) {
          platform = "kuaishou";
        }

        if (url.startsWith("bsr://live.tiktok.com/")) {
          platform = "tiktok";
        }

        if (
          url.startsWith("bsr://www.twitch.tv/") ||
          url.startsWith("bsr://m.twitch.tv/") ||
          url.startsWith("bsr://twitch.tv/")
        ) {
          platform = "twitch";
        }

        if (
          url.startsWith("bsr://www.youtube.com/") ||
          url.startsWith("bsr://youtube.com/") ||
          url.startsWith("bsr://youtu.be/")
        ) {
          platform = "youtube";
        }

        if (url && platform) {
          addModal = true;
          addRoom = url;
          selectedPlatform = platform;
        }
      }
    });
  });
</script>

<div
  class="flex-1 p-6 overflow-auto custom-scrollbar-light bg-gray-50 dark:bg-black"
>
  <div class="space-y-6">
    <!-- Header -->
    <div class="flex justify-between items-center">
      <div class="flex items-center space-x-4">
        <h1 class="text-2xl font-semibold text-gray-900 dark:text-white">
          直播间
        </h1>
        <div
          class="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm text-gray-500 dark:text-gray-400"
        >
          <span class="flex items-center space-x-1">
            <span class="w-2 h-2 rounded-full bg-green-500"></span>
            <span>{room_active} 直播中</span>
          </span>
          <span>•</span>
          <span>{room_inactive} 未直播</span>
          {#if room_unknown > 0}
            <span>•</span>
            <span>{room_unknown} 状态未知</span>
          {/if}
          {#if room_unconfirmed > 0}
            <span>•</span>
            <span>{room_unconfirmed} 待检测</span>
          {/if}
        </div>
      </div>
      <div class="flex items-center space-x-3">
        <div class="relative">
          <Search
            class="w-5 h-5 absolute left-3 top-1/2 transform -translate-y-1/2 text-gray-400 dark:text-gray-500"
          />
          <input
            type="text"
            bind:value={searchQuery}
            placeholder="搜索直播间..."
            class="pl-10 pr-4 py-2 rounded-lg bg-gray-100 dark:bg-gray-700/50 border border-gray-200 dark:border-gray-600 focus:outline-none focus:ring-2 focus:ring-blue-500 dark:focus:ring-blue-400 text-gray-900 dark:text-white"
          />
        </div>
        <button
          class="px-4 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600 transition-colors flex items-center space-x-2"
          onclick={() => {
            addModal = true;
          }}
        >
          <Plus class="w-5 h-5 icon-white" />
          <span>添加新直播间</span>
        </button>
      </div>
    </div>

<div class="custom-scrollbar-light flex-1 overflow-auto bg-canvas p-5">
  <div class="space-y-6">
    <!-- Room Grid -->
    <div class="grid grid-cols-3 gap-4">
      <!-- Active Room Card -->
      {#each filteredRecorders as room (room.room_info.room_id)}
        {@const detectionStatus = getDetectionStatus(room)}
        <div
          class="p-4 rounded-xl bg-white dark:bg-[#3c3c3e] border border-gray-200 dark:border-gray-700 hover:border-blue-500 dark:hover:border-blue-400 transition-colors"
        >
          <div class="relative">
            <img
              src={room.room_info.room_cover}
              alt="cover"
              class={"w-full h-40 object-cover rounded-lg " +
                (detectionStatus === "offline" ? "brightness-75" : "")}
            />
            <!-- Room ID watermark -->
            <div
              class="absolute bottom-2 left-2 flex h-6 max-w-[55%] items-center rounded-[6px] bg-black/50 px-1.5 backdrop-blur-sm"
            >
              <span class="truncate font-mono text-[11px] text-white/85"
                >{room.room_info.platform.toUpperCase()}#{room.room_info
                  .room_id}</span
              >
            </div>
            {#if room.enabled}
              <div
                class={"absolute top-2 left-2 p-1.5 px-2 rounded-md text-white text-xs flex items-center justify-center " +
                  (room.recording ? "bg-red-500" : "bg-gray-700/90")}
              >
                <AutoRecordIcon class="w-4 h-4 text-white" />
                {#if room.recording}
                  <span class="text-white ml-1">录制中</span>
                {/if}
              </div>
            {/if}
            {#if detectionStatus === "offline"}
              <div
                class={"absolute bottom-2 right-2 p-1.5 px-2 rounded-md text-white text-xs flex items-center justify-center bg-gray-700"}
              >
                <span>{getDetectionLabel(detectionStatus)}</span>
              </div>
            {:else if detectionStatus === "live"}
              <div
                class={"absolute bottom-2 right-2 p-1.5 px-2 rounded-md text-white text-xs flex items-center justify-center bg-green-500"}
              >
                <span>{getDetectionLabel(detectionStatus)}</span>
              </div>
            {:else if detectionStatus === "unconfirmed"}
              <div
                class="absolute bottom-2 right-2 p-1.5 px-2 rounded-md text-white text-xs flex items-center justify-center bg-gray-700"
              >
                <span>{getDetectionLabel(detectionStatus)}</span>
              </div>
            {:else}
              <div
                class="absolute bottom-2 right-2 p-1.5 px-2 rounded-md text-white text-xs flex items-center justify-center bg-amber-600"
                title={getDetectionErrorMessage(room)}
              >
                <span>{getDetectionLabel(detectionStatus)}</span>
              </div>
            {/if}
            <button
              class="absolute top-2 right-2 p-1.5 rounded-lg bg-gray-900/50 hover:bg-gray-900/70 transition-colors"
            >
              <Ellipsis class="w-5 h-5 icon-white" />
            </button>
            <Dropdown class="whitespace-nowrap">
              <button
                class="px-4 py-2 flex items-center justify-between hover:bg-gray-100 dark:hover:bg-gray-700 cursor-pointer"
                onclick={() => toggleEnabled(room)}
              >
                <span
                  class="text-sm text-gray-700 dark:text-gray-200 font-medium"
                  >启用直播间</span
                >
                <label class="toggle-switch ml-1">
                  <input type="checkbox" checked={room.enabled} />
                  <span class="toggle-slider"></span>
                </label>
              </button>
              <DropdownItem
                onclick={() => {
                  openLiveUrl(room);
                }}>打开网页直播间</DropdownItem
              >
              <DropdownItem
                onclick={() => {
                  selectedAutomationRoom = room;
                  showAutomationDialog = true;
                }}>自动切片设置</DropdownItem
              >
              <DropdownItem
                class="text-red-500"
                onclick={() => {
                  deleteRoom = room;
                  deleteModal = true;
                }}>移除直播间</DropdownItem
              >
            </Dropdown>
          </div>
          <div class="mt-3 space-y-2">
            <div class="flex items-start justify-between">
              <div>
                <div class="flex items-center space-x-2">
                  <span class="shrink-0">
                    {#if room.room_info.platform === "bilibili"}
                      <BilibiliIcon class="w-4 h-4" />
                    {:else if room.room_info.platform === "douyin"}
                      <DouyinIcon class="w-4 h-4" />
                    {:else if room.room_info.platform === "douyu"}
                      <DouyuIcon class="w-4 h-4" />
                    {:else if room.room_info.platform === "kuaishou"}
                      <KuaishouIcon class="w-4 h-4" />
                    {:else if room.room_info.platform === "huya"}
                      <HuyaIcon class="w-4 h-4" />
                    {:else if room.room_info.platform === "tiktok"}
                      <TikTokIcon class="w-5 h-5" />
                    {:else if room.room_info.platform === "twitch"}
                      <TwitchIcon class="w-5 h-5 text-purple-600" />
                    {:else if room.room_info.platform === "youtube"}
                      <YouTubeIcon class="w-4 h-4" />
                    {:else}
                      <Globe class="w-4 h-4 text-gray-400" />
                    {/if}
                  </span>
                  <h3 class="font-medium text-gray-900 dark:text-white">
                    {room.room_info.room_title}
                  </h3>
                </div>
              </div>
            </div>
            <div class="flex items-center justify-between">
              <div
                class="flex items-center space-x-2 text-sm text-gray-600 dark:text-gray-400"
              >
                <button
                  class="flex items-center space-x-2 p-1.5 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700"
                  onclick={() => {
                    openUserUrl(room);
                  }}
                >
                  <img
                    src={room.user_info.user_avatar}
                    alt="avatar"
                    class="w-8 h-8 rounded-full"
                  />
                  <span>{room.user_info.user_name}</span>
                </button>
              </div>
              <div class="flex items-center space-x-1">
                {#if room.recording}
                  <button
                    class="p-1.5 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700"
                    onclick={() => {
                      invoke("open_live", {
                        platform: room.room_info.platform,
                        roomId: room.room_info.room_id,
                        liveId: room.live_id,
                      });
                    }}
                  >
                    <Play class="w-5 h-5 dark:icon-white" />
                  </button>
                {/if}
                <button
                  class="p-1.5 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700"
                  onclick={() => {
                    archiveRoom = room;
                    showArchives(room.room_info.room_id);
                  }}
                >
                  <History class="w-5 h-5 dark:icon-white" />
                </button>
              </div>
            </div>
            {#if room.room_info.platform === "douyin" && room.identifier_type === "profile"}
              <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
                {room.room_resolved ? "房间号已解析" : "房间号待解析"}
              </p>
            {/if}
            {#if detectionStatus === "unknown"}
              <p
                class="mt-1 break-words text-xs text-amber-700 dark:text-amber-400"
                title={getDetectionErrorMessage(room)}
              >
                {getDetectionErrorMessage(room)}
              </p>
            {/if}
          </div>
        </div>
      {/each}

      <!-- Add Room Card -->
      <button
        class="p-4 rounded-xl border-2 border-dashed border-gray-300 dark:border-gray-600 hover:border-blue-500 dark:hover:border-blue-400 transition-colors flex flex-col items-center justify-center space-y-2"
        onclick={() => {
          addModal = true;
        }}
      >
        <div
          class="w-12 h-12 rounded-full bg-blue-500/10 flex items-center justify-center"
        >
          <Plus class="w-6 h-6 icon-primary" />
        </div>
        <span class="text-sm font-medium text-blue-600 dark:text-blue-400"
          >添加新直播间</span
        >
        <span class="text-xs text-gray-500 dark:text-gray-400"
          >配置一个新直播间以及其相关设置</span
        >
      </button>
    </div>
  </div>
</div>
</div>
</div>
{#if deleteModal}
  <div
    class="fixed inset-0 bg-black/20 dark:bg-black/40 backdrop-blur-sm z-50 flex items-center justify-center"
    transition:fade={{ duration: 200 }}
  >
    <div
      class="mac-modal delete-modal w-[320px] bg-white dark:bg-[#323234] rounded-xl shadow-xl overflow-hidden"
      transition:scale={{ duration: 150, start: 0.95 }}
    >
      <div class="p-6 space-y-4">
        <div class="text-center space-y-2">
          <h3 class="text-base font-medium text-gray-900 dark:text-white">
            移除直播间
          </h3>
          <p class="text-sm text-gray-500 dark:text-gray-400">
            此操作将移除所有相关的录制记录
          </p>
        </div>
        <div class="flex justify-center space-x-3">
          <button
            class="w-24 px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-600 rounded-lg transition-colors"
            onclick={() => {
              deleteModal = false;
            }}
          >
            取消
          </button>
          <button
            class="w-24 px-4 py-2 bg-red-600 hover:bg-red-700 text-white text-sm font-medium rounded-lg transition-colors"
            onclick={async () => {
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
  </div>
{/if}

{#if addModal}
  <div
    class="fixed inset-0 bg-black/20 dark:bg-black/40 backdrop-blur-sm z-50 flex items-center justify-center"
    transition:fade={{ duration: 200 }}
  >
    <div
      class="mac-modal add-modal w-[400px] bg-white dark:bg-[#323234] rounded-xl shadow-xl overflow-hidden"
      transition:scale={{ duration: 150, start: 0.95 }}
    >
      <!-- Header -->
      <div class="px-6 py-4 border-b border-gray-200 dark:border-gray-700/50">
        <h2 class="text-base font-medium text-gray-900 dark:text-white">
          添加直播间
        </h2>
      </div>

      <div class="p-6 space-y-6">
        <div class="space-y-4">
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

          <div class="space-y-2">
            <label
              for="room_id"
              class="block text-sm font-medium text-gray-700 dark:text-gray-300"
            >
              {selectedPlatform === "douyin"
                ? "主播主页、分享链接或直播间链接"
                : "直播间 ID 或链接"}
            </label>
            <input
              id="room_id"
              type="text"
              bind:value={addRoom}
              class="w-full px-3 py-2 bg-[#f5f5f7] dark:bg-[#1c1c1e] border-0 rounded-lg focus:ring-2 focus:ring-blue-500 text-gray-900 dark:text-white placeholder-gray-500 dark:placeholder-gray-400"
              placeholder={selectedPlatform === "douyin"
                ? "请输入抖音主页、分享链接、sec_uid 或直播间链接"
                : "请输入直播间 ID 或链接"}
            />
          </div>

          <div class="flex justify-end space-x-3">
            <button
              class="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-600 rounded-lg transition-colors"
              onclick={() => {
                addModal = false;
              }}
            >
              取消
            </button>
            <button
              class="px-4 py-2 bg-[#0A84FF] hover:bg-[#0A84FF]/90 text-white text-sm font-medium rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              disabled={!addRoom}
              onclick={() => {
                addNewRecorder(addRoom, selectedPlatform);
                addModal = false;
                addRoom = "";
              }}
            >
              添加
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
{/if}

{#if archiveModal}
  <div
    class="fixed inset-0 bg-black/20 dark:bg-black/40 backdrop-blur-sm z-50 flex items-center justify-center"
    transition:fade={{ duration: 200 }}
  >
    <div
      class="mac-modal archive-modal w-[900px] bg-white dark:bg-[#323234] rounded-xl shadow-xl overflow-hidden flex flex-col max-h-[80vh]"
      transition:scale={{ duration: 150, start: 0.95 }}
    >
      <!-- Header -->
      <div
        class="flex justify-between items-center px-6 py-4 border-b border-gray-200 dark:border-gray-700/50"
      >
        <div class="flex items-center space-x-3">
          <h2 class="text-base font-medium text-gray-900 dark:text-white">
            直播间记录
          </h2>
          <span class="text-sm text-gray-500 dark:text-gray-400">
            {archiveRoom?.user_info.user_name} · {archiveRoom?.room_info
              .room_id}
          </span>
        </div>
        <button
          class="p-1.5 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700/50 transition-colors"
          onclick={() => (archiveModal = false)}
        >
          <X class="w-5 h-5 dark:icon-white" />
        </button>
      </div>

      <div
        class="flex-1 overflow-auto custom-scrollbar-light"
        onscroll={handleScroll}
      >
        <div class="p-6">
          <div class="overflow-x-auto custom-scrollbar-light">
            <table class="w-full">
              <thead>
                <tr class="border-b border-gray-200 dark:border-gray-700/50">
                  <th
                    class="px-4 py-3 text-left text-sm font-medium text-gray-500 dark:text-gray-400"
                    >直播时间</th
                  >
                  <th
                    class="px-4 py-3 text-left text-sm font-medium text-gray-500 dark:text-gray-400"
                    >标题</th
                  >
                  <th
                    class="px-4 py-3 text-left text-sm font-medium text-gray-500 dark:text-gray-400"
                    >时长</th
                  >
                  <th
                    class="px-4 py-3 text-left text-sm font-medium text-gray-500 dark:text-gray-400"
                    >大小</th
                  >
                  <th
                    class="px-4 py-3 text-left text-sm font-medium text-gray-500 dark:text-gray-400"
                    >码率</th
                  >
                  <th
                    class="px-4 py-3 text-left text-sm font-medium text-gray-500 dark:text-gray-400"
                    >操作</th
                  >
                </tr>
              </thead>
              <tbody class="divide-y divide-gray-200 dark:divide-gray-700/50">
                {#each archives as archive (archive.parent_id + ":" + archive.live_id)}
                  <tr
                    class="group hover:bg-[#f5f5f7] dark:hover:bg-[#3a3a3c] transition-colors"
                  >
                    <td class="px-4 py-3">
                      <div class="flex flex-col">
                        <span class="text-sm text-gray-900 dark:text-white"
                          >{format_ts(archive.created_at).split(" ")[0]}</span
                        >
                        <span class="text-xs text-gray-500 dark:text-gray-400"
                          >{format_ts(archive.created_at).split(" ")[1]}</span
                        >
                      </div>
                    </td>
                    <td class="px-4 py-3">
                      <div class="flex items-center space-x-3">
                        <img
                          src={archive.cover}
                          alt="cover"
                          class="w-12 h-8 rounded object-cover"
                        />
                        <span class="text-sm text-gray-900 dark:text-white"
                          >{archive.title}</span
                        >
                      </div>
                    </td>
                    <td class="px-4 py-3 text-sm text-gray-900 dark:text-white"
                      >{format_duration(archive.length)}</td
                    >
                    <td class="px-4 py-3 text-sm text-gray-900 dark:text-white"
                      >{formatSize(archive.size)}</td
                    >
                    <td
                      class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400"
                      >{calc_bitrate(archive.size, archive.length)} Kbps</td
                    >
                    <td class="px-4 py-3">
                      <div class="flex items-center space-x-2">
                        <button
                          class="p-1.5 rounded-lg hover:bg-blue-500/10 transition-colors"
                          title="预览录播"
                          onclick={() => {
                            invoke("open_live", {
                              platform: archiveRoom.room_info.platform,
                              roomId: archiveRoom.room_info.room_id,
                              liveId: archive.live_id,
                            });
                          }}
                        >
                          <PlayIcon class="w-4 h-4 icon-primary" />
                        </button>
                        <button
                          class="p-1.5 rounded-lg hover:bg-blue-500/10 transition-colors"
                          title="生成完整切片"
                          onclick={() => {
                            openGenerateWholeClipModal(archive);
                          }}
                        >
                          <FileVideo class="w-4 h-4 icon-primary" />
                        </button>
                        <button
                          class="p-1.5 rounded-lg hover:bg-red-500/10 transition-colors"
                          title="删除记录"
                          onclick={() => {
                            invoke("delete_archive", {
                              platform: archiveRoom.room_info.platform,
                              roomId: archiveRoom.room_info.room_id,
                              liveId: archive.live_id,
                            })
                              .then(async () => {
                                // 删除后重新加载第一页数据
                                currentPage = 0;
                                archives = [];
                                hasMore = true;
                                loadError = "";
                                await updateArchives();
                              })
                              .catch((e) => {
                                alert(e);
                              });
                          }}
                        >
                          <Trash2 class="w-4 h-4 text-red-500" />
                        </button>
                      </div>
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>

          <!-- 加载更多区域 -->
          <div class="mt-6 flex justify-center">
            {#if loadError}
              <div class="text-red-500 dark:text-red-400 text-sm mb-3">
                {loadError}
                <button
                  class="ml-2 text-blue-500 hover:text-blue-600 underline"
                  onclick={() => {
                    loadError = "";
                    loadMoreArchives();
                  }}
                >
                  重试
                </button>
              </div>
            {/if}

            {#if isLoading && currentPage === 0}
              <div
                class="flex items-center space-x-2 text-gray-500 dark:text-gray-400"
              >
                <div
                  class="animate-spin rounded-full h-5 w-5 border-b-2 border-blue-500"
                ></div>
                <span>加载中...</span>
              </div>
            {:else if isLoading}
              <div
                class="flex items-center space-x-2 text-gray-500 dark:text-gray-400"
              >
                <div
                  class="animate-spin rounded-full h-4 w-4 border-b-2 border-blue-500"
                ></div>
                <span>加载更多...</span>
              </div>
            {:else if archives.length === 0 && !isLoading}
              <div class="text-gray-500 dark:text-gray-400 text-sm">
                暂无录制记录
              </div>
            {/if}
          </div>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- Generate Whole Clip Modal -->
<GenerateWholeClipModal
  bind:showModal={generateWholeClipModal}
  archive={generateWholeClipArchive}
  roomId={generateWholeClipArchive?.room_id || ""}
  onGenerated={handleWholeClipGenerated}
/>

<!-- 房间自动切片策略弹窗 -->
{#if selectedAutomationRoom}
  <RoomAutomationDialog
    platform={selectedAutomationRoom.room_info.platform}
    roomId={selectedAutomationRoom.room_info.room_id}
    roomTitle={selectedAutomationRoom.room_info.room_title}
    open={showAutomationDialog}
    onClose={() => {
      showAutomationDialog = false;
    }}
  />
{/if}

<svelte:window onmousedown={handleModalClickOutside} />

<style>
  /* macOS style toggle switch */
  .toggle-switch {
    position: relative;
    display: inline-block;
    width: 36px;
    height: 20px;
  }

  .toggle-switch input {
    opacity: 0;
    width: 0;
    height: 0;
  }

  .toggle-slider {
    position: absolute;
    cursor: pointer;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: rgba(120, 120, 128, 0.32);
    transition: 0.2s;
    border-radius: 20px;
  }

  .toggle-slider:before {
    position: absolute;
    content: "";
    height: 16px;
    width: 16px;
    left: 2px;
    bottom: 2px;
    background-color: white;
    transition: 0.2s;
    border-radius: 50%;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
  }

  input:checked + .toggle-slider {
    background-color: #34c759;
  }

  input:checked + .toggle-slider:before {
    transform: translateX(16px);
  }

  @keyframes spin-slow {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }

  .animate-spin-slow {
    animation: spin-slow 3s linear infinite;
  }
</style>
