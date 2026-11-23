export type StylePreset = "general" | "tech" | "cinema" | "anime";
export type GlassTheme = "smoked_dark" | "crystal_light" | "minimal_text";
export type SubtitleStatus = "partial" | "translating" | "final";
export type EnginePhase =
  | "idle"
  | "needs_setup"
  | "starting_llm"
  | "loading_asr"
  | "listening"
  | "paused"
  | "error";

export interface GlossaryEntry {
  source: string;
  target: string;
}

export interface ModelPaths {
  llamaServerBin: string;
  llmModelGguf: string;
  sherpaLibPath: string;
  asrModelDir: string;
  vadModelPath: string;
}

export interface LlmConfig {
  gpuLayers: number;
  contextSize: number;
  parallelSlots: number;
  temperature: number;
  topP: number;
  repeatPenalty: number;
  contextTurns: number;
  translatePartials: boolean;
}

export interface AsrConfig {
  sourceLanguage: string;
  numThreads: number;
  vadThreshold: number;
  minSilenceMs: number;
  maxUtteranceMs: number;
  partialIntervalMs: number;
  softSpeechBoost: boolean;
  agcMaxGainDb: number;
}

export interface AppearanceConfig {
  theme: GlassTheme;
  glassOpacity: number;
  translatedFontSize: number;
  sourceFontSize: number;
  showSource: boolean;
  sourceOnTop: boolean;
  maxVisibleLines: number;
  clickThrough: boolean;
  overlayWidth: number;
  overlayHeight: number;
}

export interface SubPrismConfig {
  targetLanguage: string;
  stylePreset: StylePreset;
  audioSourceId: string;
  glossary: GlossaryEntry[];
  paths: ModelPaths;
  llm: LlmConfig;
  asr: AsrConfig;
  appearance: AppearanceConfig;
}

export interface SubtitleEntry {
  id: string;
  timestampMs: number;
  sourceLang: string;
  sourceText: string;
  translatedText: string;
  status: SubtitleStatus;
  asrLatencyMs?: number | null;
  ttftMs?: number | null;
  translateLatencyMs?: number | null;
}

export interface EngineStatus {
  phase: EnginePhase;
  message: string;
  llmReady: boolean;
  asrReady: boolean;
  audioActive: boolean;
  llamaEndpoint?: string | null;
  llamaVersion?: string | null;
}

export interface PipelineMetrics {
  lastAsrMs: number;
  lastTtftMs: number;
  lastTranslateMs: number;
  tokensPerSec: number;
  totalUtterances: number;
}

export interface HardwareInfo {
  os: string;
  arch: string;
  backend: "cuda" | "metal" | "vulkan" | "cpu";
  gpuName?: string | null;
  vramMb?: number | null;
  cudaVersion?: string | null;
  recommendedNgl: number;
  recommendedThreads: number;
}

export interface AudioSourceInfo {
  id: string;
  name: string;
  kind: string;
  pid?: number | null;
}

export interface PathValidationResult {
  valid: boolean;
  missing: string[];
}

export type SubtitleEvent =
  | { type: "upsert"; entry: SubtitleEntry }
  | { type: "audio_level"; rms: number; speaking: boolean }
  | { type: "status_changed"; status: EngineStatus }
  | { type: "metrics"; metrics: PipelineMetrics }
  | { type: "error"; message: string };
