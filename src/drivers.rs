use crate::system_info::{Gpu, SystemInfo};
pub mod amd;
pub mod amd_update;
pub fn prepare(
    info: &SystemInfo,
    store: &crate::state::Store,
    cancel: &std::sync::atomic::AtomicBool,
    log: &mut dyn FnMut(String),
) -> crate::Result<(crate::state::Status, String)> {
    let mut detail = description(info);
    if info
        .gpus
        .iter()
        .any(|gpu| gpu.hardware_id.to_ascii_uppercase().contains("VEN_1002"))
        && info.architecture.starts_with("x86_64")
        && info.build >= 17763
        && !info.laptop
    {
        let path = amd::prepare(store, cancel, log)?;
        detail = format!(
            "AMD Auto-Detect obtido da página oficial; assinatura e editor AMD verificados. Hash calculado registrado (a página não publica hash de referência).\nArquivo pronto: {}\nUse o botão da GPU para abrir a pasta e iniciar o instalador. Escolha Recommended e Driver Only/Minimal quando disponíveis; evite Factory Reset. O fabricante ainda precisa validar cada GPU, inclusive modelos legados.\n\n{detail}",
            path.display()
        );
    }
    Ok((crate::state::Status::RequiresAction, detail))
}
pub fn official_url(gpu: &Gpu) -> Option<&'static str> {
    let id = gpu.hardware_id.to_ascii_uppercase();
    if id.contains("VEN_1002") {
        Some("https://www.amd.com/en/support/download/drivers.html")
    } else if id.contains("VEN_10DE") {
        Some("https://www.nvidia.com/en-us/drivers/")
    } else if id.contains("VEN_8086") {
        Some("https://www.intel.com/content/www/us/en/download-center/home.html")
    } else {
        None
    }
}
pub fn description(info: &SystemInfo) -> String {
    format!(
        "{}\n\nAMD: instalação automática de drivers Display atribuídos ao hardware pelo Windows Update, com UAC e termos aprovados. Não instala Adrenalin/chipset. Sem pacote recomendado, permanece pendente; o Auto-Detect é uma alternativa assistida. Outros fabricantes: instalação assistida. Compare modelo, hardware ID, versão do Windows e recomendações do fabricante/OEM. Em notebooks, prefira primeiro o suporte do fabricante do computador.\n\nAMD: use o seletor ou Auto-Detect oficial; escolha Driver Only/Minimal quando oferecido.\nNVIDIA: use o seletor oficial; instalação personalizada permite revisar componentes.\nIntel: seletor de downloads oficial; DSA é opcional e pode instalar componentes residentes.\n\nNão instalamos BIOS, firmware ou ferramentas residentes automaticamente. Abrir a página não significa que o driver foi instalado.",
        info.gpus
            .iter()
            .map(|g| format!(
                "{}\n{}\nDriver detectado: {}\n{}",
                g.name,
                g.hardware_id,
                g.driver,
                official_url(g).unwrap_or("Fabricante não identificado; consulte o OEM.")
            ))
            .collect::<Vec<_>>()
            .join("\n\n")
    )
}
