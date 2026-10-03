use std::sync::atomic::{AtomicBool, Ordering};
use serde::Serialize;
use sysinfo::System;
use tauri::{AppHandle, Emitter, Manager};

use crate::{dictation::TelemetryStats, platform::desktop_environment, AppState, LockExt};

#[derive(Debug, Serialize, Clone)]
pub struct TelemetryPayload {
    pub anonymous_id: String,
    pub os_name: String,
    pub os_version: String,
    pub arch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distribution_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desktop_env: Option<String>,
    pub app_version: String,
    pub model: String,
    pub stats: TelemetryStats,
}

pub fn create_payload(
    app: AppHandle,
    model: String,
    anonymous_id: String,
    stats: TelemetryStats,
) -> TelemetryPayload {
    let os_name = System::name().unwrap_or_else(|| std::env::consts::OS.to_string());
    let os_version = System::os_version().unwrap_or_else(|| "Unknown".to_string());
    let arch = std::env::consts::ARCH.to_string();
    let kernel_version = System::kernel_version();
    let dist_id = System::distribution_id();
    let distribution_id = if dist_id.is_empty() {
        None
    } else {
        Some(dist_id)
    };
    let desktop_env = desktop_environment();

    TelemetryPayload {
        anonymous_id,
        os_name,
        os_version,
        arch,
        kernel_version,
        distribution_id,
        desktop_env,
        app_version: app.package_info().version.to_string(),
        model,
        stats,
    }
}

static EXITING: AtomicBool = AtomicBool::new(false);

pub fn is_exiting() -> bool {
    EXITING.load(Ordering::SeqCst)
}

/// Prompts the UI to display the telemetry payload before exiting.
pub fn prompt_exit_telemetry(app: &AppHandle) {
    let state = app.state::<AppState>();
    let settings = state.settings();
    let model = settings.selected_model;
    let anonymous_id = settings.anonymous_id;
    let stats = state.telemetry.lock_unpoisoned().clone();
    let payload = create_payload(app.clone(), model, anonymous_id, stats);
    crate::tray::open_main_window(app, None);
    let _ = app.emit("show-telemetry-payload", &payload);
}

/// Gathers telemetry, logs/emits payload, and cleanly shuts down the application.
pub fn clean_exit(app: &AppHandle) {
    if EXITING.swap(true, Ordering::SeqCst) {
        return;
    }
    let state = app.state::<AppState>();
    if !state.settings().telemetry_enabled {
        eprintln!("[telemetry] disabled by user settings");
        app.exit(0);
        return;
    }
    let settings = state.settings();
    let model = settings.selected_model;
    let anonymous_id = settings.anonymous_id;
    let stats = state.telemetry.lock_unpoisoned().clone();
    let payload = create_payload(app.clone(), model, anonymous_id, stats);
    eprintln!("Exit telemetry:");
    eprintln!("{payload:?}");

    app.exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linux_payload_serialization() {
        let stats = TelemetryStats::new();
        let payload = TelemetryPayload {
            anonymous_id: "test-anon-id-123".into(),
            os_name: "Linux".into(),
            os_version: "44".into(),
            arch: "x86_64".into(),
            kernel_version: Some("6.8.0".into()),
            distribution_id: Some("fedora".into()),
            desktop_env: Some("GNOME (Wayland)".into()),
            app_version: "2.0.0".into(),
            model: "whisper-base".into(),
            stats,
        };

        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"anonymous_id\":\"test-anon-id-123\""));
        assert!(json.contains("\"distribution_id\":\"fedora\""));
        assert!(json.contains("\"desktop_env\":\"GNOME (Wayland)\""));
        assert!(json.contains("\"arch\":\"x86_64\""));
    }

    #[test]
    fn test_macos_or_windows_payload_omits_none_fields() {
        let stats = TelemetryStats::new();
        let payload = TelemetryPayload {
            anonymous_id: "test-anon-id-456".into(),
            os_name: "macOS".into(),
            os_version: "15.1".into(),
            arch: "aarch64".into(),
            kernel_version: Some("24.1.0".into()),
            distribution_id: None,
            desktop_env: None,
            app_version: "2.0.0".into(),
            model: "whisper-base".into(),
            stats,
        };

        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"anonymous_id\":\"test-anon-id-456\""));
        assert!(!json.contains("distribution_id"));
        assert!(!json.contains("desktop_env"));
        assert!(json.contains("\"os_name\":\"macOS\""));
        assert!(json.contains("\"arch\":\"aarch64\""));
    }
}
