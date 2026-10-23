import { getContext, setContext } from "svelte";
import type { PageVisibility } from "./state-refresh";

const key = Symbol("page-visibility");

/**
 * 在 PageView 组件初始化阶段提供页面可见性上下文，并返回用于同步可见状态变化的通知器。
 */
export function providePageVisibility(getVisible: () => boolean) {
  const listeners = new Set<(visible: boolean) => void>();
  const visibility: PageVisibility = {
    isVisible: getVisible,
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
  setContext(key, visibility);
  return {
    /** 通知所有订阅者当前页面的可见状态已变更。 */
    notify(visible: boolean) {
      for (const listener of listeners) {
        listener(visible);
      }
    },
  };
}

/**
 * 在页面或子组件初始化阶段获取所属 PageView 的可见性上下文。
 */
export function usePageVisibility(): PageVisibility | undefined {
  return getContext<PageVisibility | undefined>(key);
}
