import React, { useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Languages,
  Lock,
  Unlock,
  Pause,
  Play,
  Settings,
  Sparkles,
  Subtitles,
  SunMoon,
  AlertCircle,
  Loader2,
} from "lucide-react";
import { api } from "../api";
import type {
  EngineStatus,
  GlassTheme,
  PipelineMetrics,
  SubtitleEntry,
  SubPrismConfig,
} from "../types";

const QUICK_LANGUAGES = ["简体中文", "English", "日本語", "한국어", "繁體中文"];
const THEME_CYCLE: GlassTheme[] = ["smoked_dark", "crystal_light", "minimal_text"];
const THEME_LABELS: Record<GlassTheme, string> = {
  smoked_dark: "影院深玻璃",
  crystal_light: "冰透浅玻璃",
  minimal_text: "极简浮字",
};

export const SubtitleOverlay: React.FC = () => {
  const [config, setConfig] = useState<SubPrismConfig | null>(null);
  const [status, setStatus] = useState<EngineStatus>({
    phase: "idle",
    message: "正在初始化 SubPrism...",
    llmReady: false,
    asrReady: false,
    audioActive: false,
  });
  const [entries, setEntries] = useState<SubtitleEntry[]>([]);
  const [metrics, setMetrics] = useState<PipelineMetrics | null>(null);
  const [audioLevel, setAudioLevel] = useState({ rms: 0, speaking: false });
  const [hovered, setHovered] = useState(false);
  const [errorBanner, setErrorBanner] = useState<string | null>(null);

  const shellRef = useRef<HTMLDivElement>(null);
  const toolbarRef = useRef<HTMLDivElement>(null);
  const hideTimerRef = useRef<number | null>(null);

  const platform = useMemo(() => {
    const ua = navigator.userAgent.toLowerCase();
    return ua.includes("mac") ? "macos" : "windows";
  }, []);

  useEffect(() => {
    let unlistenSub: (() => void) | undefined;
    let unlistenCfg: (() => void) | undefined;

    (async () => {
      try {
        const [cfg, st, m, hist] = await Promise.all([
          api.getConfig(),
          api.getEngineStatus(),
          api.getMetrics(),
          api.getSubtitleHistory(),
        ]);
        setConfig(cfg);
        setStatus(st);
        setMetrics(m);
        if (hist.length > 0) {
          setEntries(hist.slice(-6));
        }

        unlistenSub = await api.listenSubtitleEvents((ev) => {
          switch (ev.type) {
            case "upsert":
              setErrorBanner(null);
              setEntries((prev) => {
                const idx = prev.findIndex((x) => x.id === ev.entry.id);
                if (idx >= 0) {
                  const next = [...prev];
                  next[idx] = {
                    ...next[idx],
                    ...ev.entry,
                    // Preserve previous streamed translation while a fresh token arrives
                    translatedText:
                      ev.entry.translatedText || next[idx].translatedText,
                  };
                  return next;
                }
                return [...prev.slice(-5), ev.entry];
              });
              break;
            case "audio_level":
              setAudioLevel({ rms: ev.rms, speaking: ev.speaking });
              break;
            case "status_changed":
              setStatus(ev.status);
              if (ev.status.phase !== "error") {
                setErrorBanner(null);
              }
              break;
            case "metrics":
              setMetrics(ev.metrics);
              break;
            case "error":
              setErrorBanner(ev.message);
              break;
          }
        });

        unlistenCfg = await api.listenConfigUpdates((updated) => {
          setConfig(updated);
        });
      } catch (e) {
        setErrorBanner(String(e));
      }
    })();

    return () => {
      unlistenSub?.();
      unlistenCfg?.();
    };
  }, []);

  // Install roving-tabindex fallback ONLY when native HTML focusgroup is unavailable
  useEffect(() => {
    const bar = toolbarRef.current;
    if (!bar || "focusGroup" in HTMLElement.prototype) return;

    const getItems = () =>
      Array.from(bar.querySelectorAll<HTMLButtonElement>("button"));
    const items = getItems();
    items.forEach((b, i) => {
      b.tabIndex = i === 0 ? 0 : -1;
    });

    const onFocusIn = (e: FocusEvent) => {
      const target = e.target as HTMLButtonElement | null;
      const list = getItems();
      if (target && list.includes(target)) {
        list.forEach((b) => {
          b.tabIndex = b === target ? 0 : -1;
        });
      }
    };

    const onKeyDown = (e: KeyboardEvent) => {
      const list = getItems();
      const idx = list.indexOf(document.activeElement as HTMLButtonElement);
      if (idx < 0 || list.length === 0) return;

      let nextIdx: number | null = null;
      if (e.key === "ArrowRight") nextIdx = (idx + 1) % list.length;
      else if (e.key === "ArrowLeft")
        nextIdx = (idx - 1 + list.length) % list.length;
      else if (e.key === "Home") nextIdx = 0;
      else if (e.key === "End") nextIdx = list.length - 1;

      if (nextIdx !== null) {
        e.preventDefault();
        list[nextIdx].focus();
      }
    };

    bar.addEventListener("focusin", onFocusIn);
    bar.addEventListener("keydown", onKeyDown);
    return () => {
      bar.removeEventListener("focusin", onFocusIn);
      bar.removeEventListener("keydown", onKeyDown);
    };
  }, [config !== null]);

  const handlePointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    const el = shellRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const x = `${e.clientX - rect.left}px`;
    const y = `${e.clientY - rect.top}px`;
    el.style.setProperty("--mx", x);
    el.style.setProperty("--my", y);
  };

  const handleMouseEnter = () => {
    if (hideTimerRef.current) {
      window.clearTimeout(hideTimerRef.current);
      hideTimerRef.current = null;
    }
    setHovered(true);
  };

  const handleMouseLeave = () => {
    hideTimerRef.current = window.setTimeout(() => {
      setHovered(false);
    }, 950);
  };

  const handleDragStart = (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest("button, [role='toolbar']")) return;
    if (e.button === 0) {
      getCurrentWindow().startDragging().catch(() => {});
    }
  };

  const handleToggleListen = async () => {
    if (status.phase === "starting_llm" || status.phase === "loading_asr") {
      return;
    }
    try {
      setErrorBanner(null);
      await api.toggleListening();
    } catch (e) {
      setErrorBanner(String(e));
      if (String(e).includes("未配置")) {
        await api.openSettingsWindow();
      }
    }
  };

  const handleCycleTargetLang = async () => {
    if (!config) return;
    const idx = QUICK_LANGUAGES.indexOf(config.targetLanguage);
    const nextLang = QUICK_LANGUAGES[(idx + 1) % QUICK_LANGUAGES.length];
    const updated = { ...config, targetLanguage: nextLang };
    setConfig(updated);
    await api.saveConfig(updated);
  };

  const handleToggleSource = async () => {
    if (!config) return;
    const updated: SubPrismConfig = {
      ...config,
      appearance: {
        ...config.appearance,
        showSource: !config.appearance.showSource,
      },
    };
    setConfig(updated);
    await api.saveConfig(updated);
  };

  const handleCycleTheme = async () => {
    if (!config) return;
    const idx = THEME_CYCLE.indexOf(config.appearance.theme);
    const nextTheme = THEME_CYCLE[(idx + 1) % THEME_CYCLE.length];
    const updated: SubPrismConfig = {
      ...config,
      appearance: { ...config.appearance, theme: nextTheme },
    };
    setConfig(updated);
    await api.saveConfig(updated);
  };

  const handleToggleClickThrough = async () => {
    if (!config) return;
    await api.setClickThrough(!config.appearance.clickThrough);
  };

  const theme = config?.appearance.theme ?? "smoked_dark";
  const maxLines = config?.appearance.maxVisibleLines ?? 2;
  const showSource = config?.appearance.showSource ?? true;
  const sourceOnTop = config?.appearance.sourceOnTop ?? true;
  const translatedSize = config?.appearance.translatedFontSize ?? 23;
  const sourceSize = config?.appearance.sourceFontSize ?? 13;
  const clickThrough = config?.appearance.clickThrough ?? false;
  const glassOpacity = config?.appearance.glassOpacity ?? 0.46;

  const isListening = status.phase === "listening";
  const isBusy =
    status.phase === "starting_llm" || status.phase === "loading_asr";
  const showToolbar = hovered || !isListening || status.phase === "needs_setup";

  const visibleEntries = entries.slice(-maxLines);

  const translatedColorClass =
    theme === "crystal_light"
      ? "subtitle-translated-light"
      : theme === "minimal_text"
      ? "subtitle-translated-minimal"
      : "subtitle-translated-dark";

  const sourceColorClass =
    theme === "crystal_light"
      ? "subtitle-source-light"
      : "subtitle-source-dark";

  // Reactive 4-bar fluid audio level visualization
  const barHeights = useMemo(() => {
    const level = Math.min(1, audioLevel.rms * 7.5);
    if (!isListening || !audioLevel.speaking) {
      return [3, 3, 3, 3];
    }
    return [
      Math.max(4, Math.round(5 + level * 11)),
      Math.max(5, Math.round(7 + level * 18)),
      Math.max(5, Math.round(6 + level * 15)),
      Math.max(4, Math.round(4 + level * 10)),
    ];
  }, [audioLevel, isListening]);

  return (
    <div
      ref={shellRef}
      className="overlay-shell px-6 py-3"
      data-theme={theme}
      data-platform={platform}
      style={{ "--glass-opacity": String(glassOpacity) } as React.CSSProperties}
      onPointerMove={handlePointerMove}
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
      onMouseDown={handleDragStart}
    >
      {/* Top optical specular refraction filament */}
      <div className="liquid-specular-rim" aria-hidden="true" />

      {/* Left delicate reactive voice wave indicator */}
      <div
        className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 flex items-center gap-[2.5px] transition-opacity duration-300"
        style={{ opacity: isListening && audioLevel.speaking ? 0.92 : 0.16 }}
        aria-hidden="true"
      >
        {barHeights.map((h, i) => (
          <span
            key={i}
            className="w-[2.5px] rounded-full bg-gradient-to-t from-sky-400 to-teal-200 transition-[height] duration-100"
            style={{ height: `${h}px` }}
          />
        ))}
      </div>

      {/* Hover-revealed Liquid Glass Floating Toolbar */}
      <div
        ref={toolbarRef}
        className="floating-toolbar"
        role="toolbar"
        aria-label="字幕控制工具栏"
        aria-controls="subtitle-stream"
        hidden={!showToolbar}
        {...({ focusgroup: "toolbar wrap" } as Record<string, string>)}
      >
        {/* Left status & telemetry capsule (draggable area) */}
        <div
          className="toolbar-pill text-[11px] font-medium tracking-wide cursor-grab active:cursor-grabbing"
          data-tauri-drag-region
        >
          {isBusy ? (
            <Loader2 className="w-3 h-3 animate-spin text-sky-300" aria-hidden="true" />
          ) : (
            <span
              className={`w-2 h-2 rounded-full ${
                isListening
                  ? "status-dot-live"
                  : status.phase === "error"
                  ? "bg-rose-400"
                  : status.phase === "needs_setup"
                  ? "bg-amber-400"
                  : "bg-slate-400/75"
              }`}
              aria-hidden="true"
            />
          )}
          <span className="px-1 opacity-90">
            {isListening
              ? "实时翻译中"
              : isBusy
              ? status.phase === "starting_llm"
                ? "加载 Spark-X2.5-4B"
                : "加载语音模型"
              : status.phase === "paused"
              ? "已暂停"
              : status.phase === "needs_setup"
              ? "待配置模型"
              : "SubPrism"}
          </span>
          {metrics && metrics.lastTranslateMs > 0 && (
            <span className="opacity-60 font-mono text-[10px] pl-0.5 pr-1">
              {metrics.lastTtftMs}ms
              {metrics.tokensPerSec > 0
                ? ` · ${Math.round(metrics.tokensPerSec)}t/s`
                : ""}
            </span>
          )}
        </div>

        {/* Right control capsule */}
        <div className="toolbar-pill">
          <div role="group" aria-label="监听控制" className="inline-flex items-center">
            <button
              type="button"
              className="toolbar-btn aria-disabled:opacity-50 aria-disabled:cursor-wait"
              onClick={handleToggleListen}
              aria-disabled={isBusy ? "true" : undefined}
              aria-label={isListening ? "暂停监听" : "开始监听"}
              title={
                isListening
                  ? "暂停监听 (Alt+Shift+S)"
                  : "开始监听内播音频 (Alt+Shift+S)"
              }
            >
              {isListening ? (
                <Pause className="w-3.5 h-3.5 text-teal-300" aria-hidden="true" />
              ) : (
                <Play className="w-3.5 h-3.5 text-sky-300" aria-hidden="true" />
              )}
              <span>{isListening ? "暂停" : "监听"}</span>
            </button>

            <button
              type="button"
              className="toolbar-btn"
              onClick={handleCycleTargetLang}
              aria-label="切换目标翻译语言"
              title="点击切换目标语言"
            >
              <Languages className="w-3.5 h-3.5 opacity-85" aria-hidden="true" />
              <span>→ {config?.targetLanguage ?? "简体中文"}</span>
            </button>
          </div>

          <span role="separator" aria-orientation="vertical" className="toolbar-sep" />

          <div role="group" aria-label="显示选项" className="inline-flex items-center">
            <button
              type="button"
              className="toolbar-btn"
              aria-pressed={showSource}
              onClick={handleToggleSource}
              aria-label="显示原文"
              title="切换双语原文显示"
            >
              <Subtitles className="w-3.5 h-3.5" aria-hidden="true" />
              <span>双语</span>
            </button>

            <button
              type="button"
              className="toolbar-btn"
              onClick={handleCycleTheme}
              aria-label="切换液态玻璃主题"
              title={`当前材质：${THEME_LABELS[theme]}（点击切换）`}
            >
              <SunMoon className="w-3.5 h-3.5 opacity-85" aria-hidden="true" />
            </button>

            <button
              type="button"
              className="toolbar-btn"
              aria-pressed={clickThrough}
              onClick={handleToggleClickThrough}
              aria-label="鼠标穿透锁"
              title="鼠标穿透锁 (Alt+Shift+L 随时解锁)"
            >
              {clickThrough ? (
                <Lock className="w-3.5 h-3.5 text-amber-300" aria-hidden="true" />
              ) : (
                <Unlock className="w-3.5 h-3.5 opacity-85" aria-hidden="true" />
              )}
            </button>

            <button
              type="button"
              className="toolbar-btn"
              onClick={() => api.openSettingsWindow()}
              aria-label="打开设置与模型管理"
              title="设置与本地 AI 模型"
            >
              <Settings className="w-3.5 h-3.5 opacity-90" aria-hidden="true" />
            </button>
          </div>
        </div>
      </div>

      {/* Main Long-Strip Subtitle Viewport */}
      <main
        id="subtitle-stream"
        aria-live="polite"
        aria-atomic="false"
        className="w-full flex flex-col items-center justify-center text-center gap-1.5 pt-4 px-5"
      >
        {errorBanner ? (
          <div className="subtitle-block flex items-center gap-2 text-rose-300 text-sm font-medium max-w-full">
            <AlertCircle className="w-4 h-4 shrink-0" aria-hidden="true" />
            <span className="truncate">{errorBanner}</span>
            <button
              type="button"
              onClick={() => api.openSettingsWindow()}
              className="ml-1 px-2.5 py-0.5 rounded-full text-xs bg-white/15 hover:bg-white/25 text-white cursor-pointer"
            >
              检查配置
            </button>
          </div>
        ) : visibleEntries.length === 0 ? (
          <div className="subtitle-block flex flex-col items-center gap-1">
            <div
              className={`flex items-center gap-2 font-medium tracking-wide ${translatedColorClass}`}
              style={{ fontSize: `${Math.min(translatedSize, 19)}px` }}
            >
              <Sparkles className="w-4 h-4 text-sky-300 shrink-0" aria-hidden="true" />
              <span>{status.message}</span>
            </div>
            <p className={`text-xs ${sourceColorClass}`}>
              {status.phase === "needs_setup"
                ? "点击右上角 ⚙ 指定本地 llama-server 与 Spark-X2.5-4B 模型路径"
                : "拖拽玻璃条可调整位置 · 快捷键 Alt+Shift+S 启停 · Alt+Shift+L 鼠标穿透"}
            </p>
          </div>
        ) : (
          visibleEntries.map((entry, idx) => {
            const isNewest = idx === visibleEntries.length - 1;
            const isPartial = entry.status === "partial";
            const isStreaming = entry.status === "translating";
            const displayTranslated =
              entry.translatedText || (isPartial || isStreaming ? "…" : "");

            const sourceEl = showSource && entry.sourceText && (
              <div
                className={`leading-snug tracking-normal max-w-[94%] truncate ${sourceColorClass}`}
                style={{
                  fontSize: `${sourceSize}px`,
                  opacity: isNewest ? 0.78 : 0.48,
                }}
              >
                {entry.sourceText}
              </div>
            );

            const translatedEl = (
              <div
                className={`leading-snug font-semibold tracking-[0.01em] max-w-[96%] ${translatedColorClass}`}
                style={{
                  fontSize: `${isNewest ? translatedSize : Math.max(15, translatedSize - 4)}px`,
                  opacity: isNewest ? (isPartial ? 0.84 : 1) : 0.56,
                }}
              >
                {displayTranslated}
                {isStreaming && (
                  <span
                    className="inline-block w-[2px] h-[0.88em] ml-1 align-middle rounded-full bg-sky-300/90"
                    aria-hidden="true"
                  />
                )}
              </div>
            );

            return (
              <div
                key={entry.id}
                className="subtitle-block w-full flex flex-col items-center gap-0.5"
              >
                {sourceOnTop ? (
                  <>
                    {sourceEl}
                    {translatedEl}
                  </>
                ) : (
                  <>
                    {translatedEl}
                    {sourceEl}
                  </>
                )}
              </div>
            );
          })
        )}
      </main>
    </div>
  );
};
