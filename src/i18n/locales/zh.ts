/**
 * Chinese strings. This file is the source of truth for the key set: the
 * English file is typed against it, so a missing key anywhere fails to
 * compile.
 */
export const zh = {
  app: {
    name: "ScreenCut",
  },

  header: {
    openLogs: "打开日志文件",
    minimize: "最小化",
    close: "关闭",
    language: "语言",
    languageZh: "中文",
    languageEn: "English",
    dragHint: "按住移动窗口",
  },

  source: {
    display: "整个屏幕",
    window: "应用窗口",
    region: "自定义区域",
    primarySuffix: "（主显示器）",
    selectDisplay: "选择显示器",
    selectWindow: "选择窗口",
    repick: "重新框选",
    pickRegion: "框选录制区域",
    picking: "请在屏幕上框选…",
  },

  summary: {
    noSource: "未选择源",
    regionMode: "自定义区域",
  },

  quality: {
    original: "原画",
    best: "最高",
    smooth: "流畅",
    smaller: "较小",
    smallest: "最小",
    cinematic: "电影",
    standard: "标准",
    buttery: "顺滑",
  },

  audio: {
    systemTitle: "系统音频",
    systemSubtitleOn: "录制电脑播放的声音",
    systemUnsupported: "当前系统不支持",
    micTitle: "麦克风",
    micSubtitleOn: "录制您的旁白",
    micSubtitleOff: "不录制麦克风",
    micDefault: "默认设备",
    noMics: "未检测到麦克风设备，请检查连接与隐私设置",
  },

  output: {
    quality: "画质",
    fps: "帧率",
    location: "保存位置",
    defaultDir: "默认视频文件夹",
    change: "更改",
    cursor: "录制鼠标光标",
  },

  record: {
    ffmpegMissing: "未检测到 ffmpeg",
    regionMissing: "请先框选录制区域",
    stopHint: "点击停止并保存",
    startHint: "开始录制",
    startAria: "开始录制",
    stopAria: "停止录制",
    hotkeyStartStop: "开始 / 停止",
    hotkeyPause: "暂停",
  },

  perm: {
    title: "需要屏幕录制权限",
    needsInstallBody:
      "ScreenCut 正在从安装盘或临时位置直接运行，macOS 不会授权给该副本。请将其拖入「应用程序」文件夹后重新打开。",
    body: "ScreenCut 使用系统原生的屏幕捕获能力来录制您的屏幕、窗口或选定区域。请在系统设置中允许屏幕录制。",
    installHint: "移动到「应用程序」后重启应用",
    grant: "授予屏幕录制权限",
    waiting: "等待授权…",
    recheck: "已授权？重新检测",
    stillBlocked: "仍未检测到权限，请检查系统设置 → 隐私与安全性 → 屏幕录制",
  },

  result: {
    noAudio: " · 无音频",
    reveal: "在文件夹中显示",
    delete: "删除",
    close: "关闭",
    previewUnavailable: "预览不可用",
  },

  ffmpeg: {
    // Split so the mono tokens can be styled inline in both languages.
    prefix: "未在系统中找到 ",
    first: "，无法完成视频编码。请安装 ",
    middle: " 并确保其在 PATH 中，或设置环境变量 ",
    suffix: "。",
  },

  region: {
    preparing: "画面准备中",
    hintActive: "拖动移动 · 拖角调整 · ←→↑↓ 微调 · 回车确认 · Esc 取消",
    hintIdle: "按住拖拽框选 · ←→↑↓ 微调 · 回车确认 · Esc 取消",
    cancel: "取消",
    confirm: "完成录制区域",
  },

  bar: {
    saving: "正在保存",
    paused: "已暂停",
    recording: "录制中",
    resume: "继续",
    pause: "暂停",
    stop: "停止并保存",
    discard: "丢弃录制",
  },

  toast: {
    selectDisplay: "请先选择一个显示器",
    regionFirst: "请先框选录制区域",
    noFfmpeg: "未找到 ffmpeg，无法编码视频",
    discarded: "已丢弃该录制",
    unknownError: "未知错误",
    unknownSize: "未知尺寸",
  },

  dropdown: {
    placeholder: "选择…",
  },
} as const;

/** The key structure is exact (from `as const`) but values are plain strings,
 *  so the English file only has to match the keys, not the Chinese literals. */
type Stringify<T> = {
  [K in keyof T]: T[K] extends string ? string : Stringify<T[K]>;
};

export type Translations = Stringify<typeof zh>;
