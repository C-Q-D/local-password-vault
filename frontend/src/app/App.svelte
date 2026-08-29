<script lang="ts">
  import { onMount } from "svelte";
  import { checkStack } from "../lib/tauri/commands";

  let status: "idle" | "checking" | "success" | "error" = "idle";
  let message = "等待验证 Svelte → Tauri → Rust command";
  let rustVersion = "尚未获取";

  async function runCheck() {
    status = "checking";
    message = "正在调用 Rust command…";

    try {
      const result = await checkStack("本地密码库");
      status = "success";
      message = result.message;
      rustVersion = result.rustVersion;
    } catch (error) {
      status = "error";
      message = error instanceof Error ? error.message : String(error);
    }
  }

  onMount(() => {
    void runCheck();
  });
</script>

<svelte:head>
  <title>Local Password Vault · 技术实验</title>
</svelte:head>

<div class="shell">
  <aside class="sidebar">
    <div class="brand">
      <span class="brand-mark">◇</span>
      <span>Local Vault</span>
    </div>
    <nav aria-label="实验导航">
      <button class="nav-item active" type="button">技术实验</button>
      <button class="nav-item" type="button" disabled>账号管理</button>
      <button class="nav-item" type="button" disabled>分类管理</button>
    </nav>
    <p class="sidebar-note">当前只验证技术链路，业务功能将在后续切片实现。</p>
  </aside>

  <main>
    <header>
      <div>
        <p class="eyebrow">TAURI 2 + SVELTE</p>
        <h1>最小技术实验</h1>
      </div>
      <span class:success={status === "success"} class:error={status === "error"} class="status-badge">
        {status === "success" ? "链路正常" : status === "error" ? "验证失败" : "等待验证"}
      </span>
    </header>

    <section class="card" aria-live="polite">
      <div class="card-heading">
        <div>
          <h2>Rust command 调用</h2>
          <p>点击按钮后，Svelte 通过 Tauri IPC 调用 Rust，并展示结构化返回值。</p>
        </div>
        <button class="primary" type="button" onclick={runCheck} disabled={status === "checking"}>
          {status === "checking" ? "验证中…" : "开始验证"}
        </button>
      </div>

      <dl>
        <div>
          <dt>返回消息</dt>
          <dd>{message}</dd>
        </div>
        <div>
          <dt>Rust 版本约束</dt>
          <dd>{rustVersion}</dd>
        </div>
      </dl>
    </section>

    <section class="scope-grid" aria-label="实验范围">
      <article>
        <span>01</span>
        <h3>前端</h3>
        <p>Svelte 5、TypeScript、Vite</p>
      </article>
      <article>
        <span>02</span>
        <h3>桌面桥接</h3>
        <p>Tauri 2 command / invoke</p>
      </article>
      <article>
        <span>03</span>
        <h3>后端</h3>
        <p>Rust stable workspace</p>
      </article>
    </section>
  </main>
</div>
