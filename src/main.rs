//! CLI for sigmaos-platform.

use clap::Parser;
use sigmaos_platform::{cpu_feature, detect, memory_human};

#[derive(Parser)]
#[command(name = "sigmaos-platform", about = "SigmaOS platform detection")]
struct Cli {
    /// Check if a CPU feature flag exists.
    #[arg(long)]
    cpu_feature: Option<String>,
    /// Output as JSON.
    #[arg(long)]
    json: bool,
    /// Validate hardware meets SigmaOS minimum requirements.
    #[arg(long)]
    validate: bool,
}

/// Minimum requirements for a SigmaOS install.
const MIN_MEMORY_KB: u64 = 2 * 1024 * 1024; // 2 GiB
const MIN_CORES: usize = 2;
const MIN_DISK_FREE_BYTES: u64 = 20 * 1024 * 1024 * 1024; // 20 GiB

fn check(name: &str, ok: bool, detail: String) -> bool {
    println!("  [{}] {} — {}", if ok { "PASS" } else { "FAIL" }, name, detail);
    ok
}

fn validate(info: &sigmaos_platform::PlatformInfo) -> bool {
    println!("SigmaOS hardware validation");
    let mut all_ok = true;

    all_ok &= check(
        "memory",
        info.memory_total_kb >= MIN_MEMORY_KB,
        format!("{} (min {} GiB)", sigmaos_platform::memory_human(info), MIN_MEMORY_KB / 1024 / 1024),
    );
    all_ok &= check(
        "cpu cores",
        info.cpu_cores >= MIN_CORES,
        format!("{} (min {})", info.cpu_cores, MIN_CORES),
    );
    all_ok &= check(
        "cpu features",
        sigmaos_platform::cpu_feature("sse4_2"),
        "sse4_2 required".to_string(),
    );
    all_ok &= check(
        "session",
        info.is_wayland || std::env::var("XDG_SESSION_TYPE").is_ok(),
        if info.is_wayland { "wayland".into() } else { "x11".into() },
    );
    all_ok &= check(
        "init",
        info.init_system == "systemd",
        format!("{} (systemd recommended)", info.init_system),
    );
    all_ok &= check(
        "disk space",
        disk_free() >= MIN_DISK_FREE_BYTES,
        format!("{} free (min 20 GiB)", human_bytes(disk_free())),
    );
    all_ok &= check(
        "kernel",
        kernel_supported(&info.kernel_release),
        format!("{}", info.kernel_release),
    );

    println!("result: {}", if all_ok { "PASS" } else { "FAIL" });
    all_ok
}

/// Free bytes on the filesystem containing the home directory.
fn disk_free() -> u64 {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
    // statvfs via /proc/mounts is not directly readable; use `df` fallback.
    std::process::Command::new("df")
        .args(["-B1", "--output=avail", &home])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|t| t.lines().nth(1)?.trim().parse::<u64>().ok())
        .unwrap_or(0)
}

fn human_bytes(b: u64) -> String {
    let gib = b as f64 / 1024.0 / 1024.0 / 1024.0;
    format!("{gib:.1} GiB")
}

/// SigmaOS targets kernel >= 6.1 LTS.
fn kernel_supported(release: &str) -> bool {
    release
        .split('.')
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .map(|major| major >= 6)
        .unwrap_or(false)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let cli = Cli::parse();

    if let Some(feat) = cli.cpu_feature {
        println!("{}", if cpu_feature(&feat) { "yes" } else { "no" });
        return Ok(());
    }

    let info = detect()?;
    if cli.validate {
        std::process::exit(if validate(&info) { 0 } else { 1 });
    }
    if cli.json {
        println!("{}", serde_json::to_string_pretty(&info)?);
    } else {
        println!("OS:        {} {}", info.os_name, info.os_version);
        println!("Kernel:    {}", info.kernel_release);
        println!("Arch:      {}", info.arch);
        println!("CPU:       {} ({} cores)", info.cpu_model, info.cpu_cores);
        println!("Memory:    {}", memory_human(&info));
        println!("Session:   {}", if info.is_wayland { "wayland" } else { "x11" });
        println!("Init:      {}", info.init_system);
        println!("Hostname:  {}", info.hostname);
        println!("Uptime:    {}s", info.uptime_seconds);
    }
    Ok(())
}
