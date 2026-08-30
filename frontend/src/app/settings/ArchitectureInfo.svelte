<!-- 本页只解释当前源码已经实现的机制，不读取密码库内容或本机真实路径。 -->
<section class="architecture-section" aria-labelledby="architecture-title">
  <header class="section-head">
    <div>
      <h2 id="architecture-title">项目架构与数据保护</h2>
      <p>这里说明当前软件如何从界面调用 Rust、怎样保存数据，以及加密方案为什么这样选择。</p>
    </div>
    <span>当前实现</span>
  </header>

  <div class="flow-card" aria-label="从界面到磁盘的数据流">
    <div class="flow-node">
      <strong>Svelte 前端</strong>
      <span>展示界面与收集操作</span>
    </div>
    <b aria-hidden="true">→</b>
    <div class="flow-node">
      <strong>Tauri command</strong>
      <span>转换参数与返回结果</span>
    </div>
    <b aria-hidden="true">→</b>
    <div class="flow-node">
      <strong>Rust 核心</strong>
      <span>校验规则与确认状态</span>
    </div>
    <b aria-hidden="true">→</b>
    <div class="flow-node">
      <strong>Windows 适配</strong>
      <span>DPAPI、文件与剪贴板</span>
    </div>
  </div>

  <div class="info-grid">
    <article class="info-card">
      <header>
        <span class="step">01</span>
        <div>
          <h3>各层分别负责什么</h3>
          <p>职责分开后，界面不能绕过 Rust 直接读取密码库文件。</p>
        </div>
      </header>
      <ul>
        <li><strong>Svelte：</strong>只负责界面状态、输入与结果展示。</li>
        <li><strong>Tauri command：</strong>是很薄的调用边界，只转换参数并调用应用层。</li>
        <li><strong>vault-app：</strong>协调读取、保存和恢复，只把已成功写入并重新验证的状态当作当前状态。</li>
        <li><strong>vault-core / vault-storage：</strong>分别负责业务规则、文档结构和加密信封。</li>
        <li><strong>Windows 平台层：</strong>负责 DPAPI、原子文件替换、进程锁和系统剪贴板。</li>
      </ul>
    </article>

    <article class="info-card">
      <header>
        <span class="step">02</span>
        <div>
          <h3>数据保存在什么地方</h3>
          <p>使用 Tauri 提供的应用数据目录，不把真实绝对路径暴露给前端。</p>
        </div>
      </header>
      <div class="file-list">
        <div><code>vault.dat</code><span>当前已确认的加密密码库主文件</span></div>
        <div><code>vault.backup.dat</code><span>文件替换时保留的上一份加密备份</span></div>
        <div><code>vault.lock</code><span>独占进程锁，避免两个程序实例同时写入</span></div>
      </div>
      <p class="note">当前没有数据库、HTTP 服务或云端同步。通过校验的密码库 JSON 会作为一个整体放进版本化加密信封，再写入本机文件。</p>
    </article>

    <article class="info-card wide">
      <header>
        <span class="step">03</span>
        <div>
          <h3>AES-256-GCM 如何保护密码库</h3>
          <p>它不是简单地把文字“打乱”，而是同时完成保密和篡改检测。</p>
        </div>
      </header>
      <div class="explain-columns">
        <div>
          <h4>每次保存发生什么</h4>
          <ol>
            <li>生成一个全新的随机 32 字节数据密钥，即 256 位 AES 密钥。</li>
            <li>生成一个全新的随机 12 字节 nonce，确保同一密钥下每次加密输入不同。</li>
            <li>AES-256-GCM 把密码库 JSON 加密为密文，同时生成 16 字节认证标签。</li>
            <li>版本、长度和被 DPAPI 包装后的密钥等文件头作为 AAD 参与认证，文件头被改动也会验证失败。</li>
            <li>读取时先验证认证标签，成功后才解析 JSON；验证失败的内容不会进入业务层。</li>
          </ol>
        </div>
        <div>
          <h4>算法原理</h4>
          <p><strong>AES</strong> 是对称分组加密算法；256 表示密钥长度。GCM 使用计数器方式生成密钥流来隐藏内容，并在有限域中计算认证值来检测密文和附加数据是否被修改。</p>
          <p>因此只有“解密出来”还不够，认证标签也必须正确。文件被截断、替换字节或拿错密钥时，验证会失败，而不是把损坏内容当成正常 JSON 使用。</p>
        </div>
      </div>
    </article>

    <article class="info-card">
      <header>
        <span class="step">04</span>
        <div>
          <h3>DPAPI 负责什么</h3>
          <p>DPAPI 不直接加密整个密码库，只保护 AES 数据密钥。</p>
        </div>
      </header>
      <p class="body-copy">软件使用 Windows DPAPI 的 <code>CurrentUser</code> 范围包装和解包 32 字节数据密钥。这样无需额外设置主密码，数据密钥的解锁能力绑定到当前 Windows 用户配置；保存到文件里的只有包装结果，不是明文密钥。</p>
      <p class="note">DPAPI 与 AES-GCM 是分工关系：DPAPI 管本机密钥保护，AES-GCM 管密码库内容的保密性和完整性。</p>
    </article>

    <article class="info-card">
      <header>
        <span class="step">05</span>
        <div>
          <h3>为什么选择这套组合</h3>
          <p>目标是本机直接启动、实现边界清楚，并且能检测文件篡改。</p>
        </div>
      </header>
      <ul>
        <li><strong>AES-256-GCM：</strong>一个标准 AEAD 算法同时提供保密和认证，Windows 设备通常有良好的硬件加速。</li>
        <li><strong>不只用 DPAPI 加密整文件：</strong>只包装小型数据密钥，可以把平台相关部分限制在密钥管理层，并由应用掌控版本化文件格式。</li>
        <li><strong>不选 AES-CBC 或 CTR 单独使用：</strong>它们没有内建认证，还要正确组合独立 MAC，更容易出现校验顺序或密钥复用错误。</li>
        <li><strong>不采用口令派生密钥：</strong>那需要主密码、KDF 参数和恢复流程，不符合当前打开软件即可使用的产品方式。</li>
        <li><strong>ChaCha20-Poly1305 也安全可用：</strong>但当前目标设备上 AES-GCM 已成熟且性能好，替换算法不会带来当前可见收益。</li>
      </ul>
    </article>

    <article class="info-card wide">
      <header>
        <span class="step">06</span>
        <div>
          <h3>保存失败时怎样避免破坏旧数据</h3>
          <p>加密解决内容保护；原子替换、备份与重新验证解决写入中断和损坏。</p>
        </div>
      </header>
      <div class="save-flow">
        <span>写入唯一临时文件</span>
        <b>→</b>
        <span><code>sync_all</code> 刷入磁盘</span>
        <b>→</b>
        <span><code>ReplaceFileW</code> 发布主文件并保留备份</span>
        <b>→</b>
        <span>重新打开、解密和校验</span>
        <b>→</b>
        <span>更新内存中的确认状态</span>
      </div>
      <p class="body-copy">首次保存使用不覆盖现有文件的发布方式；后续保存用 Windows 原子替换。启动时主文件有效就正常加载；主文件缺失或损坏而备份有效时，界面会要求确认恢复。主文件和备份都无效时不会创建空密码库覆盖现场。</p>
    </article>

    <aside class="security-boundary wide" aria-label="安全边界">
      <strong>安全边界</strong>
      <p>这套机制主要保护磁盘上的静态数据，并降低文件损坏风险。已经取得当前 Windows 用户权限的恶意进程、程序运行时内存，以及复制到系统剪贴板后的内容，不属于 AES-GCM 静态加密能够单独解决的范围。</p>
    </aside>
  </div>
</section>

<style>
  .architecture-section { max-width: 1120px; }

  .section-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 12px;
  }

  .section-head h2 {
    color: #30302e;
    font-size: 0.875rem;
    font-weight: 650;
  }

  .section-head p {
    margin-top: 5px;
    color: var(--muted);
    font-size: 0.71875rem;
  }

  .section-head > span {
    border-radius: 6px;
    background: #f0eff9;
    padding: 4px 7px;
    color: #514b9b;
    font-size: 0.65625rem;
    font-weight: 620;
    white-space: nowrap;
  }

  .flow-card {
    display: grid;
    grid-template-columns: repeat(7, auto);
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-bottom: 12px;
    border: 1px solid var(--line);
    border-radius: 11px;
    background: #fff;
    padding: 14px;
  }

  .flow-card > b { color: #aaa9a5; font-weight: 500; }

  .flow-node { min-width: 0; }
  .flow-node strong,
  .flow-node span { display: block; }
  .flow-node strong { color: #343431; font-size: 0.75rem; }
  .flow-node span { margin-top: 4px; color: var(--muted); font-size: 0.65625rem; }

  .info-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
  }

  .info-card,
  .security-boundary {
    border: 1px solid var(--line);
    border-radius: 11px;
    background: #fff;
    padding: 15px;
  }

  .wide { grid-column: 1 / -1; }

  .info-card > header {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    margin-bottom: 12px;
  }

  .step {
    display: grid;
    width: 28px;
    height: 28px;
    flex: 0 0 auto;
    place-items: center;
    border-radius: 7px;
    background: #f2f1f8;
    color: #57519d;
    font-size: 0.625rem;
    font-weight: 700;
  }

  h3,
  h4 { margin: 0; color: #343431; }
  h3 { font-size: 0.8125rem; font-weight: 650; }
  h4 { margin-bottom: 8px; font-size: 0.75rem; font-weight: 650; }
  header p { margin-top: 4px; color: var(--muted); font-size: 0.65625rem; line-height: 1.55; }

  ul,
  ol {
    margin: 0;
    padding-left: 19px;
    color: #5c5c58;
    font-size: 0.71875rem;
    line-height: 1.75;
  }

  li + li { margin-top: 4px; }
  li strong { color: #3f3f3c; }

  code {
    color: #514b91;
    font-family: "Cascadia Code", "SFMono-Regular", Consolas, monospace;
    font-size: 0.95em;
  }

  .file-list { display: grid; gap: 7px; }
  .file-list > div {
    display: grid;
    grid-template-columns: 132px minmax(0, 1fr);
    gap: 8px;
    border-radius: 7px;
    background: #f8f8f7;
    padding: 8px 10px;
    color: #666662;
    font-size: 0.6875rem;
  }

  .note {
    margin-top: 10px;
    border-left: 2px solid #aba7d5;
    padding-left: 9px;
    color: #666662;
    font-size: 0.6875rem;
    line-height: 1.65;
  }

  .explain-columns {
    display: grid;
    grid-template-columns: minmax(0, 1.25fr) minmax(280px, 0.75fr);
    gap: 20px;
  }

  .explain-columns p,
  .body-copy {
    color: #5c5c58;
    font-size: 0.71875rem;
    line-height: 1.75;
  }

  .explain-columns p + p { margin-top: 8px; }

  .save-flow {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 11px;
    overflow-x: auto;
    padding-bottom: 3px;
  }

  .save-flow span {
    flex: 0 0 auto;
    border-radius: 7px;
    background: #f7f7f5;
    padding: 8px 10px;
    color: #50504d;
    font-size: 0.6875rem;
  }

  .save-flow b { color: #aaa9a5; font-size: 0.6875rem; font-weight: 500; }

  .security-boundary {
    border-color: #e5d8d8;
    background: #fffafa;
  }

  .security-boundary strong { color: #874848; font-size: 0.75rem; }
  .security-boundary p {
    margin-top: 6px;
    color: #775f5f;
    font-size: 0.6875rem;
    line-height: 1.7;
  }

  @media (max-width: 980px) {
    .flow-card { grid-template-columns: 1fr; }
    .flow-card > b { transform: rotate(90deg); justify-self: start; }
    .info-grid,
    .explain-columns { grid-template-columns: 1fr; }
    .wide { grid-column: auto; }
  }
</style>
