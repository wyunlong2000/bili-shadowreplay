import { listen } from "./invoker";

/** Serializes snapshots and remembers changes that arrive during a request. */
export function createStateRefresh(
  refresh: () => Promise<void>,
  delayMs = 200,
  onError: (error: unknown) => void = (error) => console.error("刷新数据失败:", error),
) {
  let stopped = false;
  let dirty = false;
  let pending: Promise<void> | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null;

  function clearTimer() {
    if (timer !== null) clearTimeout(timer);
    timer = null;
  }

  async function drain() {
    try {
      while (dirty && !stopped) {
        dirty = false;
        try {
          await refresh();
        } catch (error) {
          onError(error);
        }
      }
    } finally {
      pending = null;
    }
  }

  function run(): Promise<void> {
    if (stopped) return Promise.resolve();
    clearTimer();
    dirty = true;
    if (!pending) {
      pending = Promise.resolve().then(drain);
    }
    return pending;
  }

  function schedule() {
    if (stopped) return;
    if (pending) {
      dirty = true;
    } else if (timer === null) {
      // A fixed coalescing window prevents continuous events postponing refresh.
      timer = setTimeout(() => { void run(); }, delayMs);
    }
  }

  return {
    run,
    schedule,
    stop() {
      stopped = true;
      dirty = false;
      clearTimer();
    },
  };
}

/** 页面激活状态感知接口，用于在页面隐藏时挂起刷新并在重新激活时补齐。 */
export interface PageVisibility {
  /** 返回当前页面是否处于激活可见状态。 */
  isVisible: () => boolean;
  /** 订阅页面激活状态变化，返回取消订阅函数。 */
  subscribe?: (listener: (visible: boolean) => void) => () => void;
}

/** 在首次快照前完成事件订阅，支持按页面激活状态挂起后台刷新，并返回同步清理函数。 */
export function watchStateChanges(
  refresh: ReturnType<typeof createStateRefresh>,
  events: string[],
  fallbackMs = 30_000,
  pageVisibility?: PageVisibility,
): () => void {
  let disposed = false;
  let ready = false;
  let dirty = false;
  const unlisteners: Array<() => void> = [];
  const isPageActive = () => (pageVisibility ? pageVisibility.isVisible() : true);
  const visible = () => document.visibilityState !== "hidden" && isPageActive();
  const schedule = () => {
    if (!ready) return;
    if (visible()) {
      dirty = false;
      refresh.schedule();
    } else {
      dirty = true;
    }
  };
  const resync = () => {
    if (!ready) return;
    if (visible()) {
      dirty = false;
      void refresh.run();
    } else {
      dirty = true;
    }
  };
  const onPageVisibilityChange = (active: boolean) => {
    if (!ready || !active) return;
    if (dirty && visible()) {
      dirty = false;
      void refresh.run();
    }
  };
  const unsubscribePage = pageVisibility?.subscribe?.(onPageVisibilityChange);

  void (async () => {
    await Promise.all([...new Set([...events, "state-resync"])].map(async (event) => {
      try {
        const unlisten = await listen(event, event === "state-resync" ? resync : schedule);
        if (disposed) unlisten();
        else unlisteners.push(unlisten);
      } catch (error) {
        console.error(`订阅 ${event} 失败:`, error);
      }
    }));
    if (!disposed) {
      ready = true;
      if (visible()) {
        dirty = false;
        await refresh.run();
      } else {
        dirty = true;
      }
    }
  })();

  const timer = setInterval(schedule, fallbackMs);
  document.addEventListener("visibilitychange", resync);
  window.addEventListener("online", resync);

  return () => {
    disposed = true;
    ready = false;
    dirty = false;
    clearInterval(timer);
    unsubscribePage?.();
    refresh.stop();
    for (const unlisten of unlisteners) unlisten();
    document.removeEventListener("visibilitychange", resync);
    window.removeEventListener("online", resync);
  };
}
