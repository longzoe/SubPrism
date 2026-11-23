import React, { useEffect, useState } from "react";
import {
  CheckCircle2,
  Cpu,
  Download,
  FolderOpen,
  History,
  Languages,
  Layers,
  Loader2,
  Plus,
  RefreshCw,
  Send,
  Sliders,
  Sparkles,
  Trash2,
  AlertTriangle,
  Crosshair,
} from "lucide-react";
import { api } from "../api";
import type {
  AudioSourceInfo,
  EngineStatus,
  GlassTheme,
  HardwareInfo,
  PipelineMetrics,
  StylePreset,
  SubtitleEntry,
  SubPrismConfig,
} from "../types";

type TabId = "models" | "translation" | "appearance" | "history";

const TARGET_LANGS = [
  "简体中文",
  "繁體中文",
  "English",
  "日本語",
  "한국어",
  "Français",
  "Deutsch",
  "Español",
  "Русский",
];

const ASR_LANGS = [
  { value: "auto", label: "自动检测 (中/英/日/韩/粤)" },
  { value: "en", label: "English (英语)" },
  { value: "ja", label: "日本語 (日语)" },
  { value: "ko", label: "한국어 (韩语)" },
  { value: "zh", label: "中文 (普通话)" },
  { value: "yue", label: "粤语 (Cantonese)" },
];

const STYLE_PRESETS: { id: StylePreset; title: string; desc: string }[] = [
  { id: "general", title: "日常通用视频", desc: "流畅自然、去口头禅，适合 YouTube/B站 泛内容" },
  { id: "tech", title: "科技 / 教程演讲", desc: "严谨专业，保留 API、Rust、CUDA 等技术词" },
  { id: "cinema", title: "影视 / 纪录片", desc: "贴合台词情绪与画面感，符合字幕组习惯" },
  { id: "anime", title: "动漫 / 游戏实况", desc: "贴合 ACG 圈层译法，保留角色口吻" },
];

export const SettingsPanel: React.FC = () => {
  const [tab, setTab] = useState<TabId>("models");
  const [config, setConfig] = useState<SubPrismConfig | null>(null);
  const [hardware, setHardware] = useState<HardwareInfo | null>(null);
  const [audioSources, setAudioSources] = useState<AudioSourceInfo[]>([]);
  const [status, setStatus] = useState<EngineStatus | null>(null);
  const [metrics, setMetrics] = useState<PipelineMetrics | null>(null);
  const [history, setHistory] = useState<SubtitleEntry[]>([]);
  const [promptPreview, setPromptPreview] = useState<string>("");
  const [missingPaths, setMissingPaths] = useState<string[]>([]);

  const [newGlossSrc, setNewGlossSrc] = useState("");
  const [newGlossDst, setNewGlossDst] = useState("");
  const [testInput, setTestInput] = useState(
    "So today we are running Spark-X2.5-4B completely locally with llama.cpp for ultra-low-latency subtitles."
  );
  const [testResult, setTestResult] = useState<SubtitleEntry | null>(null);
  const [testing, setTesting] = useState(false);
  const [toast, setToast] = useState<string | null>(null);

  const showToast = (msg: string) => {
    setToast(msg);
    window.setTimeout(() => setToast(null), 3200);
  };

  const refreshAll = async () => {
    const [cfg, hw, srcs, st, m, hist, val, prompt] = await Promise.all([
      api.getConfig(),
      api.getHardwareInfo(),
      api.listAudioSources(),
      api.getEngineStatus(),
      api.getMetrics(),
      api.getSubtitleHistory(),
      api.validatePaths(),
      api.previewSystemPrompt(),
    ]);
    setConfig(cfg);
    setHardware(hw);
    setAudioSources(srcs);
    setStatus(st);
    setMetrics(m);
    setHistory(hist);
    setMissingPaths(val.missing);
    setPromptPreview(prompt);
  };

  useEffect(() => {
    let unlistenSub: (() => void) | undefined;
    let unlistenCfg: (() => void) | undefined;

    refreshAll().catch(console.error);

    (async () => {
      unlistenSub = await api.listenSubtitleEvents((ev) => {
        if (ev.type === "status_changed") setStatus(ev.status);
        if (ev.type === "metrics") setMetrics(ev.metrics);
        if (ev.type === "upsert" && ev.entry.status === "final") {
          setHistory((prev) => [...prev.slice(-199), ev.entry]);
        }
      });
      unlistenCfg = await api.listenConfigUpdates((updated) => {
        setConfig(updated);
      });
    })();

    return () => {
      unlistenSub?.();
      unlistenCfg?.();
    };
  }, []);

  const persist = async (next: SubPrismConfig, msg?: string) => {
    setConfig(next);
    try {
      await api.saveConfig(next);
      const [val, prompt] = await Promise.all([
        api.validatePaths(),
        api.previewSystemPrompt(),
      ]);
      setMissingPaths(val.missing);
      setPromptPreview(prompt);
      if (msg) showToast(msg);
    } catch (e) {
      showToast(`保存失败: ${String(e)}`);
    }
  };

  const handleBrowsePath = async (
    field: keyof SubPrismConfig["paths"],
    kind: "file" | "dir",
    title: string,
    filters?: { name: string; extensions: string[] }[]
  ) => {
    if (!config) return;
    const picked =
      kind === "dir"
        ? await api.pickDirectory(title)
        : await api.pickFile(title, filters);
    if (!picked) return;
    await persist(
      { ...config, paths: { ...config.paths, [field]: picked } },
      "路径已更新并保存"
    );
  };

  const handleRunTest = async () => {
    if (!testInput.trim()) return;
    setTesting(true);
    try {
      const entry = await api.testTranslate(testInput.trim());
      setTestResult(entry);
      showToast(
        `Spark-X2.5-4B 测试成功！TTFT ${entry.ttftMs ?? 0}ms · 总耗时 ${
          entry.translateLatencyMs ?? 0
        }ms`
      );
    } catch (e) {
      showToast(`测试失败: ${String(e)}`);
    } finally {
      setTesting(false);
    }
  };

  const handleAddGlossary = async () => {
    if (!config || !newGlossSrc.trim() || !newGlossDst.trim()) return;
    const next: SubPrismConfig = {
      ...config,
      glossary: [
        ...config.glossary,
        { source: newGlossSrc.trim(), target: newGlossDst.trim() },
      ],
    };
    setNewGlossSrc("");
    setNewGlossDst("");
    await persist(next, "已添加术语规则");
  };

  const handleRemoveGlossary = async (idx: number) => {
    if (!config) return;
    const next: SubPrismConfig = {
      ...config,
      glossary: config.glossary.filter((_, i) => i !== idx),
    };
    await persist(next);
  };

  const handleExportSrt = async (includeSource: boolean) => {
    try {
      const path = await api.pickSaveSrt();
      if (!path) return;
      const count = await api.exportSrt(path, includeSource);
      showToast(`已成功导出 ${count} 条字幕到 ${path}`);
    } catch (e) {
      showToast(`导出失败: ${String(e)}`);
    }
  };

  if (!config) {
    return (
      <div className="settings-root flex items-center justify-center text-slate-300 text-sm">
        <Loader2 className="w-4 h-4 animate-spin mr-2" /> 正在加载配置...
      </div>
    );
  }

  return (
    <div className="settings-root flex flex-col">
      {/* Header Bar */}
      <header className="px-6 pt-5 pb-4 border-b border-white/10 flex items-center justify-between">
        <div>
          <div className="flex items-center gap-2">
            <Sparkles className="w-5 h-5 text-sky-400" aria-hidden="true" />
            <h1 className="text-lg font-semibold tracking-tight text-white">
              SubPrism 控制台
            </h1>
            <span className="px-2 py-0.5 rounded-full text-[11px] bg-sky-500/20 text-sky-300 border border-sky-400/30">
              Spark-X2.5-4B · Local AI
            </span>
          </div>
          <p className="text-xs text-slate-400 mt-0.5">
            纯本地离线实时视频字幕监听与翻译引擎 · 支持 Windows / macOS
          </p>
        </div>

        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => api.toggleListening().catch((e) => showToast(String(e)))}
            className={`px-4 py-1.5 rounded-full text-xs font-medium cursor-pointer transition ${
              status?.phase === "listening"
                ? "bg-emerald-500/25 text-emerald-200 border border-emerald-400/45 hover:bg-emerald-500/35"
                : "bg-sky-500 text-slate-950 font-semibold hover:bg-sky-400"
            }`}
          >
            {status?.phase === "listening" ? "● 正在监听（点击暂停）" : "▶ 开始实时监听"}
          </button>
        </div>
      </header>

      {/* Main Split Navigation + Content */}
      <div className="flex-1 flex min-h-0">
        {/* Sidebar Tabs */}
        <nav
          aria-label="设置分类"
          className="w-48 p-3 border-r border-white/10 flex flex-col gap-1 shrink-0"
        >
          {(
            [
              { id: "models", label: "本地模型与引擎", icon: Cpu },
              { id: "translation", label: "音频与翻译提示词", icon: Languages },
              { id: "appearance", label: "液态玻璃外观", icon: Layers },
              { id: "history", label: "字幕历史与导出", icon: History },
            ] as const
          ).map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              type="button"
              onClick={() => setTab(id)}
              className={`flex items-center gap-2.5 px-3 py-2.5 rounded-xl text-xs font-medium text-left cursor-pointer transition ${
                tab === id
                  ? "bg-white/12 text-white border border-white/15 shadow-sm"
                  : "text-slate-400 hover:text-slate-200 hover:bg-white/5 border border-transparent"
              }`}
            >
              <Icon className="w-4 h-4 shrink-0" aria-hidden="true" />
              <span>{label}</span>
              {id === "models" && missingPaths.length > 0 && (
                <span className="ml-auto w-2 h-2 rounded-full bg-amber-400" />
              )}
            </button>
          ))}

          {/* Hardware Telemetry Footer */}
          {hardware && (
            <div className="mt-auto glass-card p-3 text-[11px] space-y-1">
              <div className="text-slate-400 font-medium flex items-center justify-between">
                <span>探测硬件</span>
                <span className="uppercase text-sky-300 font-mono">
                  {hardware.backend}
                </span>
              </div>
              <div className="text-slate-200 truncate" title={hardware.gpuName ?? "CPU"}>
                {hardware.gpuName ?? "CPU 推理模式"}
              </div>
              {hardware.vramMb && (
                <div className="text-slate-400">
                  显存 {(hardware.vramMb / 1024).toFixed(1)} GB
                  {hardware.cudaVersion ? ` · CUDA ${hardware.cudaVersion}` : ""}
                </div>
              )}
              {metrics && metrics.tokensPerSec > 0 && (
                <div className="pt-1 border-t border-white/10 text-teal-300 font-mono">
                  {Math.round(metrics.tokensPerSec)} tok/s · TTFT {metrics.lastTtftMs}ms
                </div>
              )}
            </div>
          )}
        </nav>

        {/* Tab Body */}
        <section className="flex-1 p-5 overflow-y-auto glass-scroll space-y-4">
          {tab === "models" && (
            <>
              <div className="glass-card p-4 space-y-3">
                <div className="flex items-center justify-between">
                  <div>
                    <h2 className="text-sm font-semibold text-white">
                      本地 AI 引擎与模型路径（用户自行下载指定）
                    </h2>
                    <p className="text-xs text-slate-400 mt-0.5">
                      正式版本不内置大模型体积文件；请下载对应平台的运行库与模型后在此指定，软件会自动托管拉起 `llama-server`。
                    </p>
                  </div>
                  {missingPaths.length === 0 ? (
                    <span className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-xs bg-emerald-500/20 text-emerald-300 border border-emerald-400/30 shrink-0">
                      <CheckCircle2 className="w-3.5 h-3.5" /> 全部就绪
                    </span>
                  ) : (
                    <span className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-xs bg-amber-500/20 text-amber-300 border border-amber-400/30 shrink-0">
                      <AlertTriangle className="w-3.5 h-3.5" /> 缺 {missingPaths.length} 项
                    </span>
                  )}
                </div>

                <PathRow
                  label="1. llama-server 可执行文件（需 llama.cpp >= b10833 支持 spark2_5）"
                  hint="Win 下载 llama-*-bin-win-cuda-*-x64.zip + cudart；Mac 下载 macos-arm64 版"
                  value={config.paths.llamaServerBin}
                  onChange={(v) =>
                    persist({ ...config, paths: { ...config.paths, llamaServerBin: v } })
                  }
                  onBrowse={() =>
                    handleBrowsePath("llamaServerBin", "file", "选择 llama-server 可执行文件")
                  }
                />

                <PathRow
                  label="2. Spark-X2.5-4B GGUF 模型权重（推荐 Q4_K_M / Q8_0）"
                  hint="来源：ModelScope XHToken/Spark-X2.5-4B-GGUF 或 HuggingFace"
                  value={config.paths.llmModelGguf}
                  onChange={(v) =>
                    persist({ ...config, paths: { ...config.paths, llmModelGguf: v } })
                  }
                  onBrowse={() =>
                    handleBrowsePath("llmModelGguf", "file", "选择 Spark-X2.5-4B GGUF 模型", [
                      { name: "GGUF 模型", extensions: ["gguf"] },
                    ])
                  }
                />

                <PathRow
                  label="3. sherpa-onnx C-API 动态库（sherpa-onnx-c-api.dll / .dylib）"
                  hint="来源：GitHub k2-fsa/sherpa-onnx Releases (v2.0.3 shared lib)"
                  value={config.paths.sherpaLibPath}
                  onChange={(v) =>
                    persist({ ...config, paths: { ...config.paths, sherpaLibPath: v } })
                  }
                  onBrowse={() =>
                    handleBrowsePath("sherpaLibPath", "file", "选择 sherpa-onnx C-API 动态库", [
                      { name: "动态库", extensions: ["dll", "dylib", "so"] },
                    ])
                  }
                />

                <PathRow
                  label="4. SenseVoice-Small 本地语音识别模型目录（含 model.int8.onnx 与 tokens.txt）"
                  hint="纯 CPU 推理极快，不占用 Spark-X2.5-4B 显存，支持中/英/日/韩/粤"
                  value={config.paths.asrModelDir}
                  onChange={(v) =>
                    persist({ ...config, paths: { ...config.paths, asrModelDir: v } })
                  }
                  onBrowse={() =>
                    handleBrowsePath("asrModelDir", "dir", "选择 SenseVoice 模型文件夹")
                  }
                />

                <PathRow
                  label="5. Silero VAD v5 语音端点检测模型（silero_vad_v5.onnx）"
                  hint="过滤背景音乐与静音、自动精准断句，防止 ASR 幻觉"
                  value={config.paths.vadModelPath}
                  onChange={(v) =>
                    persist({ ...config, paths: { ...config.paths, vadModelPath: v } })
                  }
                  onBrowse={() =>
                    handleBrowsePath("vadModelPath", "file", "选择 Silero VAD ONNX 模型", [
                      { name: "ONNX 模型", extensions: ["onnx"] },
                    ])
                  }
                />
              </div>

              {/* Live Spark-X2.5-4B Test Bench */}
              <div className="glass-card p-4 space-y-3">
                <div className="flex items-center justify-between">
                  <div>
                    <h3 className="text-sm font-semibold text-white">
                      Spark-X2.5-4B 实机翻译联调测试
                    </h3>
                    <p className="text-xs text-slate-400">
                      自动拉起本地 llama-server（强制 Non-Thinking 低延迟模式）并将译文同步推送到液态玻璃字幕条。
                    </p>
                  </div>
                  {status?.llamaVersion && (
                    <span className="text-[11px] font-mono text-slate-400">
                      {status.llamaVersion}
                    </span>
                  )}
                </div>

                <div className="flex gap-2">
                  <input
                    type="text"
                    className="glass-input flex-1"
                    value={testInput}
                    onChange={(e) => setTestInput(e.target.value)}
                    placeholder="输入一句视频台词测试 Spark-X2.5-4B 实时翻译..."
                  />
                  <button
                    type="button"
                    disabled={testing}
                    onClick={handleRunTest}
                    className="px-4 py-2 rounded-xl bg-sky-500 hover:bg-sky-400 disabled:opacity-50 text-slate-950 font-semibold text-xs inline-flex items-center gap-1.5 cursor-pointer shrink-0"
                  >
                    {testing ? (
                      <Loader2 className="w-3.5 h-3.5 animate-spin" />
                    ) : (
                      <Send className="w-3.5 h-3.5" />
                    )}
                    <span>测试翻译</span>
                  </button>
                </div>

                {testResult && (
                  <div className="rounded-xl bg-slate-950/60 border border-sky-400/25 p-3 space-y-1">
                    <div className="text-sm text-white font-medium">
                      {testResult.translatedText}
                    </div>
                    <div className="text-[11px] text-slate-400 font-mono">
                      TTFT: {testResult.ttftMs ?? 0}ms · 整句耗时:{" "}
                      {testResult.translateLatencyMs ?? 0}ms
                    </div>
                  </div>
                )}
              </div>
            </>
          )}

          {tab === "translation" && (
            <>
              <div className="glass-card p-4 space-y-4">
                <div className="flex items-center justify-between">
                  <h2 className="text-sm font-semibold text-white">音频源与语言设置</h2>
                  <button
                    type="button"
                    onClick={async () => {
                      setAudioSources(await api.listAudioSources());
                      showToast("已刷新音频设备列表");
                    }}
                    className="text-xs text-sky-300 hover:text-sky-200 inline-flex items-center gap-1 cursor-pointer"
                  >
                    <RefreshCw className="w-3.5 h-3.5" /> 刷新设备
                  </button>
                </div>

                <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                  <label className="flex flex-col gap-1.5 text-xs">
                    <span className="text-slate-300">监听音频源（零虚拟声卡内录）</span>
                    <select
                      className="glass-input"
                      value={config.audioSourceId}
                      onChange={(e) =>
                        persist({ ...config, audioSourceId: e.target.value })
                      }
                    >
                      {audioSources.map((s) => (
                        <option key={s.id} value={s.id} className="bg-slate-900">
                          {s.name}
                        </option>
                      ))}
                    </select>
                  </label>

                  <label className="flex flex-col gap-1.5 text-xs">
                    <span className="text-slate-300">目标翻译语言</span>
                    <select
                      className="glass-input"
                      value={config.targetLanguage}
                      onChange={(e) =>
                        persist({ ...config, targetLanguage: e.target.value })
                      }
                    >
                      {TARGET_LANGS.map((l) => (
                        <option key={l} value={l} className="bg-slate-900">
                          {l}
                        </option>
                      ))}
                    </select>
                  </label>

                  <label className="flex flex-col gap-1.5 text-xs">
                    <span className="text-slate-300">视频源语音语言（ASR 提示）</span>
                    <select
                      className="glass-input"
                      value={config.asr.sourceLanguage}
                      onChange={(e) =>
                        persist({
                          ...config,
                          asr: { ...config.asr, sourceLanguage: e.target.value },
                        })
                      }
                    >
                      {ASR_LANGS.map((l) => (
                        <option key={l.value} value={l.value} className="bg-slate-900">
                          {l.label}
                        </option>
                      ))}
                    </select>
                    <span className="text-[11px] text-slate-400">
                      看日剧/番剧推荐直接选「日本語 (ja)」，轻声耳语识别率显著高于自动检测
                    </span>
                  </label>

                  <div className="flex flex-col gap-2">
                    <label className="flex items-center justify-between gap-3 rounded-xl bg-white/5 px-3.5 py-2 text-xs cursor-pointer">
                      <div>
                        <div className="text-slate-200 font-medium">轻声台词自动增益（AGC）</div>
                        <div className="text-slate-400 text-[11px]">
                          自动抬升剧中人物轻声/耳语，不放大纯静音
                        </div>
                      </div>
                      <input
                        type="checkbox"
                        checked={config.asr.softSpeechBoost ?? true}
                        onChange={(e) =>
                          persist({
                            ...config,
                            asr: { ...config.asr, softSpeechBoost: e.target.checked },
                          })
                        }
                      />
                    </label>

                    <label className="flex items-center justify-between gap-3 rounded-xl bg-white/5 px-3.5 py-2 text-xs cursor-pointer">
                      <div>
                        <div className="text-slate-200 font-medium">说话中实时草稿翻译</div>
                        <div className="text-slate-400 text-[11px]">
                          未停顿前每 ~500ms 预览译文，延迟更低
                        </div>
                      </div>
                      <input
                        type="checkbox"
                        checked={config.llm.translatePartials}
                        onChange={(e) =>
                          persist({
                            ...config,
                            llm: { ...config.llm, translatePartials: e.target.checked },
                          })
                        }
                      />
                    </label>
                  </div>
                </div>

                {/* Style presets */}
                <div className="space-y-2">
                  <span className="text-xs text-slate-300">视频翻译风格预设</span>
                  <div className="grid grid-cols-2 gap-2.5">
                    {STYLE_PRESETS.map((p) => (
                      <button
                        key={p.id}
                        type="button"
                        onClick={() => persist({ ...config, stylePreset: p.id })}
                        className={`p-3 rounded-xl border text-left cursor-pointer transition ${
                          config.stylePreset === p.id
                            ? "bg-sky-500/18 border-sky-400/55 text-white"
                            : "bg-white/4 border-white/10 text-slate-300 hover:bg-white/8"
                        }`}
                      >
                        <div className="text-xs font-semibold">{p.title}</div>
                        <div className="text-[11px] text-slate-400 mt-0.5">{p.desc}</div>
                      </button>
                    ))}
                  </div>
                </div>
              </div>

              {/* Glossary */}
              <div className="glass-card p-4 space-y-3">
                <h3 className="text-sm font-semibold text-white">
                  自定义专有名词术语表（自动注入提示词）
                </h3>
                <div className="flex gap-2">
                  <input
                    type="text"
                    className="glass-input flex-1"
                    placeholder="原文术语（如 Spark-X2.5）"
                    value={newGlossSrc}
                    onChange={(e) => setNewGlossSrc(e.target.value)}
                  />
                  <input
                    type="text"
                    className="glass-input flex-1"
                    placeholder="指定译名（如 星火 X2.5）"
                    value={newGlossDst}
                    onChange={(e) => setNewGlossDst(e.target.value)}
                  />
                  <button
                    type="button"
                    onClick={handleAddGlossary}
                    className="px-3.5 py-2 rounded-xl bg-white/12 hover:bg-white/20 text-white text-xs inline-flex items-center gap-1 cursor-pointer"
                  >
                    <Plus className="w-3.5 h-3.5" /> 添加
                  </button>
                </div>
                {config.glossary.length > 0 && (
                  <div className="flex flex-wrap gap-2">
                    {config.glossary.map((g, idx) => (
                      <span
                        key={`${g.source}-${idx}`}
                        className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs bg-white/8 border border-white/15 text-slate-200"
                      >
                        <span>
                          {g.source} → {g.target}
                        </span>
                        <button
                          type="button"
                          aria-label={`删除术语 ${g.source}`}
                          onClick={() => handleRemoveGlossary(idx)}
                          className="text-slate-400 hover:text-rose-300 cursor-pointer"
                        >
                          ×
                        </button>
                      </span>
                    ))}
                  </div>
                )}
              </div>

              {/* Tuning & Live System Prompt Preview */}
              <div className="glass-card p-4 space-y-3">
                <div className="flex items-center gap-2 text-sm font-semibold text-white">
                  <Sliders className="w-4 h-4 text-sky-400" />
                  <span>推理与切句参数 + 内置 Spark-X2.5-4B 提示词预览</span>
                </div>
                <div className="grid grid-cols-2 md:grid-cols-3 gap-3 text-xs">
                  <label className="space-y-1">
                    <span className="text-slate-300">
                      VAD 灵敏度阈值: {config.asr.vadThreshold.toFixed(2)}（越低越抓轻声）
                    </span>
                    <input
                      type="range"
                      min={0.1}
                      max={0.7}
                      step={0.02}
                      className="w-full"
                      value={config.asr.vadThreshold}
                      onChange={(e) =>
                        persist({
                          ...config,
                          asr: { ...config.asr, vadThreshold: Number(e.target.value) },
                        })
                      }
                    />
                  </label>
                  <label className="space-y-1">
                    <span className="text-slate-300">
                      轻声自动增益上限: +{Math.round(config.asr.agcMaxGainDb ?? 18)} dB
                    </span>
                    <input
                      type="range"
                      min={0}
                      max={24}
                      step={2}
                      className="w-full"
                      value={config.asr.agcMaxGainDb ?? 18}
                      onChange={(e) =>
                        persist({
                          ...config,
                          asr: { ...config.asr, agcMaxGainDb: Number(e.target.value) },
                        })
                      }
                    />
                  </label>
                  <label className="space-y-1">
                    <span className="text-slate-400">
                      停顿切句阈值: {config.asr.minSilenceMs}ms
                    </span>
                    <input
                      type="range"
                      min={250}
                      max={1200}
                      step={30}
                      className="w-full"
                      value={config.asr.minSilenceMs}
                      onChange={(e) =>
                        persist({
                          ...config,
                          asr: { ...config.asr, minSilenceMs: Number(e.target.value) },
                        })
                      }
                    />
                  </label>
                  <label className="space-y-1">
                    <span className="text-slate-400">
                      GPU 卸载层数: {config.llm.gpuLayers}
                    </span>
                    <input
                      type="range"
                      min={0}
                      max={99}
                      className="w-full"
                      value={config.llm.gpuLayers}
                      onChange={(e) =>
                        persist({
                          ...config,
                          llm: { ...config.llm, gpuLayers: Number(e.target.value) },
                        })
                      }
                    />
                  </label>
                  <label className="space-y-1">
                    <span className="text-slate-400">
                      上下文记忆句数: {config.llm.contextTurns} 句
                    </span>
                    <input
                      type="range"
                      min={0}
                      max={6}
                      className="w-full"
                      value={config.llm.contextTurns}
                      onChange={(e) =>
                        persist({
                          ...config,
                          llm: { ...config.llm, contextTurns: Number(e.target.value) },
                        })
                      }
                    />
                  </label>
                </div>

                <pre className="p-3 rounded-xl bg-slate-950/70 border border-white/10 text-[11.5px] leading-relaxed text-slate-300 whitespace-pre-wrap font-sans">
                  {promptPreview}
                </pre>
              </div>
            </>
          )}

          {tab === "appearance" && (
            <div className="glass-card p-4 space-y-4">
              <div className="flex items-center justify-between">
                <h2 className="text-sm font-semibold text-white">
                  液态玻璃长条字幕样式
                </h2>
                <button
                  type="button"
                  onClick={() => api.recenterOverlay()}
                  className="px-3 py-1.5 rounded-xl bg-white/10 hover:bg-white/18 text-xs text-slate-200 inline-flex items-center gap-1.5 cursor-pointer"
                >
                  <Crosshair className="w-3.5 h-3.5" /> 恢复底部居中
                </button>
              </div>

              <div className="grid grid-cols-3 gap-3">
                {(
                  [
                    {
                      id: "smoked_dark",
                      name: "影院深玻璃",
                      desc: "高对比烟熏液态玻璃，任何视频画面都锐利清晰",
                    },
                    {
                      id: "crystal_light",
                      name: "冰透浅玻璃",
                      desc: "Apple 冰霜通透折射质感，适合暗色调影片",
                    },
                    {
                      id: "minimal_text",
                      name: "极简浮字",
                      desc: "完全隐藏底板，仅保留柔光描边悬浮字幕",
                    },
                  ] as { id: GlassTheme; name: string; desc: string }[]
                ).map((t) => (
                  <button
                    key={t.id}
                    type="button"
                    onClick={() =>
                      persist({
                        ...config,
                        appearance: { ...config.appearance, theme: t.id },
                      })
                    }
                    className={`p-3 rounded-xl border text-left cursor-pointer transition ${
                      config.appearance.theme === t.id
                        ? "bg-sky-500/20 border-sky-400/55 text-white"
                        : "bg-white/4 border-white/10 text-slate-300 hover:bg-white/8"
                    }`}
                  >
                    <div className="text-xs font-semibold">{t.name}</div>
                    <div className="text-[11px] text-slate-400 mt-1">{t.desc}</div>
                  </button>
                ))}
              </div>

              <div className="grid grid-cols-2 gap-4 text-xs">
                <label className="space-y-1.5">
                  <span className="text-slate-300">
                    玻璃底色不透明度: {Math.round(config.appearance.glassOpacity * 100)}%
                  </span>
                  <input
                    type="range"
                    min={0.12}
                    max={0.85}
                    step={0.02}
                    className="w-full"
                    value={config.appearance.glassOpacity}
                    onChange={(e) =>
                      persist({
                        ...config,
                        appearance: {
                          ...config.appearance,
                          glassOpacity: Number(e.target.value),
                        },
                      })
                    }
                  />
                </label>

                <label className="space-y-1.5">
                  <span className="text-slate-300">
                    译文字号: {config.appearance.translatedFontSize}px
                  </span>
                  <input
                    type="range"
                    min={15}
                    max={36}
                    className="w-full"
                    value={config.appearance.translatedFontSize}
                    onChange={(e) =>
                      persist({
                        ...config,
                        appearance: {
                          ...config.appearance,
                          translatedFontSize: Number(e.target.value),
                        },
                      })
                    }
                  />
                </label>

                <label className="space-y-1.5">
                  <span className="text-slate-300">
                    原文字号: {config.appearance.sourceFontSize}px
                  </span>
                  <input
                    type="range"
                    min={11}
                    max={22}
                    className="w-full"
                    value={config.appearance.sourceFontSize}
                    onChange={(e) =>
                      persist({
                        ...config,
                        appearance: {
                          ...config.appearance,
                          sourceFontSize: Number(e.target.value),
                        },
                      })
                    }
                  />
                </label>

                <label className="space-y-1.5">
                  <span className="text-slate-300">
                    长条显示句数: {config.appearance.maxVisibleLines} 句
                  </span>
                  <input
                    type="range"
                    min={1}
                    max={3}
                    className="w-full"
                    value={config.appearance.maxVisibleLines}
                    onChange={(e) =>
                      persist({
                        ...config,
                        appearance: {
                          ...config.appearance,
                          maxVisibleLines: Number(e.target.value),
                        },
                      })
                    }
                  />
                </label>
              </div>

              <div className="grid grid-cols-2 gap-3 text-xs pt-1">
                <label className="flex items-center justify-between p-3 rounded-xl bg-white/5 cursor-pointer">
                  <span>显示识别原文（双语模式）</span>
                  <input
                    type="checkbox"
                    checked={config.appearance.showSource}
                    onChange={(e) =>
                      persist({
                        ...config,
                        appearance: {
                          ...config.appearance,
                          showSource: e.target.checked,
                        },
                      })
                    }
                  />
                </label>

                <label className="flex items-center justify-between p-3 rounded-xl bg-white/5 cursor-pointer">
                  <span>原文置于译文上方</span>
                  <input
                    type="checkbox"
                    checked={config.appearance.sourceOnTop}
                    onChange={(e) =>
                      persist({
                        ...config,
                        appearance: {
                          ...config.appearance,
                          sourceOnTop: e.target.checked,
                        },
                      })
                    }
                  />
                </label>
              </div>

              <div className="rounded-xl bg-slate-950/50 border border-white/10 p-3 text-xs text-slate-300 space-y-1">
                <div className="font-semibold text-slate-200">全局快捷键</div>
                <div>
                  <code className="text-sky-300">Alt + Shift + S</code>：开始 / 暂停监听
                </div>
                <div>
                  <code className="text-sky-300">Alt + Shift + L</code>：切换鼠标穿透锁（看视频不挡进度条）
                </div>
                <div>
                  <code className="text-sky-300">Alt + Shift + H</code>：隐藏 / 显示字幕条
                </div>
              </div>
            </div>
          )}

          {tab === "history" && (
            <div className="glass-card p-4 space-y-3">
              <div className="flex items-center justify-between">
                <h2 className="text-sm font-semibold text-white">
                  本场实时字幕记录 ({history.length} 句)
                </h2>
                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={() => handleExportSrt(true)}
                    className="px-3 py-1.5 rounded-xl bg-sky-500/20 hover:bg-sky-500/30 border border-sky-400/35 text-sky-200 text-xs inline-flex items-center gap-1.5 cursor-pointer"
                  >
                    <Download className="w-3.5 h-3.5" /> 导出双语 .SRT
                  </button>
                  <button
                    type="button"
                    onClick={() => handleExportSrt(false)}
                    className="px-3 py-1.5 rounded-xl bg-white/10 hover:bg-white/18 text-slate-200 text-xs inline-flex items-center gap-1.5 cursor-pointer"
                  >
                    <Download className="w-3.5 h-3.5" /> 导出纯译文 .SRT
                  </button>
                  <button
                    type="button"
                    onClick={async () => {
                      await api.clearSubtitleHistory();
                      setHistory([]);
                      showToast("已清空字幕记录");
                    }}
                    className="p-1.5 rounded-xl bg-white/8 hover:bg-rose-500/25 text-slate-300 hover:text-rose-200 cursor-pointer"
                    aria-label="清空记录"
                    title="清空记录"
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                  </button>
                </div>
              </div>

              {history.length === 0 ? (
                <div className="py-12 text-center text-xs text-slate-500">
                  暂无翻译记录 — 开启监听并播放任意视频后，字幕将实时归档于此。
                </div>
              ) : (
                <div className="space-y-2 max-h-[440px] overflow-y-auto glass-scroll pr-1">
                  {history.map((item) => (
                    <div
                      key={item.id}
                      className="p-3 rounded-xl bg-white/4 border border-white/8 space-y-1"
                    >
                      <div className="flex items-center justify-between text-[11px] text-slate-400">
                        <span>{new Date(item.timestampMs).toLocaleTimeString()}</span>
                        <span className="font-mono">
                          {item.asrLatencyMs != null ? `ASR ${item.asrLatencyMs}ms · ` : ""}
                          {item.translateLatencyMs != null
                            ? `LLM ${item.translateLatencyMs}ms`
                            : ""}
                        </span>
                      </div>
                      <div className="text-xs text-slate-400">{item.sourceText}</div>
                      <div className="text-sm text-white font-medium">
                        {item.translatedText}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </section>
      </div>

      {/* Live status toast */}
      <div
        role="status"
        aria-live="polite"
        className="fixed bottom-4 right-5 pointer-events-none"
      >
        {toast && (
          <div className="px-4 py-2 rounded-full bg-slate-900/95 border border-sky-400/40 text-xs text-sky-100 shadow-lg">
            {toast}
          </div>
        )}
      </div>
    </div>
  );
};

interface PathRowProps {
  label: string;
  hint: string;
  value: string;
  onChange: (val: string) => void;
  onBrowse: () => void;
}

const PathRow: React.FC<PathRowProps> = ({
  label,
  hint,
  value,
  onChange,
  onBrowse,
}) => {
  const filled = value.trim().length > 0;
  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between text-xs">
        <span className="text-slate-200 font-medium">{label}</span>
        <span className={filled ? "text-emerald-400" : "text-amber-400"}>
          {filled ? "已配置" : "未指定"}
        </span>
      </div>
      <div className="flex gap-2">
        <input
          type="text"
          className="glass-input flex-1 font-mono text-[12px]"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder="点击右侧选择本地路径..."
        />
        <button
          type="button"
          onClick={onBrowse}
          className="px-3 py-2 rounded-xl bg-white/10 hover:bg-white/18 text-slate-200 text-xs inline-flex items-center gap-1.5 cursor-pointer shrink-0"
        >
          <FolderOpen className="w-3.5 h-3.5" /> 选择
        </button>
      </div>
      <p className="text-[11px] text-slate-500">{hint}</p>
    </div>
  );
};
