import { invoke } from "@tauri-apps/api/core";

/** Rust 启动摘要的前端镜像；这里只声明界面确实需要的非敏感字段。 */
export type CategorySummary = {
  id: string;
  name: string;
  accountCount: number;
};

/** 账号列表只保留展示字段，不接收明文密码。 */
export type AccountSummary = {
  id: string;
  categoryId: string | null;
  username: string;
  address: string | null;
  email: string | null;
};

export type CreateAccountInput = {
  categoryId: string | null;
  username: string;
  password: string;
  address: string | null;
  email: string | null;
  notes: string | null;
};

/** 单条账号编辑详情；仅在打开该账号编辑弹框时请求。 */
export type AccountEditDetail = {
  id: string;
  categoryId: string | null;
  username: string;
  password: string;
  address: string | null;
  email: string | null;
  notes: string | null;
};

export type UpdateAccountInput = CreateAccountInput;

export type VaultStatus = {
  schemaVersion: number;
  revision: number;
  categoryCount: number;
  accountCount: number;
  initialized: boolean;
  categories: CategorySummary[];
  uncategorizedCount: number;
  accounts: AccountSummary[];
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

/** 删除只提交分类 ID；是否为空由 Rust 在持久化前重新判断。 */
export function deleteCategory(id: string): Promise<VaultStatus> {
  return invoke<VaultStatus>("delete_category", { id });
}

/** 明文密码只作为本次 command 请求参数发送，响应中的账号列表不含密码。 */
export function createAccount(input: CreateAccountInput): Promise<VaultStatus> {
  return invoke<VaultStatus>("create_account", { input });
}

/** 每次必须指定一个账号 ID，Rust 只返回该条记录的编辑字段。 */
export function loadAccountForEdit(id: string): Promise<AccountEditDetail> {
  return invoke<AccountEditDetail>("load_account_for_edit", { id });
}

/** 字段和分类归属共享一次 Rust 候选保存，账号 ID 仍由独立参数定位。 */
export function updateAccount(id: string, input: UpdateAccountInput): Promise<VaultStatus> {
  return invoke<VaultStatus>("update_account", { id, input });
}
