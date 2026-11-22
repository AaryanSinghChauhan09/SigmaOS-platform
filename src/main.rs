//! SigmaOS component binary: platform

use sigmaos_platform::Component;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Component::new().operate()?;
    println!("sigmaos-platform: ok");
    Ok(())
}
