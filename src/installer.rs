use crate::{
    Result, atlas, browser, catalog, drivers, fail, security,
    state::{Action, Journal, Profile, SCHEMA, Status, Store, Task},
    system_info::SystemInfo,
    tweaks,
};
use std::{
    collections::HashSet,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
pub mod handoff;
mod process;
pub mod products;
pub mod updates;

pub fn plan(profile: &Profile, info: &SystemInfo) -> Result<Journal> {
    profile.validate()?;
    let mut tasks = vec![];
    let mut seen = HashSet::new();
    let mut push = |id: String, title: String, action: Action, detail: String, admin: &str| {
        if seen.insert(id.clone()) {
            tasks.push(Task {
                id,
                title,
                action,
                status: Status::Pending,
                detail,
                admin: admin.into(),
            });
        }
    };
    if profile.atlas {
        push(
            "atlas".into(),
            "AtlasOS · etapa assistida".into(),
            Action::Atlas,
            if profile.atlas_legacy {
                atlas::legacy::description(info)
            } else {
                atlas::description(info)
            },
            "AME pode solicitar UAC; instalação limpa é manual",
        );
    }
    if profile.drivers {
        push(
            "drivers".into(),
            "Drivers oficiais".into(),
            Action::Drivers,
            drivers::description(info),
            "Instalador oficial pode solicitar UAC",
        );
    }
    if let Some(b) = profile.standalone_browser() {
        push(
            format!("app:{}", b.key()),
            catalog::find(b.key())
                .expect("navegador do catálogo")
                .name
                .into(),
            Action::Install(b.key().into()),
            "Detectar instalação existente antes de instalar.".into(),
            "Conforme escopo do pacote; UAC visível se necessário",
        );
    }
    if !profile.atlas && profile.google_search {
        push(
            "browser".into(),
            "Preferências do navegador".into(),
            Action::Browser,
            browser::guidance(profile),
            "Não",
        );
    }
    for key in &profile.apps {
        if key == "riot"
            && profile
                .apps
                .iter()
                .any(|key| matches!(key.as_str(), "riot_lol_br" | "riot_valorant_br"))
        {
            continue;
        }
        let app = catalog::find(key).ok_or_else(|| fail("App inválido"))?;
        push(
            format!("app:{key}"),
            app.name.into(),
            Action::Install(key.clone()),
            app.note.into(),
            "Conforme instalador; UAC visível se necessário",
        );
    }
    for update in &profile.updates {
        let app = catalog::find(&update.key).ok_or_else(|| fail("App inválido"))?;
        if profile.atlas && app.browser {
            continue;
        }
        push(
            format!("upgrade:{}", update.key),
            format!("Atualizar {} · {} → {}", app.name, update.from, update.to),
            Action::Upgrade(update.clone()),
            "Versão fixada no plano; consulta antes e depois da atualização.".into(),
            "Conforme escopo do pacote; UAC visível se necessário",
        );
    }
    for key in &profile.tweaks {
        let t = tweaks::find(key).ok_or_else(|| fail("Ajuste inválido"))?;
        push(
            format!("tweak:{key}"),
            t.title.into(),
            Action::Tweak(key.clone()),
            t.description.into(),
            "Não; preferência do usuário",
        );
    }
    tasks.retain(|task| !matches!(&task.action, Action::Install(key) if profile.updates.iter().any(|update| &update.key == key)));
    Ok(Journal {
        schema_version: SCHEMA,
        profile_hash: profile.fingerprint()?,
        tasks,
    })
}

pub fn review_plan(
    profile: &Profile,
    info: &SystemInfo,
    previous: Option<&Journal>,
) -> Result<Journal> {
    let mut fresh = plan(profile, info)?;
    if let Some(previous) = previous.filter(|j| j.profile_hash == fresh.profile_hash) {
        for task in &mut fresh.tasks {
            if let Some(saved) = previous.tasks.iter().find(|old| old.id == task.id) {
                task.status = saved.status;
                task.detail = saved.detail.clone();
            }
        }
    }
    Ok(fresh)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Detection {
    Present,
    Absent,
    Unknown(String),
}
pub fn parse_detection(code: i32, text: &str, id: &str) -> Detection {
    if code == 0
        && text
            .split_whitespace()
            .any(|word| word.eq_ignore_ascii_case(id))
    {
        Detection::Present
    } else if code as u32 == 0x8A150014 {
        Detection::Absent
    } else {
        Detection::Unknown(format!(
            "Detecção inconclusiva para {id} (0x{:08X}).",
            code as u32
        ))
    }
}
pub trait Executor {
    fn progress_state(&self) -> Option<Arc<std::sync::Mutex<Option<Status>>>> {
        None
    }
    fn authorize(&mut self, _profile: &Profile) {}
    fn detect(&mut self, key: &str) -> Result<Detection>;
    fn version(&mut self, _key: &str) -> Result<Option<String>> {
        Ok(None)
    }
    fn stage(&self, _task: &Task) -> Option<Status> {
        None
    }
    fn cleanup(&mut self, _task: &Task, _store: &Store) -> Result<()> {
        Ok(())
    }
    fn verify_external(
        &mut self,
        _task: &Task,
        _store: &Store,
        _profile_hash: &str,
    ) -> Result<Option<(Status, String)>> {
        Ok(None)
    }
    fn execute(
        &mut self,
        task: &Task,
        profile: &Profile,
        store: &Store,
        cancel: &AtomicBool,
        log: &mut dyn FnMut(String),
    ) -> Result<(Status, String)>;
}
fn validate_queue(profile: &Profile, journal: &Journal) -> Result<()> {
    if journal.profile_hash != profile.fingerprint()? {
        return Err(fail("Perfil mudou. Gere e aprove um novo plano."));
    }
    let expected = plan(profile, &SystemInfo::default())?;
    if journal.schema_version != SCHEMA
        || journal.tasks.len() != expected.tasks.len()
        || journal
            .tasks
            .iter()
            .zip(&expected.tasks)
            .any(|(a, b)| a.id != b.id || a.action != b.action)
    {
        return Err(fail(
            "A fila não corresponde às operações selecionadas no perfil. Revise um novo plano.",
        ));
    }
    Ok(())
}
/// Called only after the user presses the final approval button. Terms do not
/// add operations or grant permission to execute on a later session.
pub fn approve_reviewed(profile: &Profile, journal: &Journal) -> Result<(Profile, Journal)> {
    validate_queue(profile, journal)?;
    let mut approved = profile.clone();
    approved.accept_terms = true;
    let mut journal = journal.clone();
    journal.profile_hash = approved.fingerprint()?;
    Ok((approved, journal))
}
pub fn execute_plan(
    executor: &mut impl Executor,
    profile: &Profile,
    journal: &mut Journal,
    store: &Store,
    cancel: &AtomicBool,
    mut update: impl FnMut(&Journal),
) -> Result<()> {
    validate_queue(profile, journal)?;
    executor.authorize(profile);
    for index in 0..journal.tasks.len() {
        if journal.tasks[index].status == Status::Completed {
            continue;
        }
        if journal.tasks[index].status.needs_action()
            && matches!(journal.tasks[index].action, Action::Atlas)
        {
            break;
        }
        if journal.tasks[index].status != Status::Pending {
            continue;
        }
        if cancel.load(Ordering::Relaxed) {
            journal.tasks[index].transition(Status::Cancelled,"Cancelado antes do início. Nenhum processo de instalação foi interrompido à força.")?;
            store.save_journal(journal)?;
            update(journal);
            continue;
        }
        journal.tasks[index].transition(
            Status::Verifying,
            "Verificando pré-requisitos e estado atual…",
        )?;
        store.save_journal(journal)?;
        update(journal);
        let result = (|| -> Result<(Status, String)> {
            if let Action::Install(key) = &journal.tasks[index].action {
                match executor.detect(key)? {
                    Detection::Present => {
                        return Ok((
                            Status::Completed,
                            "Instalação existente verificada; nenhuma reinstalação.".into(),
                        ));
                    }
                    Detection::Unknown(reason) => return Ok((Status::RequiresAction, reason)),
                    Detection::Absent => {}
                }
                let mode = updates::mode(key, profile);
                let phase = executor
                    .stage(&journal.tasks[index])
                    .unwrap_or(Status::Installing);
                journal.tasks[index].transition(phase,format!("Modo: {mode}. Progresso indeterminado; cancelar interrompe somente as próximas tarefas."))?;

                store.save_journal(journal)?;
                update(journal);
            }
            if matches!(journal.tasks[index].action, Action::Upgrade(_)) {
                journal.tasks[index].transition(Status::Installing, "Verificando versão aprovada antes de atualizar; o instalador atual não será encerrado à força.")?;
                store.save_journal(journal)?;
                update(journal);
            }
            if matches!(journal.tasks[index].action, Action::Atlas)
                && let Some(stage) = executor.stage(&journal.tasks[index])
            {
                journal.tasks[index].transition(stage, "Preparando/verificando a integração oficial; nenhuma proteção será alterada pelo Ruhtweaks.")?;
                store.save_journal(journal)?;
                update(journal);
            }
            let task = journal.tasks[index].clone();
            let mut messages = Vec::new();
            let mut last_publish = Instant::now() - Duration::from_secs(1);
            let progress = executor.progress_state();
            let mut checkpoint_error = None;
            let outcome = executor.execute(&task, profile, store, cancel, &mut |message| {
                let previous_phase = journal.tasks[index].status;
                if let Some(state) = progress
                    .as_ref()
                    .and_then(|p| p.lock().unwrap_or_else(|e| e.into_inner()).take())
                    && state != journal.tasks[index].status
                    && let Err(error) = journal.tasks[index]
                        .transition(state, message.clone())
                        .and_then(|()| store.save_journal(journal))
                {
                    checkpoint_error = Some(error);
                }
                let phase_changed = previous_phase != journal.tasks[index].status;
                tracing::info!(task=%task.id,message=%message);
                journal.tasks[index].detail = message.clone();
                if phase_changed || last_publish.elapsed() >= Duration::from_millis(150) {
                    update(journal);
                    last_publish = Instant::now();
                }
                if messages.len() < 200 {
                    messages.push(message)
                }
            })?;
            if let Some(error) = checkpoint_error {
                return Err(error);
            }
            let detail = if messages.is_empty() {
                outcome.1
            } else {
                format!("{}\n\nDetalhes:\n{}", outcome.1, messages.join("\n"))
            };
            Ok((outcome.0, detail))
        })();
        let (status, detail) = match result {
            Ok(v) => v,
            Err(e)
                if cancel.load(Ordering::Relaxed)
                    && matches!(
                        journal.tasks[index].status,
                        Status::Preparing | Status::Downloading | Status::Verifying
                    )
                    && matches!(
                        journal.tasks[index].action,
                        Action::Atlas | Action::Install(_) | Action::Drivers
                    ) =>
            {
                (
                    Status::Cancelled,
                    format!(
                        "Cancelamento durante preparação; nenhum instalador foi interrompido: {e}"
                    ),
                )
            }
            Err(e) => (Status::Failed, e.to_string()),
        };
        journal.tasks[index].transition(status, detail)?;
        store.save_journal(journal)?;
        update(journal);
        if status == Status::Completed
            && let Err(error) = executor.cleanup(&journal.tasks[index], store)
        {
            journal.tasks[index]
                .detail
                .push_str(&format!("\nTemporário mantido: {error}"));
            store.save_journal(journal)?;
            update(journal);
        }
        if matches!(
            status,
            Status::RequiresConfirmation | Status::AwaitingReboot | Status::AwaitingVerification
        ) {
            break;
        }
        if matches!(journal.tasks[index].action, Action::Atlas) && status != Status::Completed {
            break;
        }
    }
    Ok(())
}

pub struct NativeExecutor {
    pub info: SystemInfo,
    pub winget: Option<PathBuf>,
    accept_source_terms: bool,
    client: Option<reqwest::blocking::Client>,
    confirmation: Option<handoff::Consent>,
    automatic_litecord: bool,
    phase: Arc<std::sync::Mutex<Option<Status>>>,
}
impl NativeExecutor {
    pub fn grant_confirmation(&mut self, consent: handoff::Consent) {
        self.confirmation = Some(consent);
    }
    fn network_client(&mut self) -> Result<reqwest::blocking::Client> {
        if self.client.is_none() {
            self.client = Some(security::client()?);
        }
        self.client
            .clone()
            .ok_or_else(|| fail("Cliente HTTP não disponível"))
    }
    pub fn installed_versions(
        &mut self,
        cancel: &AtomicBool,
    ) -> Result<std::collections::BTreeMap<String, String>> {
        use std::io::Read;
        let temp = tempfile::tempdir()?;
        let output = temp.path().join("installed.json");
        let mut args = vec![
            "export",
            "--output",
            output.to_str().ok_or_else(|| fail("Caminho não UTF-8"))?,
            "--source",
            "winget",
            "--include-versions",
            "--disable-interactivity",
        ];
        if self.accept_source_terms {
            args.push("--accept-source-agreements");
        }
        let (code, _) = self.command(&args, false, cancel, &mut |_| {})?;
        if code != 0 {
            return Err(fail(format!(
                "Consulta de versões falhou (0x{:08X}). Nenhuma atualização iniciada.",
                code as u32
            )));
        }
        let mut text = String::new();
        std::fs::File::open(output)?
            .take(4 * 1024 * 1024)
            .read_to_string(&mut text)?;
        updates::parse_export(&text)
    }
    pub fn scan_updates(
        &mut self,
        profile: &Profile,
        cancel: &AtomicBool,
    ) -> Result<Vec<crate::state::UpdateRequest>> {
        profile.validate()?;
        if !profile.accept_terms {
            return Err(fail(
                "Autorize os termos da fonte WinGet antes de consultar atualizações.",
            ));
        }
        self.authorize(profile);
        let installed = self.installed_versions(cancel)?;
        let mut result = Vec::new();
        for app in catalog::APPS.iter().filter(|app| {
            profile.apps.iter().any(|key| key == app.key)
                || profile
                    .standalone_browser()
                    .is_some_and(|b| b.key() == app.key)
        }) {
            if !catalog::supports_updates(app.key) {
                continue;
            }
            let Some(id) = app.winget else { continue };
            let Some(current) = installed.get(id) else {
                continue;
            };
            let (code, text) = self.command(
                &[
                    "show",
                    "--id",
                    id,
                    "--exact",
                    "--source",
                    "winget",
                    "--versions",
                    "--accept-source-agreements",
                    "--disable-interactivity",
                ],
                false,
                cancel,
                &mut |_| {},
            )?;
            if code != 0 {
                return Err(fail(format!(
                    "Não foi possível consultar as versões de {}.",
                    app.name
                )));
            }
            if let Some(request) =
                updates::candidate(app.key, current, &updates::parse_versions(&text))
            {
                result.push(request);
            }
        }
        Ok(result)
    }
    pub fn new(info: SystemInfo) -> Self {
        Self {
            info,
            winget: None,
            accept_source_terms: false,
            client: None,
            confirmation: None,
            automatic_litecord: false,
            phase: Arc::default(),
        }
    }
}
impl NativeExecutor {
    fn winget(&mut self) -> Result<PathBuf> {
        if let Some(path) = &self.winget {
            return Ok(path.clone());
        }
        let path = security::winget_path()?;
        self.winget = Some(path.clone());
        Ok(path)
    }
    fn command(
        &mut self,
        args: &[&str],
        install: bool,
        cancel: &AtomicBool,
        log: &mut dyn FnMut(String),
    ) -> Result<(i32, String)> {
        let path = self.winget()?;
        security::verify_signature(&path)?;
        let mut command = Command::new(path);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        process::collect(command.spawn()?, install, cancel, log)
    }
}
impl Executor for NativeExecutor {
    fn progress_state(&self) -> Option<Arc<std::sync::Mutex<Option<Status>>>> {
        Some(self.phase.clone())
    }
    fn stage(&self, task: &Task) -> Option<Status> {
        match &task.action {
            Action::Install(key) if key == "litecord" => Some(Status::Preparing),
            Action::Atlas => Some(Status::Preparing),
            _ => None,
        }
    }
    fn cleanup(&mut self, task: &Task, store: &Store) -> Result<()> {
        if task.status != Status::Completed {
            return Ok(());
        }
        let kind = match &task.action {
            Action::Atlas => handoff::Kind::Atlas,
            Action::Install(key) if key == "litecord" => handoff::Kind::Litecord,
            _ => return Ok(()),
        };
        if let Some(file) = handoff::load(store, kind)? {
            let journal = store
                .load_journal()?
                .ok_or_else(|| fail("Resultado durável ausente"))?;
            if file.profile_hash != journal.profile_hash
                || !journal
                    .tasks
                    .iter()
                    .any(|t| t.id == task.id && t.status == Status::Completed)
            {
                return Err(fail("Limpeza sem conclusão durável correspondente"));
            }
            if let Some(parent) = file.path(store)?.parent() {
                security::transfer::recover(parent, kind.digest())?;
            }
            handoff::cleanup_file(
                &file.path(store)?,
                &store.root.join("downloads"),
                kind.digest(),
                task.status,
            )?;
            if kind == handoff::Kind::Atlas {
                handoff::cleanup_file(
                    &handoff::artifact_path(
                        store,
                        "AtlasPlaybook_v0.5.0-hotfix.apbx",
                        atlas::REVIEWED_DIGEST,
                    )?,
                    &store.root.join("downloads"),
                    atlas::REVIEWED_DIGEST,
                    task.status,
                )?;
            }
        }
        Ok(())
    }
    fn verify_external(
        &mut self,
        task: &Task,
        store: &Store,
        profile_hash: &str,
    ) -> Result<Option<(Status, String)>> {
        if task.action != Action::Atlas {
            if task.action == Action::Drivers {
                let result = drivers::amd_update::revalidate(store, profile_hash)?;
                if result.is_none()
                    && task.status == Status::AwaitingReboot
                    && !atlas::preflight::reboot_pending()?
                {
                    return Ok(Some((Status::RequiresAction, "Reinício não está mais pendente. Tente novamente para buscar o driver AMD.".into())));
                }
                return Ok(result);
            }
            return Ok(None);
        }
        if store.load_profile()?.atlas_legacy {
            if task.status == Status::AwaitingReboot && !atlas::preflight::reboot_pending()? {
                return Ok(Some((Status::RequiresAction, "Reinício não está mais pendente. A preparação do Atlas legado pode ser retomada.".into())));
            }
            return Ok(None);
        }
        let Some(file) = handoff::load(store, handoff::Kind::Atlas)? else {
            if task.status == Status::AwaitingReboot && !atlas::preflight::reboot_pending()? {
                return Ok(Some((
                    Status::RequiresAction,
                    "Reinício não está mais pendente. Tente novamente para preparar o Atlas."
                        .into(),
                )));
            }
            return Ok(None);
        };
        if file.profile_hash != profile_hash || !file.launch_attempted {
            return Ok(None);
        }
        Ok(Some(handoff::verify_atlas(&file)))
    }
    fn authorize(&mut self, profile: &Profile) {
        self.accept_source_terms = profile.accept_terms;
    }
    fn detect(&mut self, key: &str) -> Result<Detection> {
        let app = catalog::find(key).ok_or_else(|| fail("App desconhecido"))?;
        if let Some(id) = app.winget {
            let mut args = vec![
                "list",
                "--id",
                id,
                "--exact",
                "--source",
                "winget",
                "--disable-interactivity",
            ];
            if self.accept_source_terms {
                args.push("--accept-source-agreements");
            }
            let (code, text) = self.command(&args, false, &AtomicBool::new(false), &mut |_| {})?;
            Ok(products::confirmed_registration(
                key,
                parse_detection(code, &text, id),
            ))
        } else {
            crate::installer::products::detect(key)
        }
    }
    fn version(&mut self, key: &str) -> Result<Option<String>> {
        let id = catalog::find(key)
            .and_then(|app| app.winget)
            .ok_or_else(|| fail("App sem versão WinGet"))?;
        Ok(self.installed_versions(&AtomicBool::new(false))?.remove(id))
    }
    fn execute(
        &mut self,
        task: &Task,
        profile: &Profile,
        store: &Store,
        cancel: &AtomicBool,
        log: &mut dyn FnMut(String),
    ) -> Result<(Status, String)> {
        *self.phase.lock().unwrap_or_else(|e| e.into_inner()) = None;
        match &task.action {
            Action::Install(key) => {
                let app = catalog::find(key).ok_or_else(|| fail("App desconhecido"))?;
                if key == "litecord" {
                    return self.prepared_flow(
                        handoff::Kind::Litecord,
                        profile,
                        store,
                        cancel,
                        log,
                    );
                }
                let Some(id) = app.winget else {
                    return Ok((
                        Status::RequiresAction,
                        format!(
                            "{}\nAbra a fonte oficial e instale manualmente. A instalação e eventuais componentes precisam ser revisados; não há ID WinGet independente validado.",
                            app.note
                        ),
                    ));
                };
                if !profile.accept_terms {
                    return Ok((Status::RequiresAction,"Revise os termos do pacote e da fonte. Marque a autorização de termos no perfil antes de executar WinGet.".into()));
                }
                let (code, text) = self.command(
                    &[
                        "show",
                        "--id",
                        id,
                        "--exact",
                        "--source",
                        "winget",
                        "--accept-source-agreements",
                        "--disable-interactivity",
                    ],
                    false,
                    cancel,
                    log,
                )?;
                if code != 0 {
                    return Err(fail(format!(
                        "Pacote indisponível (0x{:08X}): {text}",
                        code as u32
                    )));
                }
                if cancel.load(Ordering::Relaxed) {
                    return Ok((
                        Status::RequiresAction,
                        "Cancelamento recebido antes de iniciar o instalador.".into(),
                    ));
                }
                let args = updates::command(key, profile, None)?;
                log(format!(
                    "{}: {}. Nenhum parâmetro de instalador é sobrescrito.",
                    app.name,
                    updates::mode(key, profile)
                ));
                let (code, text) = self.command(
                    &args.iter().map(String::as_str).collect::<Vec<_>>(),
                    true,
                    cancel,
                    log,
                )?;
                if code == 3010 {
                    return Ok((Status::RequiresAction,"Instalador solicita reinicialização. Salve o trabalho e reinicie manualmente; reabra Ruhtweaks para verificar.".into()));
                }
                if code != 0 {
                    return Err(fail(format!(
                        "WinGet retornou 0x{:08X}. {text}",
                        code as u32
                    )));
                }
                match self.detect(key)? {
                    Detection::Present => Ok((Status::Completed, "Instalação confirmada por consulta exata ao WinGet. Revise opções de inicialização nas Configurações.".into())),
                    Detection::Unknown(reason) => Ok((Status::RequiresAction, reason)),
                    Detection::Absent => Ok((Status::RequiresAction, "Processo terminou, mas a instalação não pôde ser confirmada. Nenhuma repetição automática.".into())),
                }
            }
            Action::Upgrade(request) => updates::apply(
                &mut NativeUpdate {
                    executor: self,
                    cancel,
                    log,
                },
                request,
                profile,
            ),
            Action::Atlas if profile.atlas_legacy => {
                atlas::legacy::prepare(&self.info, profile, store, cancel, log)
            }
            Action::Atlas => self.prepared_flow(handoff::Kind::Atlas, profile, store, cancel, log),
            Action::Drivers => {
                let phase = self.phase.clone();
                drivers::amd_update::prepare(
                    &self.info,
                    profile,
                    store,
                    cancel,
                    &mut |state, message| {
                        *phase.lock().unwrap_or_else(|e| e.into_inner()) = Some(state);
                        log(message);
                    },
                )
            }
            Action::Browser => {
                if let Some(selected) = profile.browser
                    && self.detect(selected.key())? != Detection::Present
                {
                    return Ok((
                        Status::RequiresAction,
                        "Instale e revalide o navegador antes de configurar suas preferências."
                            .into(),
                    ));
                }
                browser::apply(profile, store)
            }
            Action::Tweak(key) => {
                let tweak = tweaks::find(key).ok_or_else(|| fail("Ajuste inválido"))?;
                if key == "extensions" {
                    tweaks::apply_extensions(store)?;
                    Ok((Status::Completed,"Extensões visíveis: valor relido e confirmado. Backup preservado; reversão em Logs e detalhes do PC → Ferramentas e reversões.".into()))
                } else if key == "balanced" {
                    tweaks::power::apply_with(&mut tweaks::power::NativePower, store)?;
                    Ok((Status::Completed, "Plano Equilibrado ativado e relido pela API do Windows. Plano anterior preservado para reversão.".into()))
                } else {
                    Ok((
                        Status::RequiresAction,
                        format!(
                            "{}\nUse Logs e detalhes do PC → Ferramentas e reversões. Abrir uma página não confirma a conclusão do ajuste.",
                            tweak.description
                        ),
                    ))
                }
            }
        }
    }
}

pub fn revalidate(executor: &mut impl Executor, journal: &mut Journal) -> Result<()> {
    for task in &mut journal.tasks {
        if task.status.needs_action()
            && let Action::Install(key) = &task.action
            && executor.detect(key)? == Detection::Present
        {
            task.status = Status::Completed;
            task.detail =
                "Instalação confirmada após retomada, sem executar outro instalador.".into();
        }
        if task.status.needs_action()
            && let Action::Upgrade(request) = &task.action
            && executor.version(&request.key)?.as_deref() == Some(&request.to)
        {
            task.status = Status::Completed;
            task.detail = format!(
                "Versão {} confirmada após retomada; instalador não repetido.",
                request.to
            );
        }
    }
    Ok(())
}
pub fn revalidate_in_store(
    executor: &mut impl Executor,
    journal: &mut Journal,
    store: &Store,
) -> Result<()> {
    revalidate(executor, journal)?;
    for task in &mut journal.tasks {
        if task.status.needs_action()
            && let Some((status, detail)) =
                executor.verify_external(task, store, &journal.profile_hash)?
        {
            task.status = status;
            task.detail = detail;
        }
    }
    store.save_journal(journal)?;
    for task in &mut journal.tasks {
        if task.status == Status::Completed
            && let Err(e) = executor.cleanup(task, store)
        {
            task.detail.push_str(&format!("\nTemporário mantido: {e}"));
        }
    }
    store.save_journal(journal)
}
pub fn retry_unfinished(journal: &mut Journal) -> Result<()> {
    for task in &mut journal.tasks {
        if matches!(task.status, Status::Failed | Status::Cancelled) || task.status.needs_action() {
            task.transition(
                Status::Pending,
                "Nova tentativa autorizada; detecção será repetida.",
            )?
        }
    }
    Ok(())
}
struct NativeUpdate<'a> {
    executor: &'a mut NativeExecutor,
    cancel: &'a AtomicBool,
    log: &'a mut dyn FnMut(String),
}
impl updates::UpdateBackend for NativeUpdate<'_> {
    fn installed_version(&mut self, key: &str) -> Result<Option<String>> {
        let id = catalog::find(key)
            .and_then(|app| app.winget)
            .ok_or_else(|| fail("Pacote inválido"))?;
        Ok(self.executor.installed_versions(self.cancel)?.remove(id))
    }
    fn upgrade(&mut self, request: &crate::state::UpdateRequest, profile: &Profile) -> Result<i32> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(fail("Cancelado antes de atualizar"));
        }
        let args = updates::command(&request.key, profile, Some(request))?;
        let (code, _) = self.executor.command(
            &args.iter().map(String::as_str).collect::<Vec<_>>(),
            true,
            self.cancel,
            self.log,
        )?;
        Ok(code)
    }
}
pub fn cancellation() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}
