//! Prepared files are facts persisted to disk. Permission to execute is session-only.
use crate::{
    Result, fail,
    security::{self, Signature},
    state::{Profile, Status, Store, atomic_json, read_json},
};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};
pub const LITE_TAG: &str = "v1.3.1";
pub const LITE_DIGEST: &str =
    "sha256:3628b3610c933c6bfdfe766d9882f2c8d7747f3f3e89e2078a34d978130ec012";
pub const AME_TAG: &str = "0.8.4";
pub const AME_DIGEST: &str =
    "sha256:a637f32b64e1493b315db9139065105656c84518dca20ce3b18c89750ee40c82";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Litecord,
    Atlas,
}
impl Kind {
    pub fn key(self) -> &'static str {
        match self {
            Self::Litecord => "litecord",
            Self::Atlas => "atlas",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Litecord => "Litecord-Setup-x64.exe",
            Self::Atlas => "AME-Beta-v0.8.4.exe",
        }
    }
    pub fn tag(self) -> &'static str {
        match self {
            Self::Litecord => LITE_TAG,
            Self::Atlas => AME_TAG,
        }
    }
    pub fn repository(self) -> &'static str {
        match self {
            Self::Litecord => "Ak4ai/Litecord",
            Self::Atlas => "Ameliorated-LLC/trusted-uninstaller-cli",
        }
    }
    pub fn digest(self) -> &'static str {
        match self {
            Self::Litecord => LITE_DIGEST,
            Self::Atlas => AME_DIGEST,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prepared {
    pub schema_version: u32,
    pub kind: Kind,
    pub profile_hash: String,
    pub source: String,
    pub signature: Signature,
    pub started_ms: Option<u64>,
    pub boot_before: Option<String>,
    pub launch_attempted: bool,
    #[serde(default)]
    pub baseline_applied_ms: Option<u64>,
}
impl Prepared {
    pub fn path(&self, store: &Store) -> Result<PathBuf> {
        artifact_path(store, self.kind.name(), self.kind.digest())
    }
    pub fn save(&self, store: &Store) -> Result<()> {
        atomic_json(&receipt_path(store, self.kind), self)
    }
    pub fn validate(&self, profile: &Profile) -> Result<()> {
        if self.schema_version != 1 || self.profile_hash != profile.fingerprint()? {
            return Err(fail(
                "Arquivos pertencem a outro plano. Prepare e revise novamente.",
            ));
        }
        self.validate_source()
    }
    pub fn validate_source(&self) -> Result<()> {
        let expected = format!(
            "https://github.com/{}/releases/download/{}/{}",
            self.kind.repository(),
            self.kind.tag(),
            self.kind.name()
        );
        if self.source != expected {
            return Err(fail("Origem do arquivo preparado foi alterada"));
        }
        Ok(())
    }
    pub fn prompt(&self) -> String {
        let signature = match &self.signature { Signature::Valid(p) => format!("Authenticode válido: {p}"), Signature::Unsigned => "SEM assinatura digital. O hash confirma a publicação, não identifica criptograficamente o editor.".into(), Signature::Unverifiable(code) => format!("Assinatura inconclusiva/inválida: 0x{code:08X}. Abertura bloqueada.") };
        let extra = match self.kind {
            Kind::Litecord => {
                "Editor declarado: Litecord Project, fonte Ak4ai/Litecord. O instalador original configura fechamento de litecord.exe; os argumentos documentados impedem fechamento e reinício forçados. Salve seu trabalho. O cliente pode abrir ao terminar, inclusive no modo silencioso. Inicialização no Windows começa desmarcada."
            }
            Kind::Atlas => {
                "Fonte: Ameliorated-LLC. Confirme backup e leitura do guia de instalação limpa. AME abre sem argumentos; selecione o playbook preparado na ferramenta. Não há aplicação automática nem aprovação dos pedidos de desativar Defender/UCPD. Se forem exigidos, interrompa. A abertura do assistente não comprova instalação."
            }
        };
        format!(
            "{} {}\n{signature}\n{extra}\nSHA-256: {}\nFonte: {}",
            self.kind.key(),
            self.kind.tag(),
            self.kind.digest(),
            self.source
        )
    }
}
fn receipt_path(store: &Store, kind: Kind) -> PathBuf {
    store
        .root
        .join("prepared")
        .join(format!("{}.json", kind.key()))
}
pub fn load(store: &Store, kind: Kind) -> Result<Option<Prepared>> {
    let path = receipt_path(store, kind);
    if !path.exists() {
        return Ok(None);
    }
    let value: Prepared = read_json(&path)?;
    if value.kind != kind || value.schema_version != 1 {
        return Err(fail("Recibo de preparação inválido"));
    }
    Ok(Some(value))
}
pub fn artifact_path(store: &Store, name: &str, digest: &str) -> Result<PathBuf> {
    if ![
        Kind::Litecord.name(),
        Kind::Atlas.name(),
        "AtlasPlaybook_v0.5.0-hotfix.apbx",
    ]
    .contains(&name)
    {
        return Err(fail("Nome de artefato não revisado"));
    }
    Ok(store
        .root
        .join("downloads")
        .join(security::validate_digest(digest)?)
        .join(name))
}
pub fn prepare(
    kind: Kind,
    profile: &Profile,
    store: &Store,
    client: &reqwest::blocking::Client,
    cancel: &AtomicBool,
    report: &mut dyn FnMut(Status, String),
) -> Result<Prepared> {
    report(
        Status::Preparing,
        format!("Consultando release oficial {} {}", kind.key(), kind.tag()),
    );
    let release = crate::atlas::release_with(client, kind.repository())?;
    if release.tag_name != kind.tag() {
        return Err(fail(format!(
            "{} {} exige nova revisão do instalador; propriedades de {} não são presumidas.",
            kind.key(),
            release.tag_name,
            kind.tag()
        )));
    }
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == kind.name() && a.digest.as_deref() == Some(kind.digest()))
        .ok_or_else(|| fail("Artefato mudou desde a revisão. Preparação interrompida."))?;
    let dest = artifact_path(store, kind.name(), kind.digest())?;
    let mut published = std::time::Instant::now() - std::time::Duration::from_secs(1);
    security::download_with_client(
        client,
        &asset.browser_download_url,
        kind.digest(),
        &dest,
        cancel,
        |done, total| {
            if published.elapsed() >= std::time::Duration::from_millis(200) || total == Some(done) {
                report(
                    Status::Downloading,
                    format!("{}: {done} bytes / {total:?}", kind.key()),
                );
                published = std::time::Instant::now();
            }
        },
    )?;
    report(
        Status::Verifying,
        "Conferindo assinatura e metadados; o download não é instalação.".into(),
    );
    let prepared = Prepared {
        schema_version: 1,
        kind,
        profile_hash: profile.fingerprint()?,
        source: asset.browser_download_url.clone(),
        signature: security::inspect_signature(&dest)?,
        started_ms: None,
        boot_before: None,
        launch_attempted: false,
        baseline_applied_ms: None,
    };
    prepared.validate(profile)?;
    prepared.save(store)?;
    Ok(prepared)
}
#[derive(Clone, Debug)]
pub struct Consent {
    pub kind: Kind,
    pub profile_hash: String,
    pub digest: String,
    pub acknowledge: bool,
    pub silent: bool,
}
impl Consent {
    pub fn validate(&self, prepared: &Prepared, profile: &Profile) -> Result<()> {
        prepared.validate(profile)?;
        if !self.acknowledge
            || self.kind != prepared.kind
            || self.profile_hash != prepared.profile_hash
            || self.digest != prepared.kind.digest()
        {
            return Err(fail("Confirmação específica do arquivo e do plano ausente"));
        }
        if matches!(prepared.signature, Signature::Unverifiable(_)) {
            return Err(fail(
                "Verificação de assinatura inconclusiva: abrir bloqueado",
            ));
        }
        Ok(())
    }
}
pub fn parameters(kind: Kind, silent: bool) -> &'static str {
    match (kind, silent) {
        (Kind::Atlas, _) => "",
        (Kind::Litecord, true) => {
            "/SILENT /NORESTART /RESTARTEXITCODE=3010 /NOCLOSEAPPLICATIONS /NOFORCECLOSEAPPLICATIONS /NORESTARTAPPLICATIONS /MERGETASKS=!startupicon"
        }
        (Kind::Litecord, false) => {
            "/NORESTART /RESTARTEXITCODE=3010 /NOCLOSEAPPLICATIONS /NOFORCECLOSEAPPLICATIONS /NORESTARTAPPLICATIONS /MERGETASKS=!startupicon"
        }
    }
}
pub trait InstallBackend {
    fn launch(&mut self, file: &Prepared, parameters: &str) -> Result<Option<u32>>;
    fn litecord_present(&mut self) -> Result<bool>;
}
pub fn run_litecord(
    backend: &mut impl InstallBackend,
    prepared: &Prepared,
    consent: &Consent,
    profile: &Profile,
) -> Result<(Status, String)> {
    consent.validate(prepared, profile)?;
    if prepared.kind != Kind::Litecord {
        return Err(fail("Tipo de operação inválido"));
    }
    if backend.litecord_present()? {
        return Ok((
            Status::Completed,
            "Litecord existente confirmado; instalador não repetido.".into(),
        ));
    }
    match backend.launch(prepared, parameters(Kind::Litecord, consent.silent))? {
        Some(1223 | 2 | 5) => Ok((Status::Cancelled, "Abertura/instalação cancelada; arquivos preservados para retomada.".into())),
        Some(3010) => Ok((Status::AwaitingReboot, "Instalador solicitou reinício. Salve o trabalho e reinicie quando desejar; nenhum reboot foi comandado.".into())),
        Some(0) if backend.litecord_present()? => Ok((Status::Completed, "Processo terminou e produto Litecord foi confirmado pelo AppId, origem declarada e executável registrado.".into())),
        Some(0) | None => Ok((Status::AwaitingVerification, "Processo sem confirmação suficiente. Revalide a instalação; download e exit code não bastam.".into())),
        Some(code) => Err(fail(format!("Instalador Litecord retornou {code}; arquivos preservados."))),
    }
}
/// Delete one registered file only after durable verified completion. Never recurse.
pub fn cleanup_file(path: &Path, root: &Path, digest: &str, status: Status) -> Result<bool> {
    if status != Status::Completed || !path.exists() {
        return Ok(false);
    }
    let root = root.canonicalize()?;
    let canonical = path.canonicalize()?;
    if !canonical.starts_with(&root)
        || path.symlink_metadata()?.file_type().is_symlink()
        || security::digest_file(&canonical)? != security::validate_digest(digest)?
    {
        return Err(fail(
            "Limpeza recusada: arquivo fora da área própria ou hash alterado",
        ));
    }
    std::fs::remove_file(path)?;
    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir(parent);
    }
    Ok(true)
}
pub fn verify_atlas(file: &Prepared) -> (Status, String) {
    let result = (|| -> Result<(Status, String)> {
        let evidence = crate::atlas::verification::read()?;
        if evidence
            .as_ref()
            .is_some_and(|e| file.baseline_applied_ms == Some(e.applied_ms))
        {
            return Ok((Status::AwaitingVerification, "O registro AME continua igual ao observado antes da abertura. Não há conclusão nova confirmada.".into()));
        }
        let boot = crate::atlas::preflight::boot_id()?;
        Ok(crate::atlas::verification::evaluate(
            evidence.as_ref(),
            file.started_ms
                .ok_or_else(|| fail("Início da etapa não registrado"))?,
            file.boot_before
                .as_ref()
                .is_some_and(|before| before != &boot),
            crate::atlas::preflight::reboot_pending()?,
            crate::atlas::preflight::protections_active()?,
        ))
    })();
    result.unwrap_or_else(|e| (Status::AwaitingVerification, format!("Verificação Atlas inconclusiva: {e}. Arquivos mantidos; nenhuma operação será repetida automaticamente.")))
}
impl super::NativeExecutor {
    pub fn approve_litecord(&mut self, profile: &Profile) {
        self.automatic_litecord =
            profile.accept_terms && profile.apps.iter().any(|key| key == "litecord");
    }
    pub(super) fn prepared_flow(
        &mut self,
        kind: Kind,
        profile: &Profile,
        store: &Store,
        cancel: &AtomicBool,
        log: &mut dyn FnMut(String),
    ) -> Result<(Status, String)> {
        if !self.info.architecture.starts_with("x86_64") {
            return Ok((
                Status::RequiresAction,
                "Esta integração exige arquitetura x64 revisada.".into(),
            ));
        }
        if kind == Kind::Atlas {
            if let Err(reason) = crate::atlas::compatibility(&self.info) {
                return Ok((Status::RequiresAction, reason));
            }
            if crate::atlas::preflight::reboot_pending()? {
                return Ok((Status::AwaitingReboot, "Windows Update informa reinício pendente. Reinicie quando desejar e revise novamente; nenhum reboot será executado.".into()));
            }
            let free = crate::atlas::preflight::workspace_space(&store.root)?;
            if free < 512 * 1024 * 1024 {
                return Err(fail(
                    "Preparação exige 512 MiB livres na área própria para downloads. Isso não representa o espaço necessário a uma reinstalação do Windows.",
                ));
            }
            log(format!(
                "Área de trabalho: {free} bytes livres. Compatibilidade do Windows conferida; requisito de segurança do playbook continua sendo externo."
            ));
        }
        let consent = if kind == Kind::Litecord && self.automatic_litecord {
            Some(Consent {
                kind,
                profile_hash: profile.fingerprint()?,
                digest: LITE_DIGEST.into(),
                acknowledge: true,
                silent: true,
            })
        } else if self.confirmation.as_ref().is_some_and(|c| c.kind == kind) {
            self.confirmation.take()
        } else {
            None
        };
        let mut file = match load(store, kind)? {
            Some(mut existing)
                if existing.validate(profile).is_ok()
                    && existing.path(store)?.is_file()
                    && security::digest_file(&existing.path(store)?)?
                        == security::validate_digest(kind.digest())? =>
            {
                existing.signature = security::inspect_signature(&existing.path(store)?)?;
                existing.save(store)?;
                existing
            }
            _ => {
                let phase = self.phase.clone();
                prepare(
                    kind,
                    profile,
                    store,
                    &self.network_client()?,
                    cancel,
                    &mut |state, message| {
                        *phase.lock().unwrap_or_else(|e| e.into_inner()) = Some(state);
                        log(message);
                    },
                )?
            }
        };
        if kind == Kind::Atlas && !file.launch_attempted {
            let destination = artifact_path(
                store,
                "AtlasPlaybook_v0.5.0-hotfix.apbx",
                crate::atlas::REVIEWED_DIGEST,
            )?;
            if !destination.is_file()
                || security::digest_file(&destination)?
                    != security::validate_digest(crate::atlas::REVIEWED_DIGEST)?
            {
                let client = self.network_client()?;
                let release = crate::atlas::release_with(&client, "Atlas-OS/Atlas")?;
                let asset = crate::atlas::verified_playbook(&release)?;
                let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
                security::download_with_client(
                    &client,
                    &asset.browser_download_url,
                    crate::atlas::REVIEWED_DIGEST,
                    &destination,
                    cancel,
                    |done, total| {
                        if last.elapsed() >= std::time::Duration::from_millis(200)
                            || total == Some(done)
                        {
                            *self.phase.lock().unwrap_or_else(|e| e.into_inner()) =
                                Some(Status::Downloading);
                            log(format!("Playbook: {done} bytes / {total:?}"));
                            last = std::time::Instant::now();
                        }
                    },
                )?;
            }
            log(format!(
                "AME e playbook preparados. Playbook para selecionar no assistente: {}. Escolha defender-enable, mitigations-default e auto-updates-default; não selecione disable-core-isolation. Se o navegador escolhido for Brave, escolha browser-brave no AME.",
                destination.display()
            ));
        }
        if matches!(file.signature, Signature::Unverifiable(_)) {
            return Ok((Status::RequiresAction, file.prompt()));
        }
        let Some(consent) = consent else {
            if kind == Kind::Atlas && file.launch_attempted {
                return Ok(verify_atlas(&file));
            }
            return Ok((
                Status::RequiresConfirmation,
                format!("Arquivos preparados e hash conferido.\n{}", file.prompt()),
            ));
        };
        consent.validate(&file, profile)?;
        if kind == Kind::Atlas && !crate::atlas::preflight::ac_power()? {
            return Ok((Status::RequiresConfirmation, "Requisito oficial PluggedIn: conecte à alimentação e confirme novamente. Arquivos preparados e preservados; AME não foi aberto.".into()));
        }
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok((
                Status::Cancelled,
                "Cancelado antes de abrir o instalador.".into(),
            ));
        }
        if kind == Kind::Atlas {
            if file.launch_attempted {
                let verified = verify_atlas(&file);
                if verified.0 == Status::Completed {
                    return Ok(verified);
                }
            }
            if file.boot_before.is_none() {
                file.boot_before = Some(crate::atlas::preflight::boot_id()?);
            }
            if !file.launch_attempted {
                file.baseline_applied_ms =
                    crate::atlas::verification::read()?.map(|e| e.applied_ms);
            }
        }
        file.started_ms.get_or_insert(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| fail(e.to_string()))?
                .as_millis() as u64,
        );
        if kind == Kind::Litecord {
            run_litecord(
                &mut NativeRun {
                    store,
                    phase: self.phase.clone(),
                    log,
                },
                &file,
                &consent,
                profile,
            )
        } else {
            match (NativeRun {
                store,
                phase: self.phase.clone(),
                log,
            })
            .launch(&file, "")?
            {
                Some(1223) => Ok((
                    Status::Cancelled,
                    "Abertura do AME cancelada pelo Windows/usuário.".into(),
                )),
                Some(code) if code != 0 => Ok((
                    Status::AwaitingVerification,
                    format!(
                        "AME retornou {code}. Consulte seus logs; arquivos e estado preservados."
                    ),
                )),
                _ => Ok(verify_atlas(
                    &load(store, kind)?.ok_or_else(|| fail("Recibo AME ausente"))?,
                )),
            }
        }
    }
}
struct NativeRun<'a> {
    store: &'a Store,
    phase: std::sync::Arc<std::sync::Mutex<Option<Status>>>,
    log: &'a mut dyn FnMut(String),
}
impl InstallBackend for NativeRun<'_> {
    fn litecord_present(&mut self) -> Result<bool> {
        match super::products::detect("litecord")? {
            super::Detection::Present => Ok(true),
            super::Detection::Absent => Ok(false),
            super::Detection::Unknown(reason) => Err(fail(reason)),
        }
    }
    fn launch(&mut self, file: &Prepared, parameters: &str) -> Result<Option<u32>> {
        let fresh = security::inspect_signature(&file.path(self.store)?)?;
        if fresh != file.signature || matches!(fresh, Signature::Unverifiable(_)) {
            return Err(fail(
                "A condição da assinatura mudou; prepare e confirme novamente",
            ));
        }
        let phase = if file.kind == Kind::Atlas {
            Status::ExternalAction
        } else {
            Status::Executing
        };
        let mut journal: crate::state::Journal = read_json(&self.store.root.join("journal.json"))?;
        if journal.profile_hash != file.profile_hash {
            return Err(fail("Plano durável mudou antes da abertura"));
        }
        let task = journal
            .tasks
            .iter_mut()
            .find(|task| match &task.action {
                crate::state::Action::Atlas => file.kind == Kind::Atlas,
                crate::state::Action::Install(key) => {
                    file.kind == Kind::Litecord && key == "litecord"
                }
                _ => false,
            })
            .ok_or_else(|| fail("Tarefa durável ausente"))?;
        if task.status != phase {
            task.transition(
                phase,
                "Abertura confirmada; Windows mantém avisos e UAC. Acompanhe a interface oficial.",
            )?;
        }
        self.store.save_journal(&journal)?;
        let mut started = file.clone();
        started.launch_attempted = true;
        started.save(self.store)?;
        *self.phase.lock().unwrap_or_else(|e| e.into_inner()) = Some(phase);
        (self.log)("Acompanhe o instalador oficial. Nenhuma proteção será desativada nem um processo será encerrado à força. AME pode reiniciar o Windows quando você autorizar a aplicação na própria ferramenta.".into());
        security::launch::wait(
            &file.path(self.store)?,
            file.kind.digest(),
            &file.source,
            parameters,
        )
    }
}

#[cfg(test)]
mod approval_tests {
    use super::*;
    #[test]
    fn automatic_litecord_is_session_only_and_requires_selected_approved_terms() {
        let mut executor =
            super::super::NativeExecutor::new(crate::system_info::SystemInfo::default());
        let mut profile = Profile::default();
        executor.approve_litecord(&profile);
        assert!(!executor.automatic_litecord);
        profile.accept_terms = true;
        executor.approve_litecord(&profile);
        assert!(executor.automatic_litecord);
        assert!(
            !super::super::NativeExecutor::new(crate::system_info::SystemInfo::default())
                .automatic_litecord
        );
        profile.apps.clear();
        executor.approve_litecord(&profile);
        assert!(!executor.automatic_litecord);
    }
}
