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
  lastUsedAt: number | null;
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

export type AccountSearchScope =
  | { kind: "all" }
  | { kind: "uncategorized" }
  | { kind: "category"; categoryId: string };

export type AccountSearchSnapshot = {
  revision: number;
  accounts: AccountSummary[];
};

/** 明文密码只在用户明确点击显示后，以单条、一次性响应返回。 */
export type RevealedPassword = {
  password: string;
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

/** 列表显示密码使用独立 command，不复用包含全部编辑字段的详情响应。 */
export function revealAccountPassword(id: string): Promise<RevealedPassword> {
  return invoke<RevealedPassword>("reveal_account_password", { id });
}

/** 复制由 Rust 按稳定 ID 读取并写入系统剪贴板，前端不接收账号名响应。 */
export function copyAccountUsername(id: string): Promise<VaultStatus> {
  return invoke<VaultStatus>("copy_account_username", { id });
}

/** 密码直接在 Rust/Windows 边界写入，响应不包含密码或清理凭证。 */
export function copyAccountPassword(id: string): Promise<VaultStatus> {
  return invoke<VaultStatus>("copy_account_password", { id });
}

/** 搜索只返回安全账号摘要和当前 revision，不会触发密码库写入。 */
export function searchAccounts(query: string, scope: AccountSearchScope): Promise<AccountSearchSnapshot> {
  return invoke<AccountSearchSnapshot>("search_accounts", { query, scope });
}

/** 字段和分类归属共享一次 Rust 候选保存，账号 ID 仍由独立参数定位。 */
export function updateAccount(id: string, input: UpdateAccountInput): Promise<VaultStatus> {
  return invoke<VaultStatus>("update_account", { id, input });
}

/** 删除只提交稳定账号 ID，成功响应仍是不含密码的最新列表快照。 */
export function deleteAccount(id: string): Promise<VaultStatus> {
  return invoke<VaultStatus>("delete_account", { id });
}
