//! SigmaOS platform — hardware and platform capability detection.
//!
//! Reads /proc, /sys, and os-release to report CPU, memory, GPU hints,
//! init system, and session type. Pure safe reads, no unsafe code.

#![forbid(unsafe_code)]

use serde::Serialize;
use std::path::Path;

/// Errors produced by this component.
#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid data: {0}")]
    InvalidData(String),
    #[error("not found: {0}")]
    NotFound(String),
}

/// Detected platform information.
#[derive(Debug, Clone, Serialize)]
pub struct PlatformInfo {
    pub os_name: String,
    pub os_version: String,
    pub kernel_release: String,
    pub arch: String,
    pub cpu_model: String,
    pub cpu_cores: usize,
    pub memory_total_kb: u64,
    pub is_wayland: bool,
    pub init_system: String,
    pub hostname: String,
    pub uptime_seconds: u64,
    /// DRM kernel driver bound to the first GPU (e.g. "amdgpu", "i915",
    /// "nouveau"), or `None` when no GPU is detected.
    pub gpu_driver: Option<String>,
}

fn read_trim(path: &Path) -> Result<String, PlatformError> {
    Ok(std::fs::read_to_string(path)?.trim().to_string())
}

/// Parse /etc/os-release into (name, version).
fn os_release() -> (String, String) {
    let Ok(text) = std::fs::read_to_string("/etc/os-release") else {
        return ("unknown".into(), "unknown".into());
    };
    let mut name = "unknown".to_string();
    let mut version = "unknown".to_string();
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("NAME=") {
            name = v.trim_matches('"').to_string();
        } else if let Some(v) = line.strip_prefix("VERSION_ID=") {
            version = v.trim_matches('"').to_string();
        }
    }
    (name, version)
}

/// Count logical CPUs from /proc/cpuinfo.
fn cpu_cores() -> usize {
    std::fs::read_to_string("/proc/cpuinfo")
        .map(|t| t.lines().filter(|l| l.starts_with("processor")).count())
        .unwrap_or(0)
}

/// CPU model name from /proc/cpuinfo.
fn cpu_model() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|t| {
            t.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split(':').nth(1))
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".into())
}

/// Total memory in kB from /proc/meminfo.
fn memory_total_kb() -> u64 {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|t| {
            t.lines()
                .find(|l| l.starts_with("MemTotal"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse::<u64>().ok())
        })
        .unwrap_or(0)
}

/// Detect init system by checking /run/systemd/system existence.
fn init_system() -> String {
    if Path::new("/run/systemd/system").exists() {
        "systemd".into()
    } else if Path::new("/etc/runit").exists() {
        "runit".into()
    } else {
        "sysvinit".into()
    }
}

/// Detect Wayland vs X11 session.
fn is_wayland() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|v| v == "wayland")
            .unwrap_or(false)
}

/// Uptime in seconds from /proc/uptime.
fn uptime_seconds() -> u64 {
    std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|t| t.split_whitespace().next()?.parse::<f64>().ok())
        .map(|s| s as u64)
        .unwrap_or(0)
}

/// DRM driver of the first GPU under /sys/class/drm (e.g. "amdgpu").
fn gpu_driver() -> Option<String> {
    let mut cards: Vec<_> = std::fs::read_dir("/sys/class/drm")
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("card") && !n.contains('-'))
        .collect();
    cards.sort();
    for card in cards {
        let uevent = Path::new("/sys/class/drm")
            .join(&card)
            .join("device/uevent");
        if let Ok(text) = std::fs::read_to_string(uevent) {
            for line in text.lines() {
                if let Some(v) = line.strip_prefix("DRIVER=") {
                    if !v.is_empty() {
                        return Some(v.to_string());
                    }
                }
            }
        }
    }
    None
}

/// Gather full platform info.
pub fn detect() -> Result<PlatformInfo, PlatformError> {
    let (os_name, os_version) = os_release();
    Ok(PlatformInfo {
        os_name,
        os_version,
        kernel_release: read_trim(Path::new("/proc/sys/kernel/osrelease"))
            .unwrap_or_else(|_| "unknown".into()),
        arch: std::env::consts::ARCH.to_string(),
        cpu_model: cpu_model(),
        cpu_cores: cpu_cores(),
        memory_total_kb: memory_total_kb(),
        is_wayland: is_wayland(),
        init_system: init_system(),
        hostname: read_trim(Path::new("/etc/hostname"))
            .or_else(|_| {
                std::env::var("HOSTNAME").map_err(|_| PlatformError::NotFound("hostname".into()))
            })
            .unwrap_or_else(|_| "unknown".into()),
        uptime_seconds: uptime_seconds(),
        gpu_driver: gpu_driver(),
    })
}

/// Check whether a CPU feature flag is present (e.g. "avx2").
pub fn cpu_feature(feature: &str) -> bool {
    std::fs::read_to_string("/proc/cpuinfo")
        .map(|t| {
            t.lines()
                .find(|l| l.starts_with("flags") || l.starts_with("Features"))
                .map(|l| l.split_whitespace().any(|w| w == feature))
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

/// Total memory formatted as human-readable string.
pub fn memory_human(info: &PlatformInfo) -> String {
    let gib = info.memory_total_kb as f64 / 1024.0 / 1024.0;
    format!("{gib:.1} GiB")
}

/// SigmaOS targets kernel >= 6.1 LTS.
pub fn kernel_supported(release: &str) -> bool {
    let mut parts = release.split('.');
    let major = parts.next().and_then(|v| v.parse::<u32>().ok());
    let minor = parts.next().and_then(|v| v.parse::<u32>().ok());
    match (major, minor) {
        (Some(maj), Some(min)) => maj > 6 || (maj == 6 && min >= 1),
        (Some(maj), None) => maj > 6,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_support_boundary() {
        assert!(!kernel_supported("6.0.21"));
        assert!(kernel_supported("6.1.0"));
        assert!(kernel_supported("6.12.1"));
        assert!(kernel_supported("7.0"));
        assert!(!kernel_supported("5.15.0"));
        assert!(!kernel_supported("garbage"));
    }

    #[test]
    fn detect_returns_platform_info() {
        let info = detect().unwrap();
        assert!(!info.kernel_release.is_empty());
        assert!(info.cpu_cores > 0 || cfg!(target_os = "unknown"));
    }
}
