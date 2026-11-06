//! Read-only: validates reviewed downloaded files without launching installers.
use ruhtweaks::{installer::handoff::Kind, security};
fn main() -> ruhtweaks::Result<()> {
    for (kind, name) in [
        (Kind::Litecord, "work/Litecord-Setup-x64.exe"),
        (Kind::Atlas, "work/AME-Beta-v0.8.4.exe"),
    ] {
        let path = std::path::Path::new(name);
        security::verify_pe_header(path)?;
        let hash = security::digest_file(path)?;
        if hash != security::validate_digest(kind.digest())? {
            return Err(ruhtweaks::fail("Arquivo inspecionado difere da revisão"));
        }
        println!(
            "{} {}: hash e formato PE conferidos; assinatura {:?}; nenhum arquivo executado.",
            kind.key(),
            kind.tag(),
            security::inspect_signature(path)?
        );
    }
    Ok(())
}
