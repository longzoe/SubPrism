import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  AudioSourceInfo,
  EngineStatus,
  HardwareInfo,
  PathValidationResult,
  PipelineMetrics,
  SubtitleEntry,
  SubtitleEvent,
  SubPrismConfig,
} from "./types";

export const SUBTITLE_EVENT_NAME = "subprism://subtitle-event";
export const CONFIG_UPDATED_EVENT = "subprism://config-updated";

export const api = {
  getConfig: () => invoke<SubPrismConfig>("get_config"),
  saveConfig: (config: SubPrismConfig) =>
    invoke<SubPrismConfig>("save_config", { config }),
  validatePaths: () => invoke<PathValidationResult>("validate_paths"),
  getHardwareInfo: () => invoke<HardwareInfo>("get_hardware_info"),
  listAudioSources: () => invoke<AudioSourceInfo[]>("list_audio_sources"),
  getEngineStatus: () => invoke<EngineStatus>("get_engine_status"),
  getMetrics: () => invoke<PipelineMetrics>("get_metrics"),
  getSubtitleHistory: () => invoke<SubtitleEntry[]>("get_subtitle_history"),
  clearSubtitleHistory: () => invoke<void>("clear_subtitle_history"),
  startListening: () => invoke<void>("start_listening"),
  stopListening: () => invoke<void>("stop_listening"),
  toggleListening: () => invoke<void>("toggle_listening"),
  testTranslate: (text: string) =>
    invoke<SubtitleEntry>("test_translate", { text }),
  previewSystemPrompt: () => invoke<string>("preview_system_prompt"),
  exportSrt: (path: string, includeSource: boolean) =>
    invoke<number>("export_srt", { path, includeSource }),
  setClickThrough: (enabled: boolean) =>
    invoke<void>("set_click_through", { enabled }),
  openSettingsWindow: () => invoke<void>("open_settings_window"),
  recenterOverlay: () => invoke<void>("recenter_overlay"),

  listenSubtitleEvents: (cb: (ev: SubtitleEvent) => void): Promise<UnlistenFn> =>
    listen<SubtitleEvent>(SUBTITLE_EVENT_NAME, (e) => cb(e.payload)),

  listenConfigUpdates: (cb: (cfg: SubPrismConfig) => void): Promise<UnlistenFn> =>
    listen<SubPrismConfig>(CONFIG_UPDATED_EVENT, (e) => cb(e.payload)),

  pickFile: async (title: string, filters?: { name: string; extensions: string[] }[]) => {
    const res = await open({ title, multiple: false, directory: false, filters });
    return typeof res === "string" ? res : null;
  },

  pickDirectory: async (title: string) => {
    const res = await open({ title, multiple: false, directory: true });
    return typeof res === "string" ? res : null;
  },

  pickSaveSrt: async () => {
    const res = await save({
      title: "导出 SRT 字幕文件",
      defaultPath: "subprism-subtitles.srt",
      filters: [{ name: "SubRip 字幕", extensions: ["srt"] }],
    });
    return typeof res === "string" ? res : null;
  },
};
