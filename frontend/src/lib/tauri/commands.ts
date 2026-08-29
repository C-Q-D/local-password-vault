import { invoke } from "@tauri-apps/api/core";

/** Rust 启动摘要的前端镜像；这里只声明界面确实需要的非敏感字段。 */
export type VaultStatus = {
  schemaVersion: number;
  revision: number;
  categoryCount: number;
  accountCount: number;
  initialized: boolean;
};

/** 所有 Tauri command 名称集中在这一层，组件不直接拼写 IPC 协议。 */
export function loadVault(): Promise<VaultStatus> {
  return invoke<VaultStatus>("load_vault");
}
