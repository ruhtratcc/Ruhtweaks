use crate::{
    Result, fail,
    state::{atomic_json, read_json},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
const LIMIT: u64 = 1024 * 1024 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Transfer {
    schema_version: u32,
    digest: String,
    partial: String,
}
pub fn recover(parent: &Path, digest: &str) -> Result<()> {
    let ledger = parent.join(".ruhtweaks-transfer.json");
    if !ledger.exists() {
        return Ok(());
    }
    let transfer: Transfer = read_json(&ledger)?;
    if transfer.schema_version != 1
        || transfer.digest != digest
        || !transfer.partial.starts_with("ruhtweaks-")
        || !transfer.partial.ends_with(".part")
        || !transfer
            .partial
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b))
    {
        return Err(fail(
            "Recibo de transferência inválido; nenhum arquivo será apagado",
        ));
    }
    let partial = parent.join(transfer.partial);
    if partial.exists() {
        let canonical = partial.canonicalize()?;
        if !canonical.starts_with(parent.canonicalize()?)
            || partial.symlink_metadata()?.file_type().is_symlink()
        {
            return Err(fail("Temporário redirecionado; limpeza recusada"));
        }
        std::fs::remove_file(partial)?;
    }
    std::fs::remove_file(ledger)?;
    Ok(())
}
pub fn copy_verified(
    input: &mut impl Read,
    destination: &Path,
    digest: &str,
    total: Option<u64>,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, Option<u64>),
) -> Result<()> {
    let expected = super::validate_digest(digest)?;
    if total.is_some_and(|n| n > LIMIT) {
        return Err(fail("Download excede 1 GiB"));
    }
    let parent = destination
        .parent()
        .ok_or_else(|| fail("Destino sem diretório"))?;
    std::fs::create_dir_all(parent)?;
    recover(parent, digest)?;
    let mut temporary = tempfile::Builder::new()
        .prefix("ruhtweaks-")
        .suffix(".part")
        .tempfile_in(parent)?;
    let partial = temporary
        .path()
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| fail("Nome temporário inválido"))?
        .to_owned();
    atomic_json(
        &parent.join(".ruhtweaks-transfer.json"),
        &Transfer {
            schema_version: 1,
            digest: digest.into(),
            partial,
        },
    )?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    let mut done = 0u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(fail("Download cancelado; destino final não publicado"));
        }
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        done += n as u64;
        if done > LIMIT {
            return Err(fail("Download excede limite"));
        }
        temporary.write_all(&buffer[..n])?;
        hash.update(&buffer[..n]);
        progress(done, total);
    }
    progress(done, total);
    if format!("{:x}", hash.finalize()) != expected.to_ascii_lowercase() {
        return Err(fail("SHA-256 divergente; arquivo final não publicado"));
    }
    temporary.as_file().sync_all()?;
    temporary.persist(destination).map_err(|e| e.error)?;
    std::fs::remove_file(parent.join(".ruhtweaks-transfer.json"))?;
    Ok(())
}
