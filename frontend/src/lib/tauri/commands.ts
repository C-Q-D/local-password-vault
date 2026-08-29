import { invoke } from "@tauri-apps/api/core";

/** Rust 启动摘要的前端镜像；这里只声明界面确实需要的非敏感字段。 */
export type CategorySummary = {
  id: string;
  name: string;
  accountCount: number;
};

export type VaultStatus = {
  schemaVersion: number;
  revision: number;
  categoryCount: number;
  accountCount: number;
  initialized: boolean;
  categories: CategorySummary[];
  uncategorizedCount: number;
};

export type CommandError = {
  code: string;
  message: string;
};

/** 所有 Tauri command 名称集中在这一层，组件不直接拼写 IPC 协议。 */
export function loadVault(): Promise<VaultStatus> {
  return invoke<VaultStatus>("load_vault");
}

/** 创建分类只提交名称，成功结果始终是 Rust 从磁盘确认后的最新快照。 */
export function createCategory(name: string): Promise<VaultStatus> {
  return invoke<VaultStatus>("create_category", { name });
}

/** 重命名始终按 Rust 返回的稳定分类 ID 定位，不能依赖可能变化的名称。 */
export function renameCategory(id: string, name: string): Promise<VaultStatus> {
  return invoke<VaultStatus>("rename_category", { id, name });
}
