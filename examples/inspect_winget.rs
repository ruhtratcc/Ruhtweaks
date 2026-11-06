//! Read-only diagnostics. Exported inventory stays in work/, never printed in full.
use ruhtweaks::security;
use std::{fs, process::Command};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all("work")?;
    let amd = std::path::Path::new("work/amd-autodetect-source-inspection.exe");
    if amd.exists() {
        println!(
            "AMD trusted signer: {}",
            security::verify_publisher(amd, &["Advanced Micro Devices"])?
        );
    }
    let exe = security::winget_path()?;
    security::verify_signature(&exe)?;
    let file = std::env::current_dir()?.join("work/winget-export.json");
    let result = Command::new(&exe)
        .args([
            "export",
            "--output",
            file.to_str().ok_or("path")?,
            "--source",
            "winget",
            "--include-versions",
            "--disable-interactivity",
        ])
        .output()?;
    fs::write("work/winget-export-log.txt", &result.stdout)?;
    println!("Export: {}", result.status);
    if file.exists() {
        let json: serde_json::Value = serde_json::from_slice(&fs::read(file)?)?;
        if let Some(sources) = json["Sources"].as_array() {
            for source in sources {
                println!("Source: {}", source["SourceDetails"]);
                if let Some(packages) = source["Packages"].as_array() {
                    for app in packages
                        .iter()
                        .filter(|app| app["PackageIdentifier"] == "Brave.Brave")
                    {
                        println!("Brave: {app}");
                    }
                }
            }
        }
    }
    let result = Command::new(&exe)
        .args([
            "show",
            "--id",
            "Brave.Brave",
            "--exact",
            "--source",
            "winget",
            "--versions",
            "--disable-interactivity",
        ])
        .output()?;
    fs::write("work/winget-versions-brave.txt", &result.stdout)?;
    println!("Show: {}", result.status);
    Ok(())
}
