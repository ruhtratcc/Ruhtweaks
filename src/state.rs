use crate::{Result, catalog::Browser, fail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

mod legacy;
pub const SCHEMA: u32 = 4;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateRequest {
    pub key: String,
    pub from: String,
    pub to: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema_version: u32,
    pub browser: Option<Browser>,
    pub apps: Vec<String>,
    pub atlas: bool,
    pub atlas_legacy: bool,
    pub drivers: bool,
    pub google_search: bool,
    pub tweaks: Vec<String>,
    pub accept_terms: bool,
    pub allow_startup: bool,
    pub prefer_silent: bool,
    pub updates: Vec<UpdateRequest>,
}
impl Default for Profile {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA,
            browser: Some(Browser::Brave),
            apps: vec![
                "litecord".into(),
                "steam".into(),
                "epic".into(),
                "riot".into(),
            ],
            atlas: false,
            atlas_legacy: false,
            drivers: false,
            google_search: false,
            tweaks: vec![],
            accept_terms: false,
            allow_startup: false,
            prefer_silent: true,
            updates: vec![],
        }
    }
}
impl Profile {
    pub fn standalone_browser(&self) -> Option<Browser> {
        // Keep the preference for opting out of Atlas without managing its browser.
        if self.atlas { None } else { self.browser }
    }
    pub fn include_automatic_setup(&mut self, info: &crate::system_info::SystemInfo) {
        if !info.supported() {
            return;
        }
        self.drivers |= info.gpus.iter().any(|gpu| {
            gpu.hardware_id
                .to_ascii_uppercase()
                .starts_with("PCI\\VEN_1002&")
        });
        self.google_search |= self.standalone_browser() == Some(Browser::Brave);
        if !self.tweaks.iter().any(|key| key == "extensions") {
            self.tweaks.push("extensions".into());
        }
    }
    pub fn suggested_for(info: &crate::system_info::SystemInfo) -> Self {
        let modern = crate::atlas::compatibility(info).is_ok();
        let legacy = !modern && crate::atlas::legacy::compatibility(info).is_ok();
        Self {
            atlas: modern || legacy,
            atlas_legacy: legacy,
            ..Self::default()
        }
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != SCHEMA {
            return Err(fail(
                "Versão do perfil não suportada. O arquivo não foi sobrescrito.",
            ));
        }
        for key in &self.apps {
            let app = crate::catalog::find(key)
                .ok_or_else(|| fail(format!("Aplicativo desconhecido: {key}")))?;
            if app.browser {
                return Err(fail("Selecione o navegador no campo próprio."));
            }
        }
        for key in &self.tweaks {
            if crate::tweaks::find(key).is_none() {
                return Err(fail(format!("Ajuste desconhecido: {key}")));
            }
        }
        let mut seen = std::collections::HashSet::new();
        for update in &self.updates {
            let app = crate::catalog::find(&update.key)
                .ok_or_else(|| fail("Aplicativo de atualização desconhecido"))?;
            if app.winget.is_none()
                || !crate::catalog::supports_updates(&update.key)
                || !seen.insert(&update.key)
                || !(self.apps.contains(&update.key)
                    || self.browser.is_some_and(|b| b.key() == update.key))
                || update.from == update.to
                || !valid_version(&update.from)
                || !valid_version(&update.to)
            {
                return Err(fail(
                    "Atualização inválida. Selecione o app e consulte as versões novamente.",
                ));
            }
        }
        if self.atlas_legacy && !self.atlas {
            return Err(fail("Selecione Atlas para usar a versão legada."));
        }
        Ok(())
    }
    pub fn fingerprint(&self) -> Result<String> {
        Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(self)?)))
    }
}
pub fn decode_profile(value: serde_json::Value) -> Result<Profile> {
    if matches!(value["schema_version"].as_u64(), Some(1..=3)) {
        return legacy::migrate(value);
    }
    let profile: Profile = serde_json::from_value(value)?;
    profile.validate()?;
    Ok(profile)
}
pub fn valid_version(version: &str) -> bool {
    !version.is_empty()
        && version.len() <= 128
        && version.as_bytes()[0].is_ascii_digit()
        && version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-_+".contains(&b))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pending,
    Downloading,
    Verifying,
    Installing,
    Completed,
    Failed,
    RequiresAction,
    Cancelled,
    Preparing,
    Executing,
    RequiresConfirmation,
    ExternalAction,
    AwaitingReboot,
    AwaitingVerification,
}
impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "Pendente",
            Self::Downloading => "Baixando",
            Self::Verifying => "Verificando",
            Self::Installing => "Instalando",
            Self::Completed => "Concluído",
            Self::Failed => "Falhou",
            Self::RequiresAction => "Requer ação",
            Self::Cancelled => "Cancelado",
            Self::Preparing => "Preparando",
            Self::Executing => "Executando",
            Self::RequiresConfirmation => "Requer confirmação",
            Self::ExternalAction => "Requer intervenção externa",
            Self::AwaitingReboot => "Aguardando reinicialização",
            Self::AwaitingVerification => "Aguardando verificação",
        }
    }
    pub fn active(self) -> bool {
        matches!(
            self,
            Self::Downloading
                | Self::Verifying
                | Self::Installing
                | Self::Preparing
                | Self::Executing
        )
    }
    pub fn needs_action(self) -> bool {
        matches!(
            self,
            Self::RequiresAction
                | Self::RequiresConfirmation
                | Self::ExternalAction
                | Self::AwaitingReboot
                | Self::AwaitingVerification
        )
    }
    pub fn permits(self, next: Self) -> bool {
        if self == Self::ExternalAction {
            return matches!(
                next,
                Self::Pending
                    | Self::Verifying
                    | Self::Completed
                    | Self::Failed
                    | Self::Cancelled
                    | Self::RequiresAction
                    | Self::AwaitingReboot
                    | Self::AwaitingVerification
            );
        }
        if matches!(self, Self::Verifying | Self::Downloading | Self::Preparing) {
            return next != Self::Pending && next != Self::Verifying
                || self != Self::Verifying && next == Self::Verifying;
        }
        if self == Self::Executing {
            return matches!(
                next,
                Self::Completed
                    | Self::Failed
                    | Self::Cancelled
                    | Self::RequiresAction
                    | Self::AwaitingReboot
                    | Self::AwaitingVerification
                    | Self::ExternalAction
            );
        }
        if self.needs_action() {
            return matches!(next, Self::Pending | Self::Verifying | Self::Cancelled);
        }
        matches!(
            (self, next),
            (
                Self::Pending,
                Self::Verifying | Self::Cancelled | Self::RequiresAction
            ) | (
                Self::Verifying,
                Self::Downloading
                    | Self::Installing
                    | Self::Completed
                    | Self::RequiresAction
                    | Self::Failed
                    | Self::Cancelled
            ) | (
                Self::Downloading,
                Self::Verifying | Self::Failed | Self::Cancelled
            ) | (
                Self::Installing,
                Self::Completed
                    | Self::RequiresAction
                    | Self::Failed
                    | Self::RequiresConfirmation
                    | Self::ExternalAction
                    | Self::AwaitingReboot
                    | Self::AwaitingVerification
                    | Self::Cancelled
            ) | (
                Self::Failed | Self::RequiresAction | Self::Cancelled,
                Self::Pending
            )
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Install(String),
    Upgrade(UpdateRequest),
    Atlas,
    Drivers,
    Browser,
    Tweak(String),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub action: Action,
    pub status: Status,
    pub detail: String,
    pub admin: String,
}
impl Task {
    pub fn transition(&mut self, next: Status, detail: impl Into<String>) -> Result<()> {
        if !self.status.permits(next) {
            return Err(fail(format!(
                "Transição inválida: {:?} -> {:?}",
                self.status, next
            )));
        }
        self.status = next;
        self.detail = detail.into();
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Journal {
    pub schema_version: u32,
    pub profile_hash: String,
    pub tasks: Vec<Task>,
}
impl Journal {
    pub fn recover(&mut self) {
        for task in &mut self.tasks {
            if task.status.active() {
                task.status = Status::RequiresAction;
                task.detail="Execução interrompida. Revalidar antes de repetir; nenhuma instalação será repetida automaticamente.".into();
            }
        }
    }
    pub fn report(&self) -> String {
        self.tasks
            .iter()
            .map(|t| format!("{} | {}\n{}\n", t.status.label(), t.title, t.detail))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
#[derive(Clone)]
pub struct Store {
    pub root: PathBuf,
}
impl Store {
    pub fn default_location() -> Result<Self> {
        #[cfg(windows)]
        let root = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| fail("LOCALAPPDATA indisponível"))?
            .join("Ruhtweaks");
        #[cfg(not(windows))]
        let root = std::env::current_dir()?.join("work/ruhtweaks-data");
        Ok(Self { root })
    }
    pub fn lock(&self) -> Result<File> {
        fs::create_dir_all(&self.root)?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.root.join("instance.lock"))?;
        fs2::FileExt::try_lock_exclusive(&file)
            .map_err(|_| fail("Outra instância do Ruhtweaks está usando estes dados."))?;
        Ok(file)
    }
    pub fn load_profile(&self) -> Result<Profile> {
        let path = self.root.join("profile.json");
        if !path.exists() {
            return Ok(Profile::default());
        }
        let value: serde_json::Value = read_json(&path)?;
        if matches!(value["schema_version"].as_u64(), Some(1..=3)) {
            // Preserve the complete old document before the first v2 save.
            let backup = self
                .root
                .join(format!("backups/profile-v{}.json", value["schema_version"]));
            if !backup.exists() {
                atomic_json(&backup, &value)?;
            }
        }
        decode_profile(value)
    }
    pub fn save_profile(&self, profile: &Profile) -> Result<()> {
        profile.validate()?;
        atomic_json(&self.root.join("profile.json"), profile)
    }
    pub fn load_journal(&self) -> Result<Option<Journal>> {
        let path = self.root.join("journal.json");
        if !path.exists() {
            return Ok(None);
        }
        let mut j: Journal = read_json(&path)?;
        if matches!(j.schema_version, 1..=3) {
            let old_schema = j.schema_version;
            let backup = self
                .root
                .join(format!("backups/journal-v{old_schema}.json"));
            if !backup.exists() {
                atomic_json(&backup, &j)?;
            }
            j.schema_version = SCHEMA;
            let profile = self.load_profile()?;
            if legacy::matches(self, old_schema, &j.profile_hash)? {
                let mut migrated =
                    crate::installer::plan(&profile, &crate::system_info::SystemInfo::default())?;
                for task in &mut migrated.tasks {
                    if let Some(old) = j
                        .tasks
                        .iter()
                        .find(|old| old.id == task.id && old.action == task.action)
                    {
                        *task = old.clone();
                        if task.status == Status::RequiresConfirmation
                            && matches!(&task.action, Action::Install(key) if key == "litecord")
                        {
                            task.status = Status::Pending;
                            task.detail =
                                "Preparado anteriormente. Exige aprovação do plano nesta sessão."
                                    .into();
                        }
                    }
                }
                j = migrated;
            }
        }
        if j.schema_version != SCHEMA {
            return Err(fail("Journal de versão incompatível"));
        }
        j.recover();
        Ok(Some(j))
    }
    pub fn save_journal(&self, journal: &Journal) -> Result<()> {
        let current = self.root.join("journal.json");
        if current.exists() {
            let previous: Journal = read_json(&current)?;
            if previous.profile_hash != journal.profile_hash {
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| fail(e.to_string()))?
                    .as_nanos();
                atomic_json(
                    &self.root.join("history").join(format!("{stamp}.json")),
                    &previous,
                )?;
            }
        }
        atomic_json(&self.root.join("journal.json"), journal)?;
        atomic_write(&self.root.join("report.txt"), journal.report().as_bytes())
    }
}
pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    use std::io::Read;
    const LIMIT: u64 = 16 * 1024 * 1024;
    let mut bytes = Vec::new();
    File::open(path)?.take(LIMIT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > LIMIT {
        return Err(fail(
            "Documento excede o limite de 16 MiB; não foi sobrescrito",
        ));
    }
    Ok(serde_json::from_slice(&bytes)?)
}
pub fn atomic_json(path: &Path, value: &impl Serialize) -> Result<()> {
    atomic_write(path, &serde_json::to_vec_pretty(value)?)
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| fail("Caminho sem diretório"))?;
    fs::create_dir_all(parent)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn v1_queue_preserves_verified_completion_without_new_consent() {
        let original = r#"{"schema_version":1,"browser":"brave","apps":["litecord","steam","epic","riot"],"atlas":false,"drivers":false,"google_search":false,"ublock":false,"tweaks":[],"accept_terms":false,"allow_startup":false}"#;
        let temp = tempfile::tempdir().unwrap();
        let store = Store {
            root: temp.path().into(),
        };
        std::fs::write(store.root.join("profile.json"), original).unwrap();
        let profile = store.load_profile().unwrap();
        let legacy_hash = format!("{:x}", Sha256::digest(original.as_bytes()));

        let mut journal =
            crate::installer::plan(&profile, &crate::system_info::SystemInfo::default()).unwrap();
        journal.schema_version = 1;
        journal.profile_hash = legacy_hash;
        journal.tasks[0].status = Status::Completed;
        atomic_json(&store.root.join("journal.json"), &journal).unwrap();
        let migrated = store.load_journal().unwrap().unwrap();
        assert_eq!(migrated.profile_hash, profile.fingerprint().unwrap());
        let reviewed = crate::installer::review_plan(
            &profile,
            &crate::system_info::SystemInfo::default(),
            Some(&migrated),
        )
        .unwrap();
        assert_eq!(reviewed.tasks[0].status, Status::Completed);
        assert!(store.root.join("backups/journal-v1.json").exists());
    }
    #[test]
    fn invalid_transition_does_not_mutate_task() {
        let mut task = Task {
            id: "app:steam".into(),
            title: "Steam".into(),
            action: Action::Install("steam".into()),
            status: Status::Pending,
            detail: "original".into(),
            admin: "UAC".into(),
        };
        assert!(task.transition(Status::Completed, "invalido").is_err());
        assert_eq!(task.status, Status::Pending);
        assert_eq!(task.detail, "original");
    }
    #[test]
    fn profile_rejects_unknown_fields_without_discarding_settings() {
        let mut value = serde_json::to_value(Profile::default()).unwrap();
        value["run_command"] = serde_json::json!("malicious");
        assert!(serde_json::from_value::<Profile>(value).is_err());
    }
}
