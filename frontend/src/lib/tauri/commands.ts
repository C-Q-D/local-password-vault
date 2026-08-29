import { invoke } from "@tauri-apps/api/core";

export type StackCheckResult = {
  message: string;
  rustVersion: string;
};

/** 前端只通过这一层访问 Tauri command，避免组件内散落 command 名称和参数。 */
export function checkStack(name: string): Promise<StackCheckResult> {
  return invoke<StackCheckResult>("check_stack", { name });
}
