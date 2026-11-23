use crate::pipeline::PipelineController;
use crate::window_glass;
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use subprism_core::{
    build_system_prompt, detect_hardware, AudioSourceInfo, EngineStatus, HardwareInfo,
    PipelineMetrics, SubtitleEntry, SubPrismConfig,
};

pub const CONFIG_UPDATED_EVENT: &str = "subprism://config-updated";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathValidationResult {
    pub valid: bool,
    pub missing: Vec<String>,
}

#[tauri::command]
pub async fn get_config(ctrl: State<'_, Arc<PipelineController>>) -> Result<SubPrismConfig, String> {
    Ok(ctrl.get_config().await)
}

#[tauri::command]
pub async fn save_config(
    app: AppHandle,
    ctrl: State<'_, Arc<PipelineController>>,
    config: SubPrismConfig,
) -> Result<SubPrismConfig, String> {
    ctrl.update_config(config.clone())
        .await
        .map_err(|e| format!("{e:#}"))?;

    if let Some(overlay) = app.get_webview_window("overlay") {
        window_glass::apply_overlay_glass(&overlay, config.appearance.theme);
        let _ = overlay.set_ignore_cursor_events(config.appearance.click_through);
    }

    let _ = app.emit(CONFIG_UPDATED_EVENT, &config);
    Ok(config)
}

#[tauri::command]
pub async fn validate_paths(
    ctrl: State<'_, Arc<PipelineController>>,
) -> Result<PathValidationResult, String> {
    let cfg = ctrl.get_config().await;
    let missing = cfg.paths.validate();
    Ok(PathValidationResult {
        valid: missing.is_empty(),
        missing,
    })
}

#[tauri::command]
pub async fn get_hardware_info() -> Result<HardwareInfo, String> {
    Ok(tokio::task::spawn_blocking(detect_hardware)
        .await
        .map_err(|e| e.to_string())?)
}

#[tauri::command]
pub async fn list_audio_sources() -> Result<Vec<AudioSourceInfo>, String> {
    Ok(tokio::task::spawn_blocking(subprism_audio::list_audio_sources)
        .await
        .map_err(|e| e.to_string())?)
}

#[tauri::command]
pub async fn get_engine_status(
    ctrl: State<'_, Arc<PipelineController>>,
) -> Result<EngineStatus, String> {
    Ok(ctrl.get_status().await)
}

#[tauri::command]
pub async fn get_metrics(
    ctrl: State<'_, Arc<PipelineController>>,
) -> Result<PipelineMetrics, String> {
    Ok(ctrl.get_metrics().await)
}

#[tauri::command]
pub async fn get_subtitle_history(
    ctrl: State<'_, Arc<PipelineController>>,
) -> Result<Vec<SubtitleEntry>, String> {
    Ok(ctrl.get_history().await)
}

#[tauri::command]
pub async fn clear_subtitle_history(
    ctrl: State<'_, Arc<PipelineController>>,
) -> Result<(), String> {
    ctrl.clear_history().await;
    Ok(())
}

#[tauri::command]
pub async fn start_listening(ctrl: State<'_, Arc<PipelineController>>) -> Result<(), String> {
    ctrl.start_listening().await.map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn stop_listening(ctrl: State<'_, Arc<PipelineController>>) -> Result<(), String> {
    ctrl.stop_listening().await;
    Ok(())
}

#[tauri::command]
pub async fn toggle_listening(ctrl: State<'_, Arc<PipelineController>>) -> Result<(), String> {
    ctrl.toggle_listening().await.map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn test_translate(
    ctrl: State<'_, Arc<PipelineController>>,
    text: String,
) -> Result<SubtitleEntry, String> {
    ctrl.test_translate(text)
        .await
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn preview_system_prompt(
    ctrl: State<'_, Arc<PipelineController>>,
) -> Result<String, String> {
    let cfg = ctrl.get_config().await;
    Ok(build_system_prompt(
        &cfg.target_language,
        cfg.style_preset,
        &cfg.glossary,
    ))
}

#[tauri::command]
pub async fn export_srt(
    ctrl: State<'_, Arc<PipelineController>>,
    path: String,
    include_source: bool,
) -> Result<usize, String> {
    ctrl.export_srt(Path::new(&path), include_source)
        .await
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn set_click_through(
    app: AppHandle,
    ctrl: State<'_, Arc<PipelineController>>,
    enabled: bool,
) -> Result<(), String> {
    let mut cfg = ctrl.get_config().await;
    cfg.appearance.click_through = enabled;
    ctrl.update_config(cfg.clone())
        .await
        .map_err(|e| format!("{e:#}"))?;
    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.set_ignore_cursor_events(enabled);
    }
    let _ = app.emit(CONFIG_UPDATED_EVENT, &cfg);
    Ok(())
}

#[tauri::command]
pub async fn open_settings_window(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
    }
    Ok(())
}

#[tauri::command]
pub async fn recenter_overlay(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("overlay") {
        window_glass::position_overlay_bottom_center(&win);
    }
    Ok(())
}
