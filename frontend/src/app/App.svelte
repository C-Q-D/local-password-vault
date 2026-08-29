<script lang="ts">
  import { onMount } from "svelte";
  import {
    createCategory,
    loadVault,
    renameCategory,
    type CategorySummary,
    type CommandError,
    type VaultStatus,
  } from "../lib/tauri/commands";

  let status: "loading" | "ready" | "error" = "loading";
  let vault: VaultStatus | null = null;
  let errorMessage = "";
  let categoryDialogOpen = false;
  let categoryDialogMode: "create" | "rename" = "create";
  let editingCategoryId: string | null = null;
  let categoryMenu: { category: CategorySummary; x: number; y: number } | null = null;
  let categoryName = "";
  let categoryError = "";
  let categorySubmitting = false;
  let categoryInput: HTMLInputElement;

  /** 每次只接受 Rust 已确认的摘要；调用失败时不伪装成空密码库。 */
  async function refreshVault() {
    status = "loading";
    errorMessage = "";

    try {
      vault = await loadVault();
      status = "ready";
    } catch (error) {
      vault = null;
      status = "error";
      errorMessage = error instanceof Error ? error.message : String(error);
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

  function handleWindowKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    categoryMenu = null;
    if (categoryDialogOpen) closeCategoryDialog();
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
    <button class="nav-item active" type="button">
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
        <button class="category-item" type="button" title={category.name} oncontextmenu={(event) => openCategoryMenu(event, category)}>
          <span class="category-dot"></span>
          <span class="category-name">{category.name}</span>
          <span class="count">{category.accountCount}</span>
        </button>
      {/each}
      <button class="category-item" type="button" title="未分类">
        <span class="category-dot muted"></span>
        <span class="category-name">未分类</span>
        <span class="count">{vault?.uncategorizedCount ?? 0}</span>
      </button>
    </div>

    <div class="vault-status">
      <span class:online={status === "ready"} class="status-dot"></span>
      <div>
        <strong>{status === "ready" ? "本地密码库已连接" : status === "error" ? "密码库连接失败" : "正在连接密码库"}</strong>
        <span>{vault ? `结构版本 ${vault.schemaVersion} · 数据版本 ${vault.revision}` : "数据只保存在当前设备"}</span>
      </div>
    </div>
  </aside>

  <main class="main-wrap">
    <section class="workspace">
      <header class="topbar">
        <h1>账号管理</h1>
        <div class="topbar-actions">
          <label class="search-box" aria-label="搜索账号">
            <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="6" /><path d="m16 16 4 4" /></svg>
            <input type="search" placeholder="搜索账号名、地址、邮箱或备注" disabled />
          </label>
          <button class="secondary-button" type="button" onclick={refreshVault} disabled={status === "loading"}>
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 11a8 8 0 1 0-2.3 5.7M20 4v7h-7" /></svg>
            {status === "loading" ? "加载中" : "重新加载"}
          </button>
          <button class="primary-button" type="button" disabled title="等待账号后端接入">
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 5v14M5 12h14" /></svg>
            新增账号
          </button>
        </div>
      </header>

      <div class="content-scroll" aria-live="polite">
        {#if status === "error"}
          <section class="load-error" role="alert">
            <div>
              <strong>无法读取本地密码库</strong>
              <p>{errorMessage || "请检查应用状态后重试。"}</p>
            </div>
            <button class="secondary-button" type="button" onclick={refreshVault}>重试</button>
          </section>
        {:else}
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
              <strong class="status-value">{status === "loading" ? "读取中" : vault?.initialized ? "已初始化" : "已加载"}</strong>
              <p>{vault ? `当前数据版本 ${vault.revision}` : "正在等待 Rust 返回确认状态"}</p>
            </article>
          </section>

          <section class="panel overview-panel">
            <div class="panel-head">
              <div>
                <h2>使用概览</h2>
                <p>查看账号使用情况</p>
              </div>
              <span class="status-pill">本地加密</span>
            </div>
            <div class="overview-placeholder">
              <span class="guide-line line-one"></span>
              <span class="guide-line line-two"></span>
              <span class="guide-line line-three"></span>
              <span>暂无使用记录</span>
            </div>
            <div class="overview-foot">
              <span><i></i>账号使用</span>
              <span><i class="muted"></i>密码复制</span>
            </div>
          </section>

          <section class="panel table-panel">
            <div class="panel-head table-heading">
              <div>
                <h2>全部账号</h2>
                <p>{vault?.accountCount ?? 0} 个账号</p>
              </div>
              <label class="sort-control">
                <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 6h12M8 12h9M8 18h6M4 5v14" /></svg>
                <select disabled aria-label="账号排序">
                  <option>最近使用优先</option>
                  <option>最早使用优先</option>
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
              </table>
              <div class="empty-state">
                <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="4" y="5" width="16" height="14" rx="3" /><path d="M8 10h8M8 14h5" /></svg>
                <strong>{status === "loading" ? "正在读取账号" : "还没有保存账号"}</strong>
                <span>{status === "loading" ? "请稍候" : "新增账号功能接入后，可在这里管理账号"}</span>
              </div>
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
  </div>
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
