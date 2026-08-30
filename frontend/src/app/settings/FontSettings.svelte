<script lang="ts">
  import {
    FONT_PRESETS,
    FONT_SIZE_OPTIONS,
    type FontPreferences,
    type FontPresetId,
    type FontSize,
  } from "../../lib/preferences/font-preferences";

  export let preferences: FontPreferences;
  export let onChange: (preferences: FontPreferences) => void;

  function changePreset(event: Event) {
    onChange({
      ...preferences,
      preset: (event.currentTarget as HTMLSelectElement).value as FontPresetId,
    });
  }

  function changeSize(event: Event) {
    onChange({
      ...preferences,
      size: Number((event.currentTarget as HTMLSelectElement).value) as FontSize,
    });
  }
</script>

<section class="settings-section" aria-labelledby="font-settings-title">
  <header class="settings-section-head">
    <div>
      <h2 id="font-settings-title">字体设置</h2>
      <p>调整整个界面的字体类型和字号，修改后立即生效。</p>
    </div>
    <span>本机保存</span>
  </header>

  <div class="settings-card">
    <div class="font-controls">
      <label>
        <span>字体类型</span>
        <select value={preferences.preset} onchange={changePreset}>
          {#each FONT_PRESETS as preset}
            <option value={preset.id}>{preset.label}</option>
          {/each}
        </select>
        <small>{FONT_PRESETS.find((preset) => preset.id === preferences.preset)?.description}</small>
      </label>

      <label>
        <span>字体大小</span>
        <select value={preferences.size} onchange={changeSize}>
          {#each FONT_SIZE_OPTIONS as option}
            <option value={option.value}>{option.label} · {option.value}px</option>
          {/each}
        </select>
        <small>只调整文字大小，不整体缩放窗口和控件。</small>
      </label>
    </div>

    <div class="font-preview" aria-label="字体效果预览">
      <span>即时预览</span>
      <strong>Local Vault 本地密码库</strong>
      <p>账号管理、分类与安全存储 · Aa 123456</p>
    </div>
  </div>
</section>

<style>
  .settings-section {
    max-width: 900px;
  }

  .settings-section-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
    margin-bottom: 0.75rem;
  }

  .settings-section-head h2 {
    color: #30302e;
    font-size: 0.875rem;
    font-weight: 650;
  }

  .settings-section-head p {
    margin-top: 0.3125rem;
    color: var(--muted);
    font-size: 0.71875rem;
  }

  .settings-section-head > span {
    border-radius: 0.375rem;
    background: #eaf5ef;
    padding: 0.25rem 0.4375rem;
    color: #287a56;
    font-size: 0.65625rem;
    font-weight: 620;
  }

  .settings-card {
    display: grid;
    grid-template-columns: minmax(0, 1.15fr) minmax(260px, 0.85fr);
    gap: 1.125rem;
    border: 1px solid var(--line);
    border-radius: 0.6875rem;
    background: #fff;
    padding: 1rem;
  }

  .font-controls {
    display: grid;
    gap: 0.875rem;
  }

  label,
  label > span,
  label > small {
    display: block;
  }

  label > span {
    margin-bottom: 0.4375rem;
    color: #4e4e4a;
    font-size: 0.75rem;
    font-weight: 620;
  }

  select {
    width: 100%;
    height: 2.375rem;
    border: 1px solid #d8d8d4;
    border-radius: 0.5rem;
    outline: 0;
    background: #fff;
    padding: 0 0.6875rem;
    color: #333331;
    font-size: 0.75rem;
  }

  select:focus { border-color: #7771ba; }

  label > small {
    margin-top: 0.375rem;
    color: var(--muted);
    font-size: 0.65625rem;
    line-height: 1.5;
  }

  .font-preview {
    display: flex;
    min-height: 150px;
    justify-content: center;
    flex-direction: column;
    border: 1px solid #e5e5e2;
    border-radius: 0.625rem;
    background: #fafafa;
    padding: 1.125rem;
  }

  .font-preview > span {
    margin-bottom: 0.75rem;
    color: #8b8b87;
    font-size: 0.65625rem;
  }

  .font-preview strong {
    color: #292927;
    font-size: 1.125rem;
    font-weight: 650;
  }

  .font-preview p {
    margin-top: 0.5rem;
    color: #666662;
    font-size: 0.75rem;
    line-height: 1.6;
  }

  @media (max-width: 1050px) {
    .settings-card { grid-template-columns: 1fr; }
    .font-preview { min-height: 120px; }
  }
</style>
