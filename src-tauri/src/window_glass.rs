use tauri::{PhysicalPosition, WebviewWindow};
#[cfg(target_os = "windows")]
use tracing::debug;
#[cfg(target_os = "macos")]
use tracing::warn;
use subprism_core::GlassTheme;

/// Positions the overlay subtitle bar cleanly at bottom-center of the primary monitor.
pub fn position_overlay_bottom_center(window: &WebviewWindow) {
    if let Ok(Some(monitor)) = window.primary_monitor() {
        let mon_size = monitor.size();
        let mon_pos = monitor.position();
        if let Ok(win_size) = window.outer_size() {
            let x = mon_pos.x + ((mon_size.width as i32 - win_size.width as i32) / 2);
            let bottom_margin = (76.0 * monitor.scale_factor()) as i32;
            let y = mon_pos.y + (mon_size.height as i32 - win_size.height as i32 - bottom_margin);
            let _ = window.set_position(PhysicalPosition::new(x, y.max(mon_pos.y + 24)));
        }
    }
}

/// Applies platform-native liquid glass translucency & fullscreen overlay flags.
pub fn apply_overlay_glass(window: &WebviewWindow, theme: GlassTheme) {
    #[cfg(target_os = "windows")]
    {
        apply_windows_backdrop(window, theme, true);
    }

    #[cfg(target_os = "macos")]
    {
        apply_macos_glass(window, theme, 28.0);
        configure_macos_fullscreen_overlay(window);
    }
}

/// Applies native translucent glass backdrop to the Settings window.
pub fn apply_settings_glass(window: &WebviewWindow) {
    #[cfg(target_os = "windows")]
    {
        apply_windows_backdrop(window, GlassTheme::SmokedDark, false);
    }

    #[cfg(target_os = "macos")]
    {
        apply_macos_glass(window, GlassTheme::SmokedDark, 16.0);
    }
}

#[cfg(target_os = "windows")]
fn apply_windows_backdrop(window: &WebviewWindow, theme: GlassTheme, is_overlay: bool) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_SYSTEMBACKDROP_TYPE,
        DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    };

    let Ok(raw_hwnd) = window.hwnd() else {
        return;
    };
    let hwnd = HWND(raw_hwnd.0 as *mut _);

    unsafe {
        // Use dark immersive mode for SmokedDark so DWM tint matches dark liquid glass
        let dark_mode: i32 = match theme {
            GlassTheme::CrystalLight => 0,
            _ => 1,
        };
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark_mode as *const _ as *const _,
            std::mem::size_of::<i32>() as u32,
        );

        // Native Windows 11 rounded corners
        let corner = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const _ as *const _,
            std::mem::size_of_val(&corner) as u32,
        );

        if is_overlay {
            // Suppress default gray Win11 DWM border so CSS specular glass rim shines cleanly
            const DWMWA_COLOR_NONE: u32 = 0xFFFFFFFE;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR,
                &DWMWA_COLOR_NONE as *const _ as *const _,
                std::mem::size_of::<u32>() as u32,
            );
        }

        // DWM SystemBackdrop:
        // 1 = DWMSBT_NONE, 2 = DWMSBT_MAINWINDOW (Mica), 3 = DWMSBT_TRANSIENTWINDOW (Acrylic)
        let backdrop_type: i32 = match theme {
            GlassTheme::MinimalText => 1,
            _ => 3, // Zero-lag GPU-composited Acrylic that blurs live video underneath
        };

        let dwm_res = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &backdrop_type as *const _ as *const _,
            std::mem::size_of::<i32>() as u32,
        );

        if dwm_res.is_err() {
            debug!("DWM SystemBackdrop 不可用（可能为 Windows 10），尝试回退 window-vibrancy Acrylic");
            match theme {
                GlassTheme::MinimalText => {
                    let _ = window_vibrancy::clear_acrylic(window);
                }
                GlassTheme::CrystalLight => {
                    let _ = window_vibrancy::apply_acrylic(window, Some((240, 246, 255, 70)));
                }
                GlassTheme::SmokedDark => {
                    let _ = window_vibrancy::apply_acrylic(window, Some((14, 18, 28, 115)));
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn apply_macos_glass(window: &WebviewWindow, theme: GlassTheme, radius: f64) {
    use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};

    let material = match theme {
        GlassTheme::MinimalText => {
            let _ = window_vibrancy::clear_vibrancy(window);
            return;
        }
        GlassTheme::CrystalLight => NSVisualEffectMaterial::Popover,
        GlassTheme::SmokedDark => NSVisualEffectMaterial::HudWindow,
    };

    if let Err(e) = apply_vibrancy(
        window,
        material,
        Some(NSVisualEffectState::Active),
        Some(radius),
    ) {
        warn!("应用 macOS 原生玻璃材质失败: {e}");
    }
}

#[cfg(target_os = "macos")]
fn configure_macos_fullscreen_overlay(window: &WebviewWindow) {
    use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};

    if let Ok(ns_win_ptr) = window.ns_window() {
        unsafe {
            let ns_win = &*(ns_win_ptr as *const NSWindow);
            // Keep subtitle bar visible across all Spaces and above native fullscreen video apps
            let behavior = NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Stationary;
            ns_win.setCollectionBehavior(behavior);
            ns_win.setHasShadow(false);
        }
    }
}
