use super::*;
// Read-only compatibility with old files. The retired extension has no runtime feature.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OldProfile {
    schema_version: u32,
    browser: Option<Browser>,
    apps: Vec<String>,
    atlas: bool,
    drivers: bool,
    google_search: bool,
    #[serde(rename = "ublock")]
    retired_extension: bool,
    tweaks: Vec<String>,
    accept_terms: bool,
    allow_startup: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prefer_silent: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    updates: Option<Vec<UpdateRequest>>,
}
pub(super) fn migrate(value: serde_json::Value) -> Result<Profile> {
    let old: OldProfile = serde_json::from_value(value)?;
    if !(1..=3).contains(&old.schema_version) {
        return Err(fail("Perfil antigo inválido"));
    }
    if old.schema_version == 1 && (old.prefer_silent.is_some() || old.updates.is_some()) {
        return Err(fail("Campos incompatíveis com o perfil v1"));
    }
    let profile = Profile {
        browser: old.browser,
        apps: old.apps,
        atlas: old.atlas,
        atlas_legacy: false,
        drivers: old.drivers,
        google_search: old.google_search,
        tweaks: old.tweaks,
        accept_terms: old.accept_terms,
        allow_startup: old.allow_startup,
        prefer_silent: old.prefer_silent.unwrap_or(false),
        updates: old.updates.unwrap_or_default(),
        schema_version: SCHEMA,
    };
    profile.validate()?;
    Ok(profile)
}
pub(super) fn matches(store: &Store, schema: u32, hash: &str) -> Result<bool> {
    let backup = store.root.join(format!("backups/profile-v{schema}.json"));
    let path = if backup.exists() {
        backup
    } else {
        store.root.join("profile.json")
    };
    let value: serde_json::Value = read_json(&path)?;
    let mut old: OldProfile = match serde_json::from_value(value) {
        Ok(v) => v,
        Err(_) => return Ok(false),
    };
    old.schema_version = schema;
    if schema == 1 {
        old.prefer_silent = None;
        old.updates = None;
    }
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(&old)?)) == hash)
}
