use crate::{
    Result, fail,
    state::{Store, atomic_json, read_json},
};
use serde::{Deserialize, Serialize};
pub const BALANCED: u128 = 0x381b4222_f694_41f0_9685_ff5bb260df2e;
pub trait PowerControl {
    fn current(&self) -> Result<u128>;
    fn set(&mut self, scheme: u128) -> Result<()>;
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Backup {
    schema_version: u32,
    original: String,
}
pub fn apply_with(api: &mut impl PowerControl, store: &Store) -> Result<()> {
    let path = store.root.join("backups/balanced-power.json");
    let current = api.current()?;
    if path.exists() {
        let original = original(&read_json(&path)?)?;
        if current != original && current != BALANCED {
            return Err(fail(
                "O plano de energia mudou externamente. Revise antes de reaplicar.",
            ));
        }
    } else {
        atomic_json(
            &path,
            &Backup {
                schema_version: 1,
                original: format!("{current:032x}"),
            },
        )?;
    }
    api.set(BALANCED)?;
    if api.current()? != BALANCED {
        return Err(fail("Plano equilibrado não confirmado. Backup preservado."));
    }
    Ok(())
}
fn original(backup: &Backup) -> Result<u128> {
    if backup.schema_version != 1 || backup.original.len() != 32 {
        return Err(fail("Backup de energia inválido"));
    }
    u128::from_str_radix(&backup.original, 16).map_err(|_| fail("GUID de energia inválido"))
}
pub fn rollback_with(api: &mut impl PowerControl, store: &Store) -> Result<()> {
    let path = store.root.join("backups/balanced-power.json");
    let backup: Backup = read_json(&path)?;
    let original = original(&backup)?;
    let current = api.current()?;
    if current != BALANCED && current != original {
        return Err(fail(
            "Energia alterada externamente. Reversão bloqueada para preservar a escolha.",
        ));
    }
    api.set(original)?;
    if api.current()? != original {
        return Err(fail(
            "Reversão de energia não confirmada; backup preservado.",
        ));
    }
    std::fs::remove_file(path)?;
    Ok(())
}
pub struct NativePower;
#[cfg(windows)]
impl PowerControl for NativePower {
    fn current(&self) -> Result<u128> {
        use windows_sys::Win32::{Foundation::LocalFree, System::Power::PowerGetActiveScheme};
        let mut ptr = std::ptr::null_mut();
        // SAFETY: Windows allocates a GUID, read before releasing with LocalFree.
        unsafe {
            let status = PowerGetActiveScheme(std::ptr::null_mut(), &mut ptr);
            if status != 0 {
                return Err(fail(format!("Consulta de energia falhou: {status}")));
            }
            if ptr.is_null() {
                return Err(fail("Plano de energia ausente"));
            }
            let guid = *ptr;
            LocalFree(ptr.cast());
            Ok(((guid.data1 as u128) << 96)
                | ((guid.data2 as u128) << 80)
                | ((guid.data3 as u128) << 64)
                | u64::from_be_bytes(guid.data4) as u128)
        }
    }
    fn set(&mut self, scheme: u128) -> Result<()> {
        let guid = windows_sys::core::GUID::from_u128(scheme);
        // SAFETY: pointer to a live GUID. The API fails if the scheme is absent
        // or managed; no fallback creates or edits another plan.
        let status = unsafe {
            windows_sys::Win32::System::Power::PowerSetActiveScheme(std::ptr::null_mut(), &guid)
        };
        if status != 0 {
            return Err(fail(format!(
                "Windows recusou o plano de energia ({status}); nenhuma política será contornada."
            )));
        }
        Ok(())
    }
}
#[cfg(not(windows))]
impl PowerControl for NativePower {
    fn current(&self) -> Result<u128> {
        Err(fail("Requer Windows"))
    }
    fn set(&mut self, _: u128) -> Result<()> {
        Err(fail("Requer Windows"))
    }
}
