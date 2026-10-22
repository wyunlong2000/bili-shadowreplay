import { invoke } from "./invoker";
import type {
  AutoClippingPolicy,
  AutomationCandidateRow,
  AutomationOutputRow,
  AutomationRunRow,
} from "./interface";

export interface GetAutomationPolicyResponse {
  policy: AutoClippingPolicy;
  inherited: boolean;
  revision: number;
  can_write: boolean;
}

export interface GenerateCandidateProjectResponse {
  material_id: number;
  project_id: number;
}

/**
 * 获取全局或指定房间的自动化切片策略
 */
export async function getAutomationPolicy(
  platform?: string,
  roomId?: string
): Promise<GetAutomationPolicyResponse> {
  return invoke<GetAutomationPolicyResponse>("get_automation_policy", {
    request: { platform, roomId },
  });
}

/**
 * 保存全局或指定房间的自动化切片策略
 */
export async function saveAutomationPolicy(
  policy: AutoClippingPolicy,
  platform?: string,
  roomId?: string,
  expectedRevision?: number
): Promise<void> {
  return invoke<void>("save_automation_policy", {
    request: { policy, platform, roomId, expectedRevision },
  });
}

/**
 * 手动发起场次自动化分析
 */
export async function startAutomationAnalysis(
  platform: string,
  roomId: string,
  parentId: string,
  liveId: string
): Promise<string> {
  return invoke<string>("start_automation_analysis", {
    request: { platform, roomId, parentId, liveId },
  });
}

/**
 * 获取场次自动化运行记录
 */
export async function getAutomationRuns(
  platform?: string,
  roomId?: string,
  limit?: number
): Promise<AutomationRunRow[]> {
  return invoke<AutomationRunRow[]>("get_automation_runs", {
    request: { platform, roomId, limit },
  });
}

/**
 * 获取某场次运行下的所有高光候选
 */
export async function getAutomationCandidates(
  runId: string
): Promise<AutomationCandidateRow[]> {
  return invoke<AutomationCandidateRow[]>("get_automation_candidates", {
    request: { runId },
  });
}

/**
 * 获取某候选对应的成片输出记录
 */
export async function getAutomationOutputs(
  candidateId: string
): Promise<AutomationOutputRow[]> {
  return invoke<AutomationOutputRow[]>("get_automation_outputs", {
    request: { candidateId },
  });
}

/**
 * 手动微调候选片段边界与类型
 */
export async function updateAutomationCandidate(
  candidateId: string,
  expectedRevision: number,
  startMs: number,
  endMs: number,
  contentType: string
): Promise<boolean> {
  return invoke<boolean>("update_automation_candidate", {
    request: { candidateId, expectedRevision, startMs, endMs, contentType },
  });
}

/**
 * 从候选片段生成素材并创建 Timeline V1 工程
 */
export async function generateCandidateProject(
  candidateId: string
): Promise<GenerateCandidateProjectResponse> {
  return invoke<GenerateCandidateProjectResponse>("generate_candidate_project", {
    request: { candidateId },
  });
}

/**
 * 取消场次自动化分析任务
 */
export async function cancelAutomationRun(runId: string): Promise<void> {
  return invoke<void>("cancel_automation_run", {
    request: { runId },
  });
}
