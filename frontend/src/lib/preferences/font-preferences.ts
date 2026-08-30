export const FONT_PRESETS = [
  {
    id: "balanced",
    label: "默认混合字体",
    description: "英文优先使用 Segoe UI，中文优先使用 MiSans。",
    stack: '"Segoe UI Variable", "SF Pro Text", "Helvetica Neue", "MiSans VF", "PingFang SC", "Noto Sans SC", "Microsoft YaHei UI", sans-serif',
  },
  {
    id: "misans",
    label: "MiSans",
    description: "中英文统一优先使用 MiSans，字形简洁。",
    stack: '"MiSans VF", "Segoe UI Variable", "PingFang SC", "Noto Sans SC", "Microsoft YaHei UI", sans-serif',
  },
  {
    id: "windows",
    label: "Windows 界面字体",
    description: "优先使用 Segoe UI Variable 和 Microsoft YaHei UI。",
    stack: '"Segoe UI Variable", "Microsoft YaHei UI", "Microsoft YaHei", sans-serif',
  },
  {
    id: "system",
    label: "系统默认字体",
    description: "跟随当前系统和 WebView 的界面字体。",
    stack: 'system-ui, "Segoe UI", "Microsoft YaHei UI", sans-serif',
  },
] as const;

export const FONT_SIZE_OPTIONS = [
  { value: 15, label: "紧凑" },
  { value: 16, label: "标准" },
  { value: 17, label: "舒适" },
  { value: 18, label: "较大" },
] as const;

export type FontPresetId = (typeof FONT_PRESETS)[number]["id"];
export type FontSize = (typeof FONT_SIZE_OPTIONS)[number]["value"];

export type FontPreferences = {
  preset: FontPresetId;
  size: FontSize;
};

export const DEFAULT_FONT_PREFERENCES: FontPreferences = {
  preset: "balanced",
  size: 16,
};

const STORAGE_KEY = "local-vault.font-preferences.v1";

function isPreset(value: unknown): value is FontPresetId {
  return FONT_PRESETS.some((preset) => preset.id === value);
}

function isSize(value: unknown): value is FontSize {
  return FONT_SIZE_OPTIONS.some((option) => option.value === value);
}

/** 本地偏好损坏或不可读取时直接回退，不影响密码库启动。 */
export function loadFontPreferences(storage: Storage): FontPreferences {
  try {
    const raw = storage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULT_FONT_PREFERENCES };
    const candidate = JSON.parse(raw) as Partial<FontPreferences>;
    return {
      preset: isPreset(candidate.preset) ? candidate.preset : DEFAULT_FONT_PREFERENCES.preset,
      size: isSize(candidate.size) ? candidate.size : DEFAULT_FONT_PREFERENCES.size,
    };
  } catch {
    return { ...DEFAULT_FONT_PREFERENCES };
  }
}

/** 保存失败不会撤销当前会话已应用的字体，只把失败结果交给界面提示。 */
export function saveFontPreferences(storage: Storage, preferences: FontPreferences): boolean {
  try {
    storage.setItem(STORAGE_KEY, JSON.stringify(preferences));
    return true;
  } catch {
    return false;
  }
}

export function applyFontPreferences(root: HTMLElement, preferences: FontPreferences) {
  const preset = FONT_PRESETS.find((candidate) => candidate.id === preferences.preset)
    ?? FONT_PRESETS[0];
  root.style.setProperty("--app-font-family", preset.stack);
  root.style.fontSize = `${preferences.size}px`;
}
