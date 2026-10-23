import {
  Brain,
  Clapperboard,
  Film,
  History,
  Info,
  LayoutDashboard,
  List,
  Settings,
  Upload,
  Users,
  Video,
} from "lucide-svelte";

/** 页面项定义。 */
export interface PageDef {
  label: string;
  icon: any;
  keywords: string[];
}

/** 导航分组定义。 */
export interface NavGroup {
  title: string;
  isTail?: boolean;
  pages: PageDef[];
}

/** 侧边栏导航职责分组。 */
export const navGroups: NavGroup[] = [
  {
    title: "录制管理",
    pages: [
      { label: "总览", icon: LayoutDashboard, keywords: ["overview", "dashboard", "summary"] },
      { label: "直播间", icon: Video, keywords: ["rooms", "live", "streamer"] },
      { label: "录播", icon: History, keywords: ["archives", "records", "replay"] },
    ],
  },
  {
    title: "内容创作",
    pages: [
      { label: "素材", icon: Film, keywords: ["materials", "videos", "footage"] },
      { label: "切片", icon: Clapperboard, keywords: ["clips", "projects", "edit"] },
      { label: "投稿", icon: Upload, keywords: ["uploads", "post", "submit", "bilibili"] },
    ],
  },
  {
    title: "工具",
    pages: [
      { label: "任务", icon: List, keywords: ["tasks", "jobs"] },
      { label: "助手", icon: Brain, keywords: ["assistant", "ai", "agent", "chat"] },
    ],
  },
  {
    title: "应用管理",
    isTail: true,
    pages: [
      { label: "账号", icon: Users, keywords: ["accounts", "login", "cookie"] },
      { label: "设置", icon: Settings, keywords: ["settings", "preferences", "config"] },
      { label: "关于", icon: Info, keywords: ["about", "version", "update"] },
    ],
  },
];

/** 扁平化的所有页面列表，保持原有顺序与导出兼容。 */
export const pages: PageDef[] = navGroups.flatMap((group) => group.pages);
