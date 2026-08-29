<script lang="ts">
  import { onMount } from "svelte";
  import { loadVault, type VaultStatus } from "../lib/tauri/commands";

  let status: "loading" | "ready" | "error" = "loading";
  let vault: VaultStatus | null = null;
  let errorMessage = "";

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
</script>

<svelte:head>
  <title>Local Vault · 账号管理</title>
</svelte:head>

<div class="app-shell">
  <aside class="sidebar">
    <div class="brand">
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M12 2 4.8 5.9v6.2c0 4.7 2.9 7.9 7.2 9.9 4.3-2 7.2-5.2 7.2-9.9V5.9L12 2Z" />
        <path d="M9.2 11.5h5.6M12 8.7v5.6" />
      </svg>
      <span>Local Vault</span>
    </div>

    <p class="nav-label">密码库</p>
    <div class="nav-item active">
      <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="4" y="4" width="6" height="6" rx="1" /><rect x="14" y="4" width="6" height="6" rx="1" /><rect x="4" y="14" width="6" height="6" rx="1" /><rect x="14" y="14" width="6" height="6" rx="1" /></svg>
      <span>账号管理</span>
      <span class="count">{vault?.accountCount ?? 0}</span>
    </div>

    <div class="vault-status">
      <span class:online={status === "ready"} class="status-dot"></span>
      <div>
        <strong>{status === "ready" ? "本地密码库已连接" : status === "error" ? "密码库连接失败" : "正在连接密码库"}</strong>
        <span>数据只保存在当前设备</span>
      </div>
    </div>
  </aside>

  <main class="main-wrap">
    <section class="workspace">
      <header class="topbar">
        <div>
          <h1>账号管理</h1>
          <p>当前仅展示 Rust 后端确认的启动状态</p>
        </div>
        <button type="button" onclick={refreshVault} disabled={status === "loading"}>
          {status === "loading" ? "加载中" : "重新加载"}
        </button>
      </header>

      <div class="content" aria-live="polite">
        {#if status === "error"}
          <section class="load-error" role="alert">
            <div>
              <strong>无法读取本地密码库</strong>
              <p>{errorMessage || "请检查应用状态后重试。"}</p>
            </div>
            <button type="button" onclick={refreshVault}>重试</button>
          </section>
        {:else}
          <section class="metric-grid" aria-label="密码库启动摘要">
            <article>
              <span>账号总数</span>
              <strong>{vault?.accountCount ?? 0}</strong>
            </article>
            <article>
              <span>分类数量</span>
              <strong>{vault?.categoryCount ?? 0}</strong>
            </article>
            <article>
              <span>数据版本</span>
              <strong>{vault?.revision ?? 0}</strong>
            </article>
          </section>

          <section class="account-panel">
            <div class="panel-heading">
              <div>
                <h2>全部账号</h2>
                <p>{vault?.accountCount ?? 0} 个账号</p>
              </div>
              <span>{vault ? `结构版本 ${vault.schemaVersion}` : "正在读取"}</span>
            </div>
            <div class="empty-state">
              <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="4" y="5" width="16" height="14" rx="3" /><path d="M8 10h8M8 14h5" /></svg>
              <strong>{status === "loading" ? "正在读取账号" : "还没有保存账号"}</strong>
              <span>{status === "loading" ? "请稍候" : "账号新增将在对应后端切片中接入"}</span>
            </div>
          </section>
        {/if}
      </div>
    </section>
  </main>
</div>
