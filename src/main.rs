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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let cli = Cli::parse();

    if let Some(feat) = cli.cpu_feature {
        println!("{}", if cpu_feature(&feat) { "yes" } else { "no" });
        return Ok(());
    }

    let info = detect()?;
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
