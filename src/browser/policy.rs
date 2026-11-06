//! Three Chromium policies supported by Brave. No personal profile or Shields key is written.
use crate::{
    Result, fail,
    state::{Store, atomic_json, read_json},
};
use serde::{Deserialize, Serialize};
const NAMES: [&str; 3] = [
    "DefaultSearchProviderEnabled",
    "DefaultSearchProviderName",
    "DefaultSearchProviderSearchURL",
];
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Value {
    Dword(u32),
    Text(String),
}
fn desired() -> [Option<Value>; 3] {
    [
        Some(Value::Dword(1)),
        Some(Value::Text("Google".into())),
        Some(Value::Text(
            "https://www.google.com/search?q={searchTerms}".into(),
        )),
    ]
}
pub trait Registry {
    fn read(&self, machine: bool, name: &str) -> Result<Option<Value>>;
    fn write(&mut self, name: &str, value: &Option<Value>) -> Result<()>;
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Backup {
    schema_version: u32,
    original: [Option<Value>; 3],
}
fn snapshot(reg: &impl Registry) -> Result<[Option<Value>; 3]> {
    Ok([
        reg.read(false, NAMES[0])?,
        reg.read(false, NAMES[1])?,
        reg.read(false, NAMES[2])?,
    ])
}
pub fn apply_with(reg: &mut impl Registry, store: &Store) -> Result<()> {
    for name in NAMES {
        if reg.read(true, name)?.is_some() {
            return Err(fail(
                "Há uma política de pesquisa da máquina. Configuração do Brave bloqueada para preservar a administração existente.",
            ));
        }
    }
    let path = store.root.join("backups/brave-search.json");
    let before = snapshot(reg)?;
    let target = desired();
    let backup = if path.exists() {
        let backup: Backup = read_json(&path)?;
        if backup.schema_version != 1 {
            return Err(fail("Backup de pesquisa não suportado"));
        }
        backup
    } else {
        if before
            .iter()
            .zip(&target)
            .any(|(a, b)| a.is_some() && a != b)
        {
            return Err(fail(
                "O Brave já possui outra política de pesquisa. Nenhuma política foi sobrescrita.",
            ));
        }
        let backup = Backup {
            schema_version: 1,
            original: before.clone(),
        };
        atomic_json(&path, &backup)?;
        backup
    };
    if before
        .iter()
        .zip(&backup.original)
        .zip(&target)
        .any(|((a, old), new)| a != old && a != new)
    {
        return Err(fail(
            "A política foi alterada externamente. Revise antes de reaplicar.",
        ));
    }
    for (name, value) in NAMES.iter().zip(&target) {
        reg.write(name, value)?;
    }
    if snapshot(reg)? != target {
        return Err(fail(
            "Política de pesquisa não confirmada; backup preservado para recuperação.",
        ));
    }
    Ok(())
}
pub fn rollback_with(reg: &mut impl Registry, store: &Store) -> Result<()> {
    let path = store.root.join("backups/brave-search.json");
    let backup: Backup = read_json(&path)?;
    if backup.schema_version != 1 {
        return Err(fail("Backup de pesquisa não suportado"));
    }
    let current = snapshot(reg)?;
    if current
        .iter()
        .zip(&backup.original)
        .zip(desired())
        .any(|((a, old), new)| a != old && *a != new)
    {
        return Err(fail(
            "A pesquisa foi alterada por outro programa. Reversão bloqueada para preservar a alteração.",
        ));
    }
    for (name, value) in NAMES.iter().zip(&backup.original) {
        reg.write(name, value)?;
    }
    if snapshot(reg)? != backup.original {
        return Err(fail("Reversão não confirmada; backup preservado."));
    }
    std::fs::remove_file(path)?;
    Ok(())
}
pub struct NativeRegistry;
#[cfg(windows)]
impl Registry for NativeRegistry {
    fn read(&self, machine: bool, name: &str) -> Result<Option<Value>> {
        use winreg::{RegKey, enums::*};
        let hive = if machine {
            HKEY_LOCAL_MACHINE
        } else {
            HKEY_CURRENT_USER
        };
        let key = match RegKey::predef(hive).open_subkey_with_flags(
            "Software\\Policies\\BraveSoftware\\Brave",
            KEY_READ | KEY_WOW64_64KEY,
        ) {
            Ok(k) => k,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let raw = match key.get_raw_value(name) {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        Ok(Some(match raw.vtype {
            REG_DWORD => Value::Dword(key.get_value(name)?),
            REG_SZ => Value::Text(key.get_value(name)?),
            _ => return Err(fail("Tipo de política inesperado; alteração bloqueada")),
        }))
    }
    fn write(&mut self, name: &str, value: &Option<Value>) -> Result<()> {
        use winreg::{RegKey, enums::*};
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey_with_flags(
            "Software\\Policies\\BraveSoftware\\Brave",
            KEY_READ | KEY_WRITE | KEY_WOW64_64KEY,
        )?;
        match value {
            Some(Value::Dword(v)) => key.set_value(name, v)?,
            Some(Value::Text(v)) => key.set_value(name, v)?,
            None => match key.delete_value(name) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            },
        }
        Ok(())
    }
}
#[cfg(not(windows))]
impl Registry for NativeRegistry {
    fn read(&self, _: bool, _: &str) -> Result<Option<Value>> {
        Err(fail("Requer Windows"))
    }
    fn write(&mut self, _: &str, _: &Option<Value>) -> Result<()> {
        Err(fail("Requer Windows"))
    }
}
