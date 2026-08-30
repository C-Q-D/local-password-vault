<script lang="ts">
  import { onMount } from "svelte";
  import {
    createAccount,
    createCategory,
    copyAccountPassword,
    copyAccountUsername,
    deleteAccount,
    deleteCategory,
    loadAccountForEdit,
    loadVaultState,
    recoverFromBackup,
    renameCategory,
    revealAccountPassword,
    searchAccounts,
    updateAccount,
    type AccountSearchScope,
    type AccountSummary,
    type CategorySummary,
    type CommandError,
    type UsageEvent,
    type VaultStatus,
  } from "../lib/tauri/commands";

  let status: "loading" | "ready" | "recoveryRequired" | "recovering" | "unavailable" | "error" = "loading";
  let vault: VaultStatus | null = null;
  let errorMessage = "";
  let recoveryError = "";
  let recoveryCancelled = false;
  let categoryDialogOpen = false;
  let categoryDialogMode: "create" | "rename" = "create";
  let editingCategoryId: string | null = null;
  let categoryMenu: { category: CategorySummary; x: number; y: number } | null = null;
  let pendingDeleteCategory: CategorySummary | null = null;
  let deleteSubmitting = false;
  let deleteError = "";
  let notice = "";
  let noticeTimer: number | undefined;
  let selectedCategoryId: string | "uncategorized" | null = null;
  let categoryName = "";
  let categoryError = "";
  let categorySubmitting = false;
  let categoryInput: HTMLInputElement;
  let accountDialogOpen = false;
  let accountDialogMode: "create" | "edit" = "create";
  let editingAccountId: string | null = null;
  let accountDetailLoaded = false;
  let accountLoading = false;
  let accountLoadSequence = 0;
  let accountSubmitting = false;
  let pendingDeleteAccount: { id: string; username: string } | null = null;
  let accountDeleteSubmitting = false;
  let accountDeleteError = "";
  let accountError = "";
  let accountUsernameError = "";
  let accountPasswordError = "";
  let accountPasswordVisible = false;
  let accountUsernameInput: HTMLInputElement;
  let accountDraft = emptyAccountDraft();
  let confirmedAccountDraft: ReturnType<typeof emptyAccountDraft> | null = null;
  let searchQuery = "";
  let searchResults: AccountSummary[] = [];
  let searchStatus: "idle" | "loading" | "ready" | "error" = "idle";
  let searchError = "";
  let searchSequence = 0;
  let revealedCredential: { accountId: string; password: string } | null = null;
  let revealLoadingAccountId: string | null = null;
  let revealSequence = 0;
  let credentialAction: { accountId: string; kind: "username" | "password" } | null = null;
  let accountSort: "recent" | "earliest" = "recent";

  type UsageDay = {
    key: string;
    label: string;
    accountUses: number;
    passwordCopies: number;
  };

  type UsageChart = {
    days: UsageDay[];
    ticks: number[];
    maximum: number;
    hasData: boolean;
  };

  let currentUsageChart: UsageChart;
  $: currentUsageChart = usageChart(vault?.usageEvents ?? []);

  function emptyAccountDraft() {
    return {
      username: "",
      password: "",
      address: "",
      email: "",
      notes: "",
      categoryId: "uncategorized",
    };
  }

  /** 每次只接受 Rust 已确认的摘要；调用失败时不伪装成空密码库。 */
  async function refreshVault() {
    clearRevealedPassword();
    status = "loading";
    errorMessage = "";
    recoveryError = "";
    recoveryCancelled = false;

    try {
      const loaded = await loadVaultState();
      if (loaded.status === "ready") {
        vault = loaded.vault;
        status = "ready";
        if (searchQuery.trim()) await runSearch();
      } else {
        vault = null;
        status = loaded.status;
      }
    } catch (error) {
      vault = null;
      status = "error";
      errorMessage = error instanceof Error ? error.message : String(error);
    }
  }

  /** 只有用户点击确认按钮才调用写命令；取消只改变当前界面状态。 */
  async function confirmRecovery() {
    if (status !== "recoveryRequired") return;
    status = "recovering";
    recoveryError = "";
    try {
      vault = await recoverFromBackup();
      status = "ready";
      recoveryCancelled = false;
      showNotice("已从上一版备份恢复密码库");
    } catch (error) {
      const commandError = error as Partial<CommandError> | null;
      status = "recoveryRequired";
      recoveryError = commandError?.message || "备份恢复失败，密码库仍保持不可编辑";
    }
  }

  onMount(() => {
    void refreshVault();
  });

  /** 每次打开新建弹框都从空表单开始。 */
  function openCategoryDialog() {
    categoryDialogMode = "create";
    editingCategoryId = null;
    categoryName = "";
    categoryError = "";
    categoryDialogOpen = true;
    requestAnimationFrame(() => categoryInput?.focus());
  }

  function openRenameDialog(category: CategorySummary) {
    categoryDialogMode = "rename";
    editingCategoryId = category.id;
    categoryName = category.name;
    categoryError = "";
    categoryDialogOpen = true;
    categoryMenu = null;
    requestAnimationFrame(() => {
      categoryInput?.focus();
      categoryInput?.select();
    });
  }

  function openCategoryMenu(event: MouseEvent, category: CategorySummary) {
    event.preventDefault();
    categoryMenu = {
      category,
      x: Math.min(event.clientX, window.innerWidth - 150),
      y: Math.min(event.clientY, window.innerHeight - 96),
    };
  }

  function showNotice(message: string) {
    notice = message;
    if (noticeTimer) window.clearTimeout(noticeTimer);
    noticeTimer = window.setTimeout(() => (notice = ""), 2600);
  }

  function selectedCategory(): CategorySummary | null {
    if (!vault || !selectedCategoryId || selectedCategoryId === "uncategorized") return null;
    return vault.categories.find((category) => category.id === selectedCategoryId) ?? null;
  }

  function selectedAccountCount(): number {
    if (selectedCategoryId === "uncategorized") return vault?.uncategorizedCount ?? 0;
    return selectedCategory()?.accountCount ?? vault?.accountCount ?? 0;
  }

  function selectedViewName(): string {
    if (selectedCategoryId === "uncategorized") return "未分类";
    return selectedCategory()?.name ?? "全部账号";
  }

  function currentSearchScope(): AccountSearchScope {
    if (selectedCategoryId === "uncategorized") return { kind: "uncategorized" };
    if (selectedCategoryId) return { kind: "category", categoryId: selectedCategoryId };
    return { kind: "all" };
  }

  async function runSearch() {
    const sequence = ++searchSequence;
    const query = searchQuery.trim();
    searchError = "";
    if (!query) {
      searchResults = [];
      searchStatus = "idle";
      return;
    }

    const scope = currentSearchScope();
    searchResults = [];
    searchStatus = "loading";
    try {
      const result = await searchAccounts(query, scope);
      // 快速输入或切换分类后，旧请求即使较晚返回也不能覆盖当前结果。
      if (sequence !== searchSequence) return;
      searchResults = result.accounts;
      searchStatus = "ready";
    } catch (error) {
      if (sequence !== searchSequence) return;
      const commandError = error as Partial<CommandError> | null;
      searchError = commandError?.message || "账号搜索失败，请重试";
      searchStatus = "error";
    }
  }

  /** 先同步输入框的新值，再发起搜索，避免事件处理早于 bind:value 更新而少搜一个字符。 */
  function handleSearchInput(event: Event) {
    clearRevealedPassword();
    searchQuery = (event.currentTarget as HTMLInputElement).value;
    void runSearch();
  }

  function selectCategory(categoryId: string | "uncategorized" | null) {
    clearRevealedPassword();
    selectedCategoryId = categoryId;
    if (searchQuery.trim()) void runSearch();
  }

  /** 通过参数让 Svelte 明确追踪搜索结果，Rust 返回后列表才能立即重新渲染。 */
  function visibleAccounts(currentSearchResults: AccountSummary[]): AccountSummary[] {
    const accounts = searchQuery.trim()
      ? currentSearchResults
      : selectedCategoryId === null
        ? vault?.accounts ?? []
        : selectedCategoryId === "uncategorized"
          ? (vault?.accounts ?? []).filter((account) => account.categoryId === null)
          : (vault?.accounts ?? []).filter((account) => account.categoryId === selectedCategoryId);

    // 未使用账号始终排在有记录账号之后；相同时间保持原有顺序，避免列表无故跳动。
    return accounts
      .map((account, index) => ({ account, index }))
      .sort((left, right) => {
        const leftTime = left.account.lastUsedAt;
        const rightTime = right.account.lastUsedAt;
        if (leftTime === null && rightTime === null) return left.index - right.index;
        if (leftTime === null) return 1;
        if (rightTime === null) return -1;
        const difference = accountSort === "recent" ? rightTime - leftTime : leftTime - rightTime;
        return difference || left.index - right.index;
      })
      .map(({ account }) => account);
  }

  function formatLastUsed(lastUsedAt: number | null): string {
    if (lastUsedAt === null) return "未使用";
    return new Intl.DateTimeFormat("zh-CN", {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      hour12: false,
    }).format(new Date(lastUsedAt));
  }

  function localDayKey(date: Date): string {
    const month = String(date.getMonth() + 1).padStart(2, "0");
    const day = String(date.getDate()).padStart(2, "0");
    return `${date.getFullYear()}-${month}-${day}`;
  }

  /** Rust 只返回最小事件摘要；自然日边界由运行界面的本地时区决定。 */
  function usageChart(events: UsageEvent[]): UsageChart {
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const weekdays = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];
    const days: UsageDay[] = [];
    const byKey = new Map<string, UsageDay>();

    for (let offset = 6; offset >= 0; offset -= 1) {
      const date = new Date(today);
      date.setDate(today.getDate() - offset);
      const day: UsageDay = {
        key: localDayKey(date),
        label: offset === 0 ? "今天" : weekdays[date.getDay()],
        accountUses: 0,
        passwordCopies: 0,
      };
      days.push(day);
      byKey.set(day.key, day);
    }

    for (const event of events) {
      if (!Number.isFinite(event.occurredAt)) continue;
      const day = byKey.get(localDayKey(new Date(event.occurredAt)));
      if (!day) continue;
      // 密码复制本身也是一次账号使用，因此第一组柱子统计所有成功复制事件。
      day.accountUses += 1;
      if (event.kind === "passwordCopy") day.passwordCopies += 1;
    }

    const highest = days.reduce(
      (maximum, day) => Math.max(maximum, day.accountUses, day.passwordCopies),
      0,
    );
    const maximum = Math.max(3, Math.ceil(highest / 3) * 3);
    return {
      days,
      maximum,
      ticks: [maximum, maximum * 2 / 3, maximum / 3, 0],
      hasData: highest > 0,
    };
  }

  function accountCategoryName(categoryId: string | null): string {
    if (!categoryId) return "未分类";
    return vault?.categories.find((category) => category.id === categoryId)?.name ?? "未知分类";
  }

  function clearRevealedPassword() {
    revealSequence += 1;
    revealedCredential = null;
    revealLoadingAccountId = null;
  }

  /** 同一时刻只保留一条临时密码；迟到响应不得恢复已隐藏或已切换的密码。 */
  async function toggleRevealedPassword(accountId: string) {
    if (revealedCredential?.accountId === accountId) {
      clearRevealedPassword();
      return;
    }

    const sequence = ++revealSequence;
    revealedCredential = null;
    revealLoadingAccountId = accountId;
    try {
      const result = await revealAccountPassword(accountId);
      if (sequence !== revealSequence || revealLoadingAccountId !== accountId) return;
      revealedCredential = { accountId, password: result.password };
    } catch (error) {
      if (sequence !== revealSequence) return;
      const commandError = error as Partial<CommandError> | null;
      showNotice(commandError?.message || "密码读取失败，请重试");
    } finally {
      if (sequence === revealSequence) revealLoadingAccountId = null;
    }
  }

  async function copyCredential(accountId: string, kind: "username" | "password") {
    if (credentialAction) return;
    credentialAction = { accountId, kind };
    try {
      vault = kind === "username"
        ? await copyAccountUsername(accountId)
        : await copyAccountPassword(accountId);
      if (searchQuery.trim()) await runSearch();
      showNotice(kind === "username" ? "账号名已复制" : "密码已复制，30 秒后自动清理");
    } catch (error) {
      const commandError = error as Partial<CommandError> | null;
      showNotice(commandError?.message || "复制失败，请重试");
    } finally {
      credentialAction = null;
    }
  }

  function openAccountFromRow(event: MouseEvent, accountId: string) {
    const target = event.target;
    if (target instanceof Element && target.closest("button")) return;
    clearRevealedPassword();
    void openEditAccountDialog(accountId);
  }

  function openAccountFromRowKey(event: KeyboardEvent, accountId: string) {
    if (event.target !== event.currentTarget || (event.key !== "Enter" && event.key !== " ")) return;
    event.preventDefault();
    clearRevealedPassword();
    void openEditAccountDialog(accountId);
  }

  /** 新增账号每次都使用全新的 Draft，取消后不会残留上一次输入。 */
  function openAccountDialog() {
    clearRevealedPassword();
    accountDialogMode = "create";
    editingAccountId = null;
    accountDetailLoaded = true;
    accountLoading = false;
    accountDraft = emptyAccountDraft();
    confirmedAccountDraft = null;
    if (selectedCategoryId && selectedCategoryId !== "uncategorized") {
      accountDraft.categoryId = selectedCategoryId;
    }
    accountError = "";
    accountUsernameError = "";
    accountPasswordError = "";
    accountPasswordVisible = false;
    accountDialogOpen = true;
    requestAnimationFrame(() => accountUsernameInput?.focus());
  }

  /** 只在用户点击一条账号行后读取该账号的编辑详情。 */
  async function openEditAccountDialog(accountId: string) {
    clearRevealedPassword();
    const sequence = ++accountLoadSequence;
    accountDialogMode = "edit";
    editingAccountId = accountId;
    accountDetailLoaded = false;
    accountLoading = true;
    accountDraft = emptyAccountDraft();
    confirmedAccountDraft = null;
    accountError = "";
    accountUsernameError = "";
    accountPasswordError = "";
    accountPasswordVisible = false;
    accountDialogOpen = true;

    try {
      const detail = await loadAccountForEdit(accountId);
      // 关闭弹框或切换目标后，迟到响应不得重新填入旧账号密码。
      if (sequence !== accountLoadSequence || !accountDialogOpen || editingAccountId !== accountId) return;
      const loadedDraft = {
        username: detail.username,
        password: detail.password,
        address: detail.address ?? "",
        email: detail.email ?? "",
        notes: detail.notes ?? "",
        categoryId: detail.categoryId ?? "uncategorized",
      };
      accountDraft = loadedDraft;
      // 保存失败时只能退回 Rust 已确认的数据，不能让未落盘的修改伪装成当前值。
      confirmedAccountDraft = { ...loadedDraft };
      accountDetailLoaded = true;
      requestAnimationFrame(() => accountUsernameInput?.focus());
    } catch (error) {
      if (sequence !== accountLoadSequence || !accountDialogOpen) return;
      const commandError = error as Partial<CommandError> | null;
      accountError = commandError?.message || "账号详情加载失败，请关闭后重试";
    } finally {
      if (sequence === accountLoadSequence) accountLoading = false;
    }
  }

  /** 关闭、取消和 Esc 只丢弃 Draft，不触发字段校验。 */
  function closeAccountDialog() {
    if (accountSubmitting) return;
    accountLoadSequence += 1;
    accountDialogOpen = false;
    accountDialogMode = "create";
    editingAccountId = null;
    accountDetailLoaded = false;
    accountLoading = false;
    accountDraft = emptyAccountDraft();
    confirmedAccountDraft = null;
    accountError = "";
    accountUsernameError = "";
    accountPasswordError = "";
  }

  function optionalField(value: string): string | null {
    return value.trim() ? value : null;
  }

  async function submitAccount(event: SubmitEvent) {
    event.preventDefault();
    accountUsernameError = accountDraft.username.trim() ? "" : "请输入账号名";
    accountPasswordError = accountDraft.password.trim() ? "" : "请输入密码";
    accountError = "";
    if (accountUsernameError || accountPasswordError) {
      if (accountUsernameError) accountUsernameInput?.focus();
      return;
    }

    if (accountDialogMode === "edit" && (!editingAccountId || !accountDetailLoaded)) return;
    accountSubmitting = true;
    try {
      const fields = {
        categoryId: accountDraft.categoryId === "uncategorized" ? null : accountDraft.categoryId,
        username: accountDraft.username,
        password: accountDraft.password,
        address: optionalField(accountDraft.address),
        email: optionalField(accountDraft.email),
        notes: optionalField(accountDraft.notes),
      };
      vault = accountDialogMode === "edit" && editingAccountId
        ? await updateAccount(editingAccountId, fields)
        : await createAccount(fields);
      clearRevealedPassword();
      if (searchQuery.trim()) await runSearch();
      const edited = accountDialogMode === "edit";
      accountDialogOpen = false;
      accountDialogMode = "create";
      editingAccountId = null;
      accountDetailLoaded = false;
      accountDraft = emptyAccountDraft();
      confirmedAccountDraft = null;
      showNotice(edited ? "账号修改已保存" : "账号已保存");
    } catch (error) {
      const commandError = error as Partial<CommandError> | null;
      if (
        commandError?.code === "VAULT_SAVE_FAILED" &&
        accountDialogMode === "edit" &&
        confirmedAccountDraft
      ) {
        // 编辑保存失败后恢复已确认快照；创建失败仍保留输入，方便用户直接重试。
        accountDraft = { ...confirmedAccountDraft };
        accountPasswordVisible = false;
        accountError = "密码库保存失败，已恢复上一次保存的内容，请重试";
      } else if (commandError?.code === "ACCOUNT_USERNAME_REQUIRED") accountUsernameError = "请输入账号名";
      else if (commandError?.code === "ACCOUNT_PASSWORD_REQUIRED") accountPasswordError = "请输入密码";
      else if (commandError?.code === "CATEGORY_NOT_FOUND") accountError = "所选分类不存在，请重新选择";
      else accountError = commandError?.message || "账号保存失败，请重试";
    } finally {
      accountSubmitting = false;
    }
  }

  function requestDeleteAccount() {
    if (accountDialogMode !== "edit" || !editingAccountId || !accountDetailLoaded) return;
    accountDeleteError = "";
    pendingDeleteAccount = {
      id: editingAccountId,
      username: accountDraft.username,
    };
  }

  function cancelDeleteAccount() {
    if (accountDeleteSubmitting) return;
    pendingDeleteAccount = null;
    accountDeleteError = "";
  }

  async function confirmDeleteAccount() {
    if (!pendingDeleteAccount || accountDeleteSubmitting) return;
    accountDeleteSubmitting = true;
    accountDeleteError = "";
    try {
      const deletedUsername = pendingDeleteAccount.username;
      vault = await deleteAccount(pendingDeleteAccount.id);
      clearRevealedPassword();
      if (searchQuery.trim()) await runSearch();
      pendingDeleteAccount = null;
      closeAccountDialog();
      showNotice(`账号“${deletedUsername}”已删除`);
    } catch (error) {
      const commandError = error as Partial<CommandError> | null;
      accountDeleteError = commandError?.message || "账号删除失败，请重试";
    } finally {
      accountDeleteSubmitting = false;
    }
  }

  function requestDeleteCategory(category: CategorySummary) {
    categoryMenu = null;
    if (category.accountCount > 0) {
      showNotice(`分类“${category.name}”中还有 ${category.accountCount} 个账号，不能删除`);
      return;
    }
    deleteError = "";
    pendingDeleteCategory = category;
  }

  function cancelDeleteCategory() {
    if (deleteSubmitting) return;
    pendingDeleteCategory = null;
    deleteError = "";
  }

  /** 关闭和取消只清理表单，不触发必填校验或后端写入。 */
  function closeCategoryDialog() {
    if (categorySubmitting) return;
    categoryDialogOpen = false;
    editingCategoryId = null;
    categoryName = "";
    categoryError = "";
  }

  function commandMessage(error: unknown): string {
    const commandError = error as Partial<CommandError> | null;
    if (commandError?.code === "CATEGORY_NAME_EMPTY") return "请输入分类名称";
    if (commandError?.code === "CATEGORY_NAME_DUPLICATE") return "该分类名称已经存在";
    if (commandError?.code === "CATEGORY_NOT_FOUND") return "分类不存在，请重新加载";
    if (commandError?.code === "CATEGORY_NOT_EMPTY") return "分类中仍有账号，请先处理分类内账号";
    if (commandError?.message) return commandError.message;
    return "分类保存失败，请重试";
  }

  async function submitCategory(event: SubmitEvent) {
    event.preventDefault();
    if (!categoryName.trim()) {
      categoryError = "请输入分类名称";
      categoryInput?.focus();
      return;
    }

    categorySubmitting = true;
    categoryError = "";
    try {
      vault = categoryDialogMode === "rename" && editingCategoryId
        ? await renameCategory(editingCategoryId, categoryName)
        : await createCategory(categoryName);
      categoryDialogOpen = false;
      editingCategoryId = null;
      categoryName = "";
    } catch (error) {
      categoryError = commandMessage(error);
    } finally {
      categorySubmitting = false;
    }
  }

  async function confirmDeleteCategory() {
    if (!pendingDeleteCategory || deleteSubmitting) return;
    deleteSubmitting = true;
    deleteError = "";
    try {
      const deletedName = pendingDeleteCategory.name;
      vault = await deleteCategory(pendingDeleteCategory.id);
      if (selectedCategoryId === pendingDeleteCategory.id) selectCategory(null);
      pendingDeleteCategory = null;
      showNotice(`分类“${deletedName}”已删除`);
    } catch (error) {
      const message = commandMessage(error);
      if ((error as Partial<CommandError> | null)?.code === "CATEGORY_NOT_EMPTY") {
        pendingDeleteCategory = null;
        showNotice(message);
      } else {
        deleteError = message;
      }
    } finally {
      deleteSubmitting = false;
    }
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    categoryMenu = null;
    if (pendingDeleteAccount) {
      cancelDeleteAccount();
      return;
    }
    if (pendingDeleteCategory) {
      cancelDeleteCategory();
      return;
    }
    if (categoryDialogOpen) closeCategoryDialog();
    if (accountDialogOpen) closeAccountDialog();
  }

  function handleWindowClick() {
    categoryMenu = null;
  }
</script>

<svelte:window onkeydown={handleWindowKeydown} onclick={handleWindowClick} />

<svelte:head>
  <title>Local Vault · 账号管理</title>
</svelte:head>

<div class="app-shell">
  <aside class="sidebar">
    <div class="brand" aria-label="Local Vault">
      <svg class="brand-mark" viewBox="0 0 24 24" aria-hidden="true">
        <path d="M12 2 4.8 5.9v6.2c0 4.7 2.9 7.9 7.2 9.9 4.3-2 7.2-5.2 7.2-9.9V5.9L12 2Z" />
        <path d="M9.2 11.5h5.6M12 8.7v5.6" />
      </svg>
      <span>Local Vault</span>
    </div>

    <p class="nav-label">密码库</p>
    <button class:active={selectedCategoryId === null} class="nav-item" type="button" onclick={() => selectCategory(null)}>
      <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="4" y="4" width="6" height="6" rx="1" /><rect x="14" y="4" width="6" height="6" rx="1" /><rect x="4" y="14" width="6" height="6" rx="1" /><rect x="14" y="14" width="6" height="6" rx="1" /></svg>
      <span>账号管理</span>
      <span class="count">{vault?.accountCount ?? 0}</span>
    </button>

    <div class="category-heading">
      <p class="nav-label">分类</p>
      <button class="icon-button quiet" type="button" onclick={openCategoryDialog} disabled={status !== "ready"} aria-label="新增分类" title="新建分类">
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 5v14M5 12h14" /></svg>
      </button>
    </div>
    <div class="category-list">
      {#each vault?.categories ?? [] as category (category.id)}
        <button class:active={selectedCategoryId === category.id} class="category-item" type="button" title={category.name} onclick={() => selectCategory(category.id)} oncontextmenu={(event) => openCategoryMenu(event, category)}>
          <span class="category-dot"></span>
          <span class="category-name">{category.name}</span>
          <span class="count">{category.accountCount}</span>
        </button>
      {/each}
      <button class:active={selectedCategoryId === "uncategorized"} class="category-item" type="button" title="未分类" onclick={() => selectCategory("uncategorized")}>
        <span class="category-dot muted"></span>
        <span class="category-name">未分类</span>
        <span class="count">{vault?.uncategorizedCount ?? 0}</span>
      </button>
    </div>

    <div class="vault-status">
      <span class:online={status === "ready"} class="status-dot"></span>
      <div>
        <strong>{status === "ready" ? "本地密码库已连接" : status === "recoveryRequired" || status === "recovering" ? "密码库等待恢复" : status === "unavailable" || status === "error" ? "密码库不可编辑" : "正在连接密码库"}</strong>
        <span>{vault ? `结构版本 ${vault.schemaVersion} · 数据版本 ${vault.revision}` : "数据只保存在当前设备"}</span>
      </div>
    </div>
  </aside>

  <main class="main-wrap">
    <section class="workspace">
      <header class="topbar">
        <h1>账号管理</h1>
        {#if status === "ready"}
        <div class="topbar-actions">
          <label class="search-box" aria-label="搜索账号">
            <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="6" /><path d="m16 16 4 4" /></svg>
            <input type="search" value={searchQuery} oninput={handleSearchInput} placeholder="搜索账号名、地址、邮箱或备注" disabled={status !== "ready"} />
          </label>
          <button class="secondary-button" type="button" onclick={refreshVault}>
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 11a8 8 0 1 0-2.3 5.7M20 4v7h-7" /></svg>
            重新加载
          </button>
          <button class="primary-button" type="button" onclick={openAccountDialog} disabled={status !== "ready"}>
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 5v14M5 12h14" /></svg>
            新增账号
          </button>
        </div>
        {/if}
      </header>

      <div class="content-scroll" aria-live="polite">
        {#if status === "recoveryRequired" || status === "recovering"}
          <section class="recovery-state" role="alert">
            <div class="recovery-copy">
              <span class="recovery-badge">备份恢复</span>
              <h2>{recoveryCancelled ? "已暂缓恢复" : "检测到可用的上一版备份"}</h2>
              <p>{recoveryCancelled ? "当前密码库保持不可编辑。点击重新加载后可以再次选择是否恢复。" : "主密码库无法使用，但上一版备份已经通过完整校验。恢复后将使用上一版数据，当前损坏现场会被保留。"}</p>
              {#if recoveryError}<p class="recovery-error" role="alert">{recoveryError}</p>{/if}
            </div>
            <div class="recovery-actions">
              {#if recoveryCancelled}
                <button class="secondary-button" type="button" onclick={refreshVault}>重新加载</button>
              {:else}
                <button class="secondary-button" type="button" onclick={() => (recoveryCancelled = true)} disabled={status === "recovering"}>暂不恢复</button>
                <button class="primary-button" type="button" onclick={confirmRecovery} disabled={status === "recovering"}>{status === "recovering" ? "恢复中" : "恢复上一版"}</button>
              {/if}
            </div>
          </section>
        {:else if status === "unavailable"}
          <section class="load-error" role="alert">
            <div>
              <strong>主密码库和备份都无法使用</strong>
              <p>为避免覆盖现有数据，应用已停止所有编辑操作，也不会创建新的空密码库。</p>
            </div>
            <button class="secondary-button" type="button" onclick={refreshVault}>重新检查</button>
          </section>
        {:else if status === "error"}
          <section class="load-error" role="alert">
            <div>
              <strong>无法读取本地密码库</strong>
              <p>{errorMessage || "请检查应用状态后重试。"}</p>
            </div>
            <button class="secondary-button" type="button" onclick={refreshVault}>重试</button>
          </section>
        {:else if status === "loading"}
          <section class="loading-state" role="status">正在读取并验证本地密码库…</section>
        {:else}
          {#if selectedCategoryId === null}
          <section class="metric-grid" aria-label="密码库概览">
            <article class="metric-card">
              <div class="metric-title">
                <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="8" r="3" /><path d="M6.5 19c.7-3.2 2.5-5 5.5-5s4.8 1.8 5.5 5" /></svg>
                <span>账号总数</span>
              </div>
              <strong>{vault?.accountCount ?? 0}</strong>
              <p>已确认保存在本地密码库中的账号</p>
            </article>
            <article class="metric-card">
              <div class="metric-title">
                <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 6h6l2 2h8v10H4Z" /></svg>
                <span>分类数量</span>
              </div>
              <strong>{vault?.categoryCount ?? 0}</strong>
              <p>用户创建的账号分类</p>
            </article>
            <article class="metric-card">
              <div class="metric-title">
                <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3 5 6v6c0 4 2.5 6.8 7 9 4.5-2.2 7-5 7-9V6Z" /></svg>
                <span>本地状态</span>
              </div>
              <strong class="status-value">{vault?.initialized ? "已初始化" : "已加载"}</strong>
              <p>{vault ? `当前数据版本 ${vault.revision}` : "正在等待 Rust 返回确认状态"}</p>
            </article>
          </section>

          <section class="panel overview-panel">
            <div class="panel-head">
              <div>
                <h2>使用概览</h2>
                <p>查看最近一周账号的使用情况</p>
              </div>
              <div class="overview-actions">
                <button class="overview-refresh" type="button" onclick={refreshVault} aria-label="刷新使用概览" title="刷新使用概览">
                  <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 11a8 8 0 1 0-2.3 5.7M20 4v7h-7" /></svg>
                </button>
                <span class="overview-range">近 7 天</span>
              </div>
            </div>
            {#if currentUsageChart.hasData}
              <div class="overview-body" aria-label="最近七天账号使用柱状图">
                <div class="chart-grid"><span></span><span></span><span></span><span></span></div>
                <div class="chart-labels">
                  {#each currentUsageChart.ticks as tick}<span>{tick}</span>{/each}
                </div>
                <div class="chart-bars">
                  {#each currentUsageChart.days as day}
                    <div class="bar-group" title={`${day.label}：账号使用 ${day.accountUses} 次，密码复制 ${day.passwordCopies} 次`}>
                      <i class="usage-bar" style={`height: ${(day.accountUses / currentUsageChart.maximum) * 100}%`}></i>
                      <i class="usage-bar secondary" style={`height: ${(day.passwordCopies / currentUsageChart.maximum) * 100}%`}></i>
                      <span class="bar-label">{day.label}</span>
                    </div>
                  {/each}
                </div>
              </div>
            {:else}
              <div class="overview-placeholder">
                <span class="guide-line line-one"></span>
                <span class="guide-line line-two"></span>
                <span class="guide-line line-three"></span>
                <span>暂无使用记录</span>
              </div>
            {/if}
            <div class="overview-foot">
              <span><i></i>账号使用</span>
              <span><i class="muted"></i>密码复制</span>
            </div>
          </section>
          {/if}

          <section class="panel table-panel">
            <div class="panel-head table-heading">
              <div>
                <h2>{selectedViewName()}</h2>
                <p>{searchQuery.trim() ? `${visibleAccounts(searchResults).length} 个匹配账号` : `${selectedAccountCount()} 个账号`}</p>
              </div>
              <label class="sort-control">
                <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 6h12M8 12h9M8 18h6M4 5v14" /></svg>
                <select bind:value={accountSort} aria-label="账号排序">
                  <option value="recent">最近使用优先</option>
                  <option value="earliest">最早使用优先</option>
                </select>
              </label>
            </div>
            <div class="table-wrap">
              <table>
                <thead>
                  <tr>
                    <th>账号名</th>
                    <th>地址</th>
                    <th>邮箱</th>
                    <th>分类</th>
                    <th>密码状态</th>
                    <th>最近使用</th>
                    <th aria-label="操作"></th>
                  </tr>
                </thead>
                {#if visibleAccounts(searchResults).length > 0}
                  <tbody>
                    {#each visibleAccounts(searchResults) as account (account.id)}
                      <tr class="account-row" role="button" tabindex="0" onclick={(event) => openAccountFromRow(event, account.id)} onkeydown={(event) => openAccountFromRowKey(event, account.id)}>
                        <td><strong class="account-name">{account.username}</strong></td>
                        <td title={account.address ?? ""}>{account.address || "—"}</td>
                        <td title={account.email ?? ""}>{account.email || "—"}</td>
                        <td>{accountCategoryName(account.categoryId)}</td>
                        <td>
                          <div class="password-cell">
                            <span class:revealed={revealedCredential?.accountId === account.id}>{revealedCredential?.accountId === account.id ? revealedCredential.password : "••••••••"}</span>
                          </div>
                        </td>
                        <td>{formatLastUsed(account.lastUsedAt)}</td>
                        <td>
                          <div class="credential-actions">
                            <button
                              class="credential-button"
                              type="button"
                              aria-label={revealedCredential?.accountId === account.id ? "隐藏密码" : "显示密码"}
                              title={revealedCredential?.accountId === account.id ? "隐藏密码" : "显示密码"}
                              disabled={revealLoadingAccountId === account.id}
                              onclick={(event) => { event.stopPropagation(); void toggleRevealedPassword(account.id); }}
                              onkeydown={(event) => event.stopPropagation()}
                            >
                              {revealLoadingAccountId === account.id ? "…" : revealedCredential?.accountId === account.id ? "隐藏" : "显示"}
                            </button>
                            <button
                              class="credential-button"
                              type="button"
                              aria-label="复制账号名"
                              title="复制账号名"
                              disabled={credentialAction !== null}
                              onclick={(event) => { event.stopPropagation(); void copyCredential(account.id, "username"); }}
                              onkeydown={(event) => event.stopPropagation()}
                            >账号</button>
                            <button
                              class="credential-button"
                              type="button"
                              aria-label="复制密码"
                              title="复制密码"
                              disabled={credentialAction !== null}
                              onclick={(event) => { event.stopPropagation(); void copyCredential(account.id, "password"); }}
                              onkeydown={(event) => event.stopPropagation()}
                            >密码</button>
                          </div>
                        </td>
                      </tr>
                    {/each}
                  </tbody>
                {/if}
              </table>
              {#if visibleAccounts(searchResults).length === 0}
                <div class="empty-state">
                  <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="4" y="5" width="16" height="14" rx="3" /><path d="M8 10h8M8 14h5" /></svg>
                  <strong>{searchStatus === "loading" ? "正在搜索账号" : searchStatus === "error" ? "搜索失败" : searchQuery.trim() ? "没有匹配账号" : selectedCategoryId === null ? "还没有保存账号" : "当前分类还没有账号"}</strong>
                  <span>{searchStatus === "loading" ? "请稍候" : searchStatus === "error" ? searchError : searchQuery.trim() ? "请尝试其他账号名、地址、邮箱或备注" : "点击右上角“新增账号”保存第一条记录"}</span>
                </div>
              {/if}
            </div>
          </section>
        {/if}
      </div>
    </section>
  </main>
</div>

{#if categoryMenu}
  <div class="category-context-menu" style={`left:${categoryMenu.x}px;top:${categoryMenu.y}px`} role="menu" tabindex="-1">
    <button type="button" role="menuitem" onclick={() => openRenameDialog(categoryMenu!.category)}>
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m4 20 4.2-1 10-10-3.2-3.2-10 10Z" /><path d="m13.8 7 3.2 3.2" /></svg>
      重命名
    </button>
    <button class="danger" type="button" role="menuitem" onclick={() => requestDeleteCategory(categoryMenu!.category)}>
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h16M9 7V4h6v3M7 7l1 13h8l1-13M10 11v5M14 11v5" /></svg>
      删除
    </button>
  </div>
{/if}

{#if accountDialogOpen}
  <div class="dialog-backdrop" role="presentation" onclick={(event) => event.target === event.currentTarget && closeAccountDialog()}>
    <div class="account-dialog" role="dialog" aria-modal="true" aria-labelledby="account-dialog-title">
      <form onsubmit={submitAccount} novalidate>
        <header class="dialog-head">
          <h2 id="account-dialog-title">{accountDialogMode === "edit" ? "编辑账号" : "新增账号"}</h2>
          <button class="dialog-close" type="button" onclick={closeAccountDialog} disabled={accountSubmitting} aria-label="关闭">×</button>
        </header>
        <div class="dialog-body account-form" class:account-form-loading={accountLoading}>
          {#if accountLoading}<p class="account-loading">正在读取账号详情…</p>{/if}
          <div class="field-grid">
            <div class="field">
              <label class="field-label" for="account-username">账号名 <span>*</span></label>
              <input id="account-username" class:invalid={Boolean(accountUsernameError)} bind:this={accountUsernameInput} bind:value={accountDraft.username} oninput={() => (accountUsernameError = "")} autocomplete="off" placeholder="例如：GitHub 工作账号" disabled={accountSubmitting || accountLoading || (accountDialogMode === "edit" && !accountDetailLoaded)} />
              {#if accountUsernameError}<p class="field-error" role="alert">{accountUsernameError}</p>{/if}
            </div>
            <div class="field">
              <label class="field-label" for="account-category">分类 <span>*</span></label>
              <select id="account-category" bind:value={accountDraft.categoryId} disabled={accountSubmitting || accountLoading || (accountDialogMode === "edit" && !accountDetailLoaded)}>
                {#each vault?.categories ?? [] as category (category.id)}
                  <option value={category.id}>{category.name}</option>
                {/each}
                <option value="uncategorized">未分类</option>
              </select>
            </div>
          </div>
          <div class="field">
            <label class="field-label" for="account-password">密码 <span>*</span></label>
            <div class="password-input">
              <input id="account-password" class:invalid={Boolean(accountPasswordError)} type={accountPasswordVisible ? "text" : "password"} bind:value={accountDraft.password} oninput={() => (accountPasswordError = "")} autocomplete="new-password" placeholder="请输入密码" disabled={accountSubmitting || accountLoading || (accountDialogMode === "edit" && !accountDetailLoaded)} />
              <button type="button" onclick={() => (accountPasswordVisible = !accountPasswordVisible)} disabled={accountSubmitting || accountLoading || (accountDialogMode === "edit" && !accountDetailLoaded)}>{accountPasswordVisible ? "隐藏" : "显示"}</button>
            </div>
            {#if accountPasswordError}<p class="field-error" role="alert">{accountPasswordError}</p>{/if}
          </div>
          <div class="field-grid">
            <div class="field">
              <label class="field-label" for="account-address">地址</label>
              <input id="account-address" bind:value={accountDraft.address} autocomplete="off" placeholder="https://example.com" disabled={accountSubmitting || accountLoading || (accountDialogMode === "edit" && !accountDetailLoaded)} />
            </div>
            <div class="field">
              <label class="field-label" for="account-email">邮箱</label>
              <input id="account-email" type="email" bind:value={accountDraft.email} autocomplete="off" placeholder="name@example.com" disabled={accountSubmitting || accountLoading || (accountDialogMode === "edit" && !accountDetailLoaded)} />
            </div>
          </div>
          <div class="field">
            <label class="field-label" for="account-notes">备注</label>
            <textarea id="account-notes" bind:value={accountDraft.notes} rows="3" placeholder="可选备注" disabled={accountSubmitting || accountLoading || (accountDialogMode === "edit" && !accountDetailLoaded)}></textarea>
          </div>
          {#if accountError}<p class="field-error account-submit-error" role="alert">{accountError}</p>{/if}
        </div>
        <footer class="dialog-foot">
          {#if accountDialogMode === "edit"}
            <button class="danger-button account-delete-button" type="button" onclick={requestDeleteAccount} disabled={accountSubmitting || accountLoading || !accountDetailLoaded}>删除账号</button>
          {/if}
          <button class="secondary-button" type="button" onclick={closeAccountDialog} disabled={accountSubmitting}>取消</button>
          <button class="primary-button" type="submit" disabled={accountSubmitting || accountLoading || (accountDialogMode === "edit" && !accountDetailLoaded)}>{accountSubmitting ? "保存中" : accountDialogMode === "edit" ? "保存修改" : "保存账号"}</button>
        </footer>
      </form>
    </div>
  </div>
{/if}

{#if pendingDeleteAccount}
  <div class="dialog-backdrop" role="presentation" onclick={(event) => event.target === event.currentTarget && cancelDeleteAccount()}>
    <div class="compact-dialog" role="alertdialog" aria-modal="true" aria-labelledby="delete-account-title" aria-describedby="delete-account-copy">
      <header class="dialog-head">
        <h2 id="delete-account-title">删除账号</h2>
        <button class="dialog-close" type="button" onclick={cancelDeleteAccount} disabled={accountDeleteSubmitting} aria-label="关闭">×</button>
      </header>
      <div class="dialog-body">
        <p class="dialog-copy" id="delete-account-copy">确定删除账号“<strong>{pendingDeleteAccount.username}</strong>”吗？删除后无法撤销。</p>
        {#if accountDeleteError}<p class="field-error" role="alert">{accountDeleteError}</p>{/if}
      </div>
      <footer class="dialog-foot">
        <button class="secondary-button" type="button" onclick={cancelDeleteAccount} disabled={accountDeleteSubmitting}>取消</button>
        <button class="danger-button" type="button" onclick={confirmDeleteAccount} disabled={accountDeleteSubmitting}>{accountDeleteSubmitting ? "删除中" : "确认删除"}</button>
      </footer>
    </div>
  </div>
{/if}

{#if pendingDeleteCategory}
  <div class="dialog-backdrop" role="presentation" onclick={(event) => event.target === event.currentTarget && cancelDeleteCategory()}>
    <div class="compact-dialog" role="alertdialog" aria-modal="true" aria-labelledby="delete-category-title" aria-describedby="delete-category-copy">
      <header class="dialog-head">
        <h2 id="delete-category-title">删除分类</h2>
        <button class="dialog-close" type="button" onclick={cancelDeleteCategory} disabled={deleteSubmitting} aria-label="关闭">×</button>
      </header>
      <div class="dialog-body">
        <p class="dialog-copy" id="delete-category-copy">确定删除分类“<strong>{pendingDeleteCategory.name}</strong>”吗？删除后无法撤销。</p>
        {#if deleteError}<p class="field-error" role="alert">{deleteError}</p>{/if}
      </div>
      <footer class="dialog-foot">
        <button class="secondary-button" type="button" onclick={cancelDeleteCategory} disabled={deleteSubmitting}>取消</button>
        <button class="danger-button" type="button" onclick={confirmDeleteCategory} disabled={deleteSubmitting}>{deleteSubmitting ? "删除中" : "确认删除"}</button>
      </footer>
    </div>
  </div>
{/if}

{#if notice}
  <div class="toast" role="status">{notice}</div>
{/if}

{#if categoryDialogOpen}
  <div class="dialog-backdrop" role="presentation" onclick={(event) => event.target === event.currentTarget && closeCategoryDialog()}>
    <div class="compact-dialog" role="dialog" aria-modal="true" aria-labelledby="category-dialog-title">
      <form onsubmit={submitCategory} novalidate>
        <header class="dialog-head">
          <h2 id="category-dialog-title">{categoryDialogMode === "create" ? "新建分类" : "重命名分类"}</h2>
          <button class="dialog-close" type="button" onclick={closeCategoryDialog} disabled={categorySubmitting} aria-label="关闭">×</button>
        </header>
        <div class="dialog-body">
          <label class="field-label" for="category-name">分类名称</label>
          <input
            id="category-name"
            class:invalid={Boolean(categoryError)}
            bind:this={categoryInput}
            bind:value={categoryName}
            oninput={() => (categoryError = "")}
            maxlength="24"
            autocomplete="off"
            placeholder="例如：工作"
            disabled={categorySubmitting}
          />
          {#if categoryError}<p class="field-error" role="alert">{categoryError}</p>{/if}
        </div>
        <footer class="dialog-foot">
          <button class="secondary-button" type="button" onclick={closeCategoryDialog} disabled={categorySubmitting}>取消</button>
          <button class="primary-button" type="submit" disabled={categorySubmitting}>{categorySubmitting ? "保存中" : categoryDialogMode === "create" ? "创建分类" : "保存修改"}</button>
        </footer>
      </form>
    </div>
  </div>
{/if}
    type AccountSummary,
