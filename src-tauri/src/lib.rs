mod commands;
mod pipeline;
mod window_glass;

use crate::commands::CONFIG_UPDATED_EVENT;
use crate::pipeline::PipelineController;
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tracing::{info, warn};
use subprism_core::SubPrismConfig;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "subprism=info,subprism_audio=info,subprism_asr=info,subprism_llm=info".into()),
        )
        .init();

    let initial_cfg = SubPrismConfig::load_or_default();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(move |app| {
            let handle = app.handle().clone();
            let controller = Arc::new(PipelineController::new(handle.clone(), initial_cfg.clone()));
            app.manage(controller.clone());

            // Configure Overlay Liquid Glass window
            if let Some(overlay) = app.get_webview_window("overlay") {
                window_glass::position_overlay_bottom_center(&overlay);
                window_glass::apply_overlay_glass(&overlay, initial_cfg.appearance.theme);
                let _ = overlay.set_ignore_cursor_events(initial_cfg.appearance.click_through);
            }

            // Configure Settings window (hide on close instead of destroying)
            if let Some(settings) = app.get_webview_window("settings") {
                window_glass::apply_settings_glass(&settings);
                let settings_ref = settings.clone();
                settings.on_window_event(move |ev| {
                    if let WindowEvent::CloseRequested { api, .. } = ev {
                        api.prevent_close();
                        let _ = settings_ref.hide();
                    }
                });
            }

            // If user hasn't configured model paths yet (fresh release install), surface Settings automatically
            if !initial_cfg.paths.is_complete() {
                if let Some(settings) = app.get_webview_window("settings") {
                    let _ = settings.show();
                    let _ = settings.set_focus();
                }
            }

            setup_tray(&handle, controller.clone())?;
            register_shortcuts(&handle, controller.clone());

            info!("SubPrism 跨平台本地 AI 实时字幕翻译软件已启动");
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "overlay" {
                if let WindowEvent::Destroyed = event {
                    let ctrl = window.state::<Arc<PipelineController>>().inner().clone();
                    tauri::async_runtime::spawn(async move {
                        ctrl.shutdown_all().await;
                    });
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::validate_paths,
            commands::get_hardware_info,
            commands::list_audio_sources,
            commands::get_engine_status,
            commands::get_metrics,
            commands::get_subtitle_history,
            commands::clear_subtitle_history,
            commands::start_listening,
            commands::stop_listening,
            commands::toggle_listening,
            commands::test_translate,
            commands::preview_system_prompt,
            commands::export_srt,
            commands::set_click_through,
            commands::open_settings_window,
            commands::recenter_overlay,
        ])
        .build(tauri::generate_context!())
        .expect("构建 Tauri 应用失败")
        .run(|app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit = event {
                if let Some(ctrl) = app_handle.try_state::<Arc<PipelineController>>() {
                    let ctrl_ref = ctrl.inner().clone();
                    tauri::async_runtime::block_on(async move {
                        ctrl_ref.shutdown_all().await;
                    });
                }
            }
        });
}

fn setup_tray(app: &AppHandle, ctrl: Arc<PipelineController>) -> tauri::Result<()> {
    let toggle_listen = MenuItem::with_id(app, "toggle_listen", "开始 / 暂停监听 (Alt+Shift+S)", true, None::<&str>)?;
    let toggle_overlay = MenuItem::with_id(app, "toggle_overlay", "显示 / 隐藏字幕条 (Alt+Shift+H)", true, None::<&str>)?;
    let toggle_click = MenuItem::with_id(app, "toggle_click", "切换鼠标穿透锁 (Alt+Shift+L)", true, None::<&str>)?;
    let open_settings = MenuItem::with_id(app, "open_settings", "设置与本地 AI 模型...", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出 SubPrism", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &toggle_listen,
            &toggle_overlay,
            &toggle_click,
            &open_settings,
            &quit_item,
        ],
    )?;

    let icon = app.default_window_icon().cloned();
    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("SubPrism — 本地 AI 实时字幕翻译")
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref();
            match id {
                "toggle_listen" => {
                    let c = ctrl.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = c.toggle_listening().await;
                    });
                }
                "toggle_overlay" => toggle_overlay_visibility(app),
                "toggle_click" => {
                    let app_c = app.clone();
                    let c = ctrl.clone();
                    tauri::async_runtime::spawn(async move {
                        toggle_click_through_action(&app_c, &c).await;
                    });
                }
                "open_settings" => {
                    if let Some(win) = app.get_webview_window("settings") {
                        let _ = win.unminimize();
                        let _ = win.show();
                        let _ = win.set_focus();
                    }
                }
                "quit" => {
                    let app_c = app.clone();
                    let c = ctrl.clone();
                    tauri::async_runtime::spawn(async move {
                        c.shutdown_all().await;
                        app_c.exit(0);
                    });
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(win) = app.get_webview_window("overlay") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
        });

    if let Some(ic) = icon {
        builder = builder.icon(ic);
    }

    let _tray = builder.build(app)?;
    Ok(())
}

fn register_shortcuts(app: &AppHandle, ctrl: Arc<PipelineController>) {
    let sc_click = Shortcut::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyL);
    let sc_listen = Shortcut::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyS);
    let sc_hide = Shortcut::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyH);

    let ctrl_l = ctrl.clone();
    if let Err(e) = app.global_shortcut().on_shortcut(sc_click, move |app, _sc, ev| {
        if ev.state() == ShortcutState::Pressed {
            let app_c = app.clone();
            let c = ctrl_l.clone();
            tauri::async_runtime::spawn(async move {
                toggle_click_through_action(&app_c, &c).await;
            });
        }
    }) {
        warn!("注册全局快捷键 Alt+Shift+L 失败: {e}");
    }

    let ctrl_s = ctrl.clone();
    if let Err(e) = app.global_shortcut().on_shortcut(sc_listen, move |_app, _sc, ev| {
        if ev.state() == ShortcutState::Pressed {
            let c = ctrl_s.clone();
            tauri::async_runtime::spawn(async move {
                let _ = c.toggle_listening().await;
            });
        }
    }) {
        warn!("注册全局快捷键 Alt+Shift+S 失败: {e}");
    }

    if let Err(e) = app.global_shortcut().on_shortcut(sc_hide, move |app, _sc, ev| {
        if ev.state() == ShortcutState::Pressed {
            toggle_overlay_visibility(app);
        }
    }) {
        warn!("注册全局快捷键 Alt+Shift+H 失败: {e}");
    }
}

fn toggle_overlay_visibility(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("overlay") {
        if win.is_visible().unwrap_or(true) {
            let _ = win.hide();
        } else {
            let _ = win.show();
            let _ = win.set_focus();
        }
    }
}

async fn toggle_click_through_action(app: &AppHandle, ctrl: &Arc<PipelineController>) {
    let mut cfg = ctrl.get_config().await;
    cfg.appearance.click_through = !cfg.appearance.click_through;
    let enabled = cfg.appearance.click_through;
    let _ = ctrl.update_config(cfg.clone()).await;
    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.set_ignore_cursor_events(enabled);
    }
    let _ = app.emit(CONFIG_UPDATED_EVENT, &cfg);
}
