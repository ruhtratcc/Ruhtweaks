use crate::{
    Result, atlas, browser,
    catalog::{self, Browser},
    drivers,
    installer::{self, NativeExecutor},
    security,
    state::{Journal, Profile, Status, Store},
    system_info::{self, SystemInfo},
    tweaks,
};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};
slint::include_modules!();
mod dispatch;
mod handoff;
mod navigation;
pub mod presentation;
use navigation::{Navigation, Next};

fn read_navigation(ui: &MainWindow) -> Navigation {
    Navigation {
        page: ui.get_page(),
        wizard: ui.get_wizard(),
        step: ui.get_wizard_step(),
        ..Navigation::default()
    }
}
fn navigate(ui: &MainWindow, store: &Store, nav: Navigation) -> Result<()> {
    nav.save(store)?;
    ui.set_page(nav.page);
    ui.set_wizard(nav.wizard);
    ui.set_wizard_step(nav.step);
    ui.set_more_open(false);
    Ok(())
}
fn update_selection(ui: &MainWindow, profile: &Profile, _apps: &VecModel<AppRow>) {
    let (count, summary) = presentation::selection_summary(profile);
    ui.set_selected_count(count);
    ui.set_selected_summary(summary.into());
    ui.set_litecord_selected(profile.apps.iter().any(|key| key == "litecord"));
    ui.set_changes_summary(
        profile
            .tweaks
            .iter()
            .filter_map(|key| tweaks::find(key).map(|t| t.title))
            .collect::<Vec<_>>()
            .join(" · ")
            .into(),
    );
}

fn read_profile(ui: &MainWindow, apps: &VecModel<AppRow>, tweaks: &VecModel<TweakRow>) -> Profile {
    let app_catalog = catalog::APPS
        .iter()
        .filter(|a| !a.browser)
        .collect::<Vec<_>>();
    Profile {
        browser: Browser::from_index(ui.get_browser_index()),
        apps: (0..apps.row_count())
            .filter(|i| apps.row_data(*i).is_some_and(|a| a.selected))
            .map(|i| app_catalog[i].key.to_string())
            .collect(),
        atlas: ui.get_atlas_selected(),
        drivers: ui.get_drivers_selected(),
        google_search: ui.get_google_selected() && ui.get_browser_index() < 4,
        atlas_legacy: ui.get_atlas_selected() && ui.get_atlas_legacy(),
        tweaks: (0..tweaks.row_count())
            .filter(|i| tweaks.row_data(*i).is_some_and(|t| t.selected))
            .map(|i| crate::tweaks::TWEAKS[i].key.to_string())
            .collect(),
        accept_terms: ui.get_terms_accepted(),
        allow_startup: ui.get_allow_startup(),
        prefer_silent: ui.get_prefer_silent(),
        updates: (0..ui.get_updates().row_count())
            .filter_map(|i| ui.get_updates().row_data(i))
            .filter(|row| {
                row.selected
                    && ((!ui.get_atlas_selected()
                        && Browser::from_index(ui.get_browser_index())
                            .is_some_and(|b| b.key() == row.key.as_str()))
                        || (0..apps.row_count()).any(|i| {
                            apps.row_data(i).is_some_and(|a| a.selected)
                                && app_catalog[i].key == row.key.as_str()
                        }))
            })
            .map(|row| crate::state::UpdateRequest {
                key: row.key.to_string(),
                from: row.from.to_string(),
                to: row.to.to_string(),
            })
            .collect(),
        ..Profile::default()
    }
}
fn prepare_automatic_profile(
    ui: &MainWindow,
    apps: &VecModel<AppRow>,
    tweaks: &VecModel<TweakRow>,
    info: &SystemInfo,
) -> Profile {
    let mut profile = read_profile(ui, apps, tweaks);
    profile.include_automatic_setup(info);
    ui.set_drivers_selected(profile.drivers);
    ui.set_google_selected(profile.google_search);
    for index in 0..tweaks.row_count() {
        if let Some(mut row) = tweaks.row_data(index) {
            let selected = profile
                .tweaks
                .iter()
                .any(|key| key == crate::tweaks::TWEAKS[index].key);
            if row.selected != selected {
                row.selected = selected;
                tweaks.set_row_data(index, row);
            }
        }
    }
    let profile = read_profile(ui, apps, tweaks);
    update_selection(ui, &profile, apps);
    profile
}
fn show_journal(ui: &MainWindow, journal: &Journal) {
    ui.set_needs_check(journal.tasks.iter().any(|t| {
        matches!(
            t.status,
            Status::AwaitingReboot | Status::AwaitingVerification
        )
    }));
    ui.set_remaining_pending(journal.tasks.iter().any(|t| t.status == Status::Pending));
    ui.set_retry_needed(journal.tasks.iter().any(|t| {
        matches!(t.status, Status::Failed | Status::Cancelled)
            || (t.status == Status::RequiresAction
                && matches!(
                    t.action,
                    crate::state::Action::Atlas | crate::state::Action::Drivers
                ))
    }));
    ui.set_external_wait(
        journal
            .tasks
            .iter()
            .any(|task| task.status == Status::ExternalAction),
    );
    let (title, summary) = presentation::run_summary(journal);
    ui.set_run_title(title.into());
    ui.set_run_summary(summary.into());
    if ui.get_page() == 8 {
        ui.set_details(journal.report().into());
    }
    if let Some(atlas) = journal
        .tasks
        .iter()
        .find(|task| task.action == crate::state::Action::Atlas)
    {
        ui.set_atlas_stage(atlas.status.label().into());
    }
    if let Some(task) = journal
        .tasks
        .iter()
        .find(|t| t.status.active() || (ui.get_busy() && t.status == Status::ExternalAction))
    {
        ui.set_status(
            format!(
                "{} · {} · {}",
                task.title,
                task.status.label(),
                task.detail.chars().take(160).collect::<String>()
            )
            .into(),
        );
    }
    let rows = journal
        .tasks
        .iter()
        .map(|t| QueueRow {
            title: t.title.clone().into(),
            status: t.status.label().into(),
            detail: format!(
                "{}\n\nPrivilégios: {}\n\n{}{}",
                t.title,
                t.admin,
                t.detail.chars().take(8192).collect::<String>(),
                if t.detail.chars().count() > 8192 {
                    "\nDetalhes completos no relatório técnico."
                } else {
                    ""
                }
            )
            .into(),
            code: presentation::status_code(t.status),
        })
        .collect::<Vec<_>>();
    let model = ui.get_queue();
    if model.row_count() == rows.len() {
        for (index, row) in rows.into_iter().enumerate() {
            if model.row_data(index).as_ref() != Some(&row) {
                model.set_row_data(index, row);
            }
        }
    } else {
        ui.set_queue(ModelRc::new(VecModel::from(rows)));
    }
}
fn message(ui: &MainWindow, result: Result<()>, ok: &str) {
    ui.set_notice_tone(if result.is_err() { 3 } else { 0 });
    ui.set_status(
        match result {
            Ok(()) => ok.to_string(),
            Err(e) => e.to_string(),
        }
        .into(),
    );
}

pub fn run(
    store: Store,
    profile: Profile,
    existing: Option<Journal>,
    first_run: bool,
) -> Result<()> {
    let ui = MainWindow::new().map_err(|e| crate::fail(e.to_string()))?;
    ui.set_data_path(store.root.display().to_string().into());
    ui.set_browser_index(profile.browser.map_or(4, Browser::index));
    ui.set_atlas_selected(profile.atlas);
    ui.set_drivers_selected(profile.drivers);
    ui.set_google_selected(profile.google_search);
    ui.set_atlas_legacy(profile.atlas_legacy);
    ui.set_terms_accepted(profile.accept_terms);
    ui.set_allow_startup(profile.allow_startup);
    ui.set_prefer_silent(profile.prefer_silent);
    let updates = Rc::new(VecModel::from(update_rows(&profile.updates, true)));
    ui.set_updates(ModelRc::from(updates.clone()));
    let apps = Rc::new(VecModel::from(presentation::app_rows(&profile)));
    let tweaks = Rc::new(VecModel::from(presentation::tweak_rows(
        &profile,
        "Consultando estado…",
    )));
    ui.set_apps(ModelRc::from(apps.clone()));
    ui.set_tweaks(ModelRc::from(tweaks.clone()));
    update_selection(&ui, &profile, &apps);
    match Navigation::load(&store) {
        Ok(nav) => {
            ui.set_page(0);
            ui.set_wizard(nav.wizard);
            ui.set_wizard_step(0);
        }
        Err(error) => message(&ui, Err(error), ""),
    }
    let info = Rc::new(RefCell::new(SystemInfo::default()));
    let journal = Rc::new(RefCell::new(existing));
    let cancel = installer::cancellation();
    let active = Arc::new(AtomicBool::new(false));
    let close_active = active.clone();
    let authorized = Rc::new(RefCell::new(None::<String>));
    if let Some(j) = journal.borrow().as_ref() {
        show_journal(&ui, j);
        ui.set_status(
            "Fila recuperada. Revalide as instalações; execução exige nova revisão.".into(),
        );
    }
    let sender = UiSender::new(ui.as_weak());
    {
        let weak = ui.as_weak();
        let info = info.clone();
        let journal = journal.clone();
        let active = active.clone();
        let tweaks = tweaks.clone();
        let updates = updates.clone();
        let authorized = authorized.clone();
        let event_store = store.clone();
        let inbox = sender.inbox.clone();
        let suggest_atlas = Cell::new(first_run && journal.borrow().is_none());
        ui.on_worker_wakeup(move || {
            let Some(ui) = weak.upgrade() else { return };
            let events = inbox.lock().unwrap_or_else(|e| e.into_inner()).take();
            for event in events {
                match event {
                    UiEvent::Prepared(result) => match result {
                        Ok(Some(file)) => {
                            let file_path = file.path(&event_store).map_or_else(|e| e.to_string(), |p| p.display().to_string());
                            ui.set_handoff_kind(if file.kind == installer::handoff::Kind::Litecord { 0 } else { 1 });
                            let playbook = if file.kind == installer::handoff::Kind::Atlas { format!("\nPlaybook: {}", installer::handoff::artifact_path(&event_store, "AtlasPlaybook_v0.5.0-hotfix.apbx", atlas::REVIEWED_DIGEST).map_or_else(|e| e.to_string(), |p| p.display().to_string())) } else { String::new() };
                            ui.set_handoff_description(format!("{}\nArquivo: {}{playbook}", file.prompt(), file_path).into());
                            ui.set_handoff_ack(false);
                            ui.set_handoff_allowed(authorized.borrow().as_deref() == Some(&file.profile_hash) && !matches!(file.signature, security::Signature::Unverifiable(_)));
                        },
                        Ok(None) => { ui.set_handoff_kind(-1); ui.set_handoff_ack(false); ui.set_handoff_allowed(false); },
                        Err(error) => message(&ui, Err(error), ""),
                    },
                    UiEvent::Operation(result, text) => { active.store(false, Ordering::Relaxed); ui.set_busy(false); message(&ui, result, &text); },
                    UiEvent::Updates(result) => {
                        active.store(false, Ordering::Relaxed);
                        ui.set_busy(false);
                        match result {
                            Ok(requests) => {
                                ui.set_updates_status(format!("{} atualizações identificadas. Marque as versões que deseja aprovar. Versões sem correspondência no catálogo são ignoradas.", requests.len()).into());
                                updates.set_vec(update_rows(&requests, false));
                                ui.invoke_changed();
                            }
                            Err(error) => { ui.set_updates_status("Consulta não concluída. Nenhuma atualização nova foi selecionada; tente novamente após resolver o erro.".into()); message(&ui, Err(error), ""); },
                        }
                    }
                    UiEvent::Power(state) => {
                        if let Some(index) = crate::tweaks::TWEAKS.iter().position(|t| t.key == "balanced")
                            && let Some(mut row) = tweaks.row_data(index) { row.state = state.into(); tweaks.set_row_data(index, row); }
                    }
                    UiEvent::Info(inventory) => {
                        presentation::populate_inventory(&ui, &inventory);
                        if suggest_atlas.replace(false) {
                            let suggested = Profile::suggested_for(&inventory);
                            ui.set_atlas_selected(suggested.atlas);
                            ui.set_atlas_legacy(suggested.atlas_legacy);
                            ui.invoke_changed();
                        }
                        if !inventory.supported() {
                            message(&ui, Err(crate::fail("O Windows não foi identificado como suportado. Consulte os detalhes do PC antes de executar.")), "");
                        }
                        *info.borrow_mut() = inventory;
                        tracing::info!("Inventário local concluído");
                    }
                    UiEvent::Extensions(state) => {
                        if let Some(index) = crate::tweaks::TWEAKS.iter().position(|t| t.key == "extensions")
                            && let Some(mut row) = tweaks.row_data(index) {
                            row.state = state.into(); tweaks.set_row_data(index, row);
                        }
                    }
                    UiEvent::Journal(updated) => {
                        show_journal(&ui, &updated);
                        *journal.borrow_mut() = Some(updated);
                    }
                    UiEvent::Finished(result) => {
                        let interrupted = result.is_err();
                        active.store(false, Ordering::Relaxed);
                        ui.set_busy(false); ui.set_approval(false);
                        message(&ui, result, "Execução encerrada. Confira o resultado de cada etapa.");
                        if let Some(journal) = journal.borrow().as_ref() {
                            show_journal(&ui, journal);
                            if interrupted { ui.set_run_title("Execução interrompida.".into()); }
                            else if journal.tasks.iter().any(|task| task.status == Status::Failed || task.status.needs_action()) {
                                ui.set_notice_tone(2);
                                ui.set_status("Há etapas que precisam de atenção. Consulte as pendências abaixo.".into());
                            }
                        }
                    }
                }
            }
        });
    }
    {
        let weak = ui.as_weak();
        let sender = sender.clone();
        let apps = apps.clone();
        let tweaks = tweaks.clone();
        let info = info.clone();
        let active = active.clone();
        let cancel = cancel.clone();
        ui.on_scan_updates(move || {
            let Some(ui) = weak.upgrade() else { return };
            let profile = read_profile(&ui, &apps, &tweaks);
            if !profile.accept_terms {
                message(
                    &ui,
                    Err(crate::fail(
                        "Autorize os termos em Aplicativos → Mais opções antes da consulta.",
                    )),
                    "",
                );
                return;
            }
            if active.swap(true, Ordering::Relaxed) {
                return;
            }
            ui.set_busy(true);
            ui.set_plan_ready(false);
            ui.set_approval(false);
            ui.set_updates_status(
                "Consultando versões pelo WinGet… Nenhuma instalação será iniciada.".into(),
            );
            cancel.store(false, Ordering::Relaxed);
            let sender = sender.clone();
            let cancel = cancel.clone();
            let inventory = info.borrow().clone();
            thread::spawn(move || {
                let result = NativeExecutor::new(inventory).scan_updates(&profile, &cancel);
                let _ = sender.send(UiEvent::Updates(result));
            });
        });
    }
    {
        let weak = ui.as_weak();
        let updates = updates.clone();
        ui.on_toggle_update(move |index, selected| {
            if let Some(mut row) = updates.row_data(index as usize) {
                row.selected = selected;
                updates.set_row_data(index as usize, row);
                if let Some(ui) = weak.upgrade() {
                    ui.invoke_changed();
                }
            }
        });
    }
    {
        let sender = sender.clone();
        send_inventory(sender);
    }
    {
        let weak = ui.as_weak();
        let store = store.clone();
        let apps = apps.clone();
        let tweaks = tweaks.clone();
        let updates = updates.clone();
        let authorized = authorized.clone();
        ui.on_changed(move || {
            if let Some(ui) = weak.upgrade() {
                ui.set_plan_ready(false);
                ui.set_approval(false);
                *authorized.borrow_mut() = None;
                ui.set_handoff_allowed(false);
                ui.set_handoff_ack(false);
                let profile = read_profile(&ui, &apps, &tweaks);
                for index in 0..updates.row_count() {
                    if let Some(mut row) = updates.row_data(index)
                        && row.selected
                        && !profile
                            .updates
                            .iter()
                            .any(|update| update.key == row.key.as_str())
                    {
                        row.selected = false;
                        updates.set_row_data(index, row);
                    }
                }
                update_selection(&ui, &profile, &apps);
                message(
                    &ui,
                    store.save_profile(&profile),
                    "Seleção salva. Você pode continuar quando quiser.",
                );
            }
        });
    }
    {
        let weak = ui.as_weak();
        let apps = apps.clone();
        ui.on_toggle_app(move |index, selected| {
            if let Some(mut row) = apps.row_data(index as usize) {
                row.selected = selected;
                apps.set_row_data(index as usize, row);
                if let Some(ui) = weak.upgrade() {
                    ui.invoke_changed();
                }
            }
        });
    }
    {
        let weak = ui.as_weak();
        let tweaks = tweaks.clone();
        ui.on_toggle_tweak(move |index, selected| {
            if let Some(mut row) = tweaks.row_data(index as usize) {
                row.selected = selected;
                tweaks.set_row_data(index as usize, row);
                if let Some(ui) = weak.upgrade() {
                    ui.invoke_changed();
                }
            }
        });
    }
    {
        let weak = ui.as_weak();
        let store = store.clone();
        let apps = apps.clone();
        let tweaks = tweaks.clone();
        let info = info.clone();
        let journal = journal.clone();
        ui.on_prepare(move || {
            let Some(ui) = weak.upgrade() else { return };
            let profile = prepare_automatic_profile(&ui, &apps, &tweaks, &info.borrow());
            let result = (|| -> Result<()> {
                store.save_profile(&profile)?;
                let planned = installer::review_plan(&profile, &info.borrow(), journal.borrow().as_ref())?;
                show_journal(&ui, &planned);
                let operations = planned.tasks.iter().map(|task| {
                    format!("• {} | {}", task.title, task.admin)
                }).collect::<Vec<_>>().join("\n");
                let terms = if profile.accept_terms { "autorizados" } else { "serão autorizados no botão Iniciar e aceitar termos" };
                let startup = if profile.allow_startup { "escolha permitida nos instaladores" } else { "desmarcar nos instaladores e revisar no Windows" };
                ui.set_details(format!(
                    "REVISÃO · {} operações\nNenhuma instalação ou ajuste foi executado nesta revisão.\n\n{operations}\n\nTermos WinGet: {terms}.\nInicialização: {startup}.\n\nReveja cada tarefa, os privilégios e limitações; depois aprove o plano.", planned.tasks.len()
                ).into());
                *journal.borrow_mut() = Some(planned);
                ui.set_plan_ready(true);
                ui.set_approval(false);
                let mut nav=read_navigation(&ui); nav.visit(6); navigate(&ui,&store,nav)?;
                Ok(())
            })();
            message(&ui, result, "Plano preparado. Clique em Iniciar para executar a fila.");
        });
    }
    {
        let weak = ui.as_weak();
        let store = store.clone();
        let journal = journal.clone();
        let info = info.clone();
        let apps = apps.clone();
        let tweaks = tweaks.clone();
        let cancel = cancel.clone();
        let active = active.clone();
        let sender = sender.clone();
        let authorized = authorized.clone();
        ui.on_execute(move || {
            let Some(ui) = weak.upgrade() else { return };
            if !ui.get_approval() || !ui.get_plan_ready() || active.swap(true, Ordering::Relaxed) {
                return;
            }
            let profile = read_profile(&ui, &apps, &tweaks);
            let Some(j) = journal.borrow().clone() else {
                active.store(false, Ordering::Relaxed);
                return;
            };
            let (profile, mut j) =
                match installer::approve_reviewed(&profile, &j).and_then(|(p, j)| {
                    store.save_profile(&p)?;
                    Ok((p, j))
                }) {
                    Ok(value) => value,
                    Err(error) => {
                        active.store(false, Ordering::Relaxed);
                        message(&ui, Err(error), "");
                        return;
                    }
                };
            ui.set_terms_accepted(true);
            *authorized.borrow_mut() = profile.fingerprint().ok();
            let info = info.borrow().clone();
            if !info.supported() {
                active.store(false, Ordering::Relaxed);
                ui.set_status(
                    "Inventário incompleto ou Windows não suportado. Atualize o inventário.".into(),
                );
                return;
            }
            ui.set_busy(true);
            let mut nav = read_navigation(&ui);
            nav.visit(7);
            if let Err(error) = navigate(&ui, &store, nav) {
                active.store(false, Ordering::Relaxed);
                ui.set_busy(false);
                message(&ui, Err(error), "");
                return;
            }
            cancel.store(false, Ordering::Relaxed);
            let cancel = cancel.clone();
            let store = store.clone();
            let sender = sender.clone();
            thread::spawn(move || {
                let mut executor = NativeExecutor::new(info);
                executor.approve_litecord(&profile);
                let result = installer::execute_plan(
                    &mut executor,
                    &profile,
                    &mut j,
                    &store,
                    &cancel,
                    |j| {
                        let _ = sender.send(UiEvent::Journal(j.clone()));
                    },
                );
                send_prepared(&sender, &store, &j);
                send_extensions(&sender);
                let _ = sender.send(UiEvent::Finished(result));
            });
        });
    }
    {
        let weak = ui.as_weak();
        let cancel = cancel.clone();
        ui.on_cancel(move || {
            cancel.store(true, Ordering::Relaxed);
            if let Some(ui) = weak.upgrade() {
                ui.set_status("Cancelamento solicitado. O instalador atual pode precisar terminar; próximas tarefas serão canceladas.".into());
            }
        });
    }
    {
        let weak = ui.as_weak();
        let store = store.clone();
        let journal = journal.clone();
        let info = info.clone();
        let sender = sender.clone();
        let active = active.clone();
        ui.on_revalidate(move || {
            let Some(ui) = weak.upgrade() else { return };
            let Some(mut j) = journal.borrow().clone() else {
                return;
            };
            if active.swap(true, Ordering::Relaxed) {
                return;
            }
            ui.set_busy(true);
            let info = info.borrow().clone();
            let mut nav = read_navigation(&ui);
            nav.visit(7);
            if let Err(error) = navigate(&ui, &store, nav) {
                active.store(false, Ordering::Relaxed);
                ui.set_busy(false);
                message(&ui, Err(error), "");
                return;
            }
            ui.set_status(
                "Verificando as instalações existentes. Nenhum instalador será iniciado.".into(),
            );
            let store = store.clone();
            let sender = sender.clone();
            thread::spawn(move || {
                let result =
                    installer::revalidate_in_store(&mut NativeExecutor::new(info), &mut j, &store);
                send_prepared(&sender, &store, &j);
                let _ = sender.send(UiEvent::Journal(j));
                send_extensions(&sender);
                let _ = sender.send(UiEvent::Finished(result));
            });
        });
    }
    {
        let weak = ui.as_weak();
        let journal = journal.clone();
        let store = store.clone();
        let apps = apps.clone();
        let tweaks = tweaks.clone();
        ui.on_retry(move || {
            let Some(ui) = weak.upgrade() else { return };
            let profile = read_profile(&ui, &apps, &tweaks);
            let result = (|| -> Result<()> {
                let mut journal = journal.borrow_mut();
                let Some(saved) = journal.as_mut() else {
                    return Ok(());
                };
                if saved.profile_hash != profile.fingerprint()? {
                    return Err(crate::fail(
                        "A seleção mudou. Revise um novo plano antes de repetir tarefas.",
                    ));
                }
                installer::retry_unfinished(saved)?;
                store.save_journal(saved)?;
                show_journal(&ui, saved);
                ui.set_plan_ready(true);
                ui.set_approval(false);
                let mut nav = read_navigation(&ui);
                nav.visit(6);
                navigate(&ui, &store, nav)
            })();
            message(
                &ui,
                result,
                "Nova tentativa preparada. Revise e aprove antes de iniciar.",
            );
        });
    }
    {
        let sender = sender.clone();
        let weak = ui.as_weak();
        ui.on_refresh(move || {
            if let Some(ui) = weak.upgrade() {
                ui.set_inventory_loading(true);
            }
            let sender = sender.clone();
            send_inventory(sender);
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_open_tweak(move |index| {
            if let Some(ui) = weak.upgrade()
                && let Some(uri) = tweaks::TWEAKS.get(index as usize).and_then(|t| t.uri)
            {
                message(
                    &ui,
                    security::open_official(uri),
                    "Configurações abertas. Revise e aplique a opção desejada no Windows.",
                );
            }
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_open_source(move |index| {
            if let Some(ui) = weak.upgrade()
                && let Some(url) = catalog::APPS
                    .iter()
                    .filter(|a| !a.browser)
                    .nth(index as usize)
                    .and_then(|a| a.official)
            {
                message(&ui, security::open_official(url), "Fonte oficial aberta.");
            }
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_open_atlas(move || {
            if let Some(ui) = weak.upgrade() {
                message(
                    &ui,
                    security::open_official(atlas::GUIDE),
                    "Guia oficial aberto. Leia requisitos, backup e limites de segurança.",
                );
            }
        });
    }
    {
        let weak = ui.as_weak();
        let info = info.clone();
        ui.on_open_gpu(move |index| {
            if let Some(ui) = weak.upgrade() {
                let result = info.borrow().gpus.get(index as usize).and_then(drivers::official_url)
                    .ok_or_else(||crate::fail("Fabricante não identificado. Consulte o suporte do computador."))
                    .and_then(security::open_official);
                message(&ui, result, "Fonte oficial aberta. Você acompanha a instalação; nenhum download foi iniciado pelo Ruhtweaks.");
            }
        });
    }
    {
        let weak = ui.as_weak();
        let store = store.clone();
        ui.on_open_amd_downloads(move || {
            if let Some(ui) = weak.upgrade() { message(&ui, security::open_amd_downloads(&store), "Pasta aberta. Inicie o instalador AMD, escolha Recommended e revise os componentes. A instalação ainda não foi confirmada."); }
        });
    }
    {
        let weak = ui.as_weak();
        let store = store.clone();
        let info = info.clone();
        let cancel = cancel.clone();
        let sender = sender.clone();
        let active = active.clone();
        ui.on_prepare_amd_tool(move || {
            let Some(ui) = weak.upgrade() else { return };
            if active.swap(true, Ordering::Relaxed) {
                return;
            }
            ui.set_busy(true);
            cancel.store(false, Ordering::Relaxed);
            let (store, info, cancel, sender) = (
                store.clone(),
                info.borrow().clone(),
                cancel.clone(),
                sender.clone(),
            );
            thread::spawn(move || {
                let result = drivers::prepare(&info, &store, &cancel, &mut |line| {
                    tracing::info!("{line}");
                });
                let (result, text) = match result {
                    Ok((_, detail)) => (Ok(()), detail),
                    Err(error) => (Err(error), String::new()),
                };
                let _ = sender.send(UiEvent::Operation(result, text));
            });
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_open_browser(move || {
            if let Some(ui) = weak.upgrade() {
                let url = browser::help_url(
                    Browser::from_index(ui.get_browser_index()).unwrap_or(Browser::Brave),
                );
                message(&ui, security::open_official(url), "Ajuda oficial aberta.");
            }
        });
    }
    for kind in 0..3 {
        let weak = ui.as_weak();
        let store = store.clone();
        let sender = sender.clone();
        let active = active.clone();
        let callback = move || {
            let Some(ui) = weak.upgrade() else { return };
            if active.swap(true, Ordering::Relaxed) {
                return;
            }
            ui.set_busy(true);
            let sender = sender.clone();
            let store = store.clone();
            thread::spawn(move || {
                let result = match kind {
                    0 => tweaks::power::rollback_with(&mut tweaks::power::NativePower, &store),
                    1 => browser::rollback(&store),
                    _ => tweaks::rollback_extensions(&store),
                };
                send_extensions(&sender);
                let _ = sender.send(UiEvent::Operation(result, "Reversão executada e estado relido. Confira o resultado; nenhuma instalação foi iniciada.".into()));
            });
        };
        match kind {
            0 => ui.on_rollback_power(callback),
            1 => ui.on_rollback_browser(callback),
            _ => ui.on_rollback(callback),
        }
    }
    {
        let weak = ui.as_weak();
        let store = store.clone();
        let journal = journal.clone();
        ui.on_navigate(move |page| {
            if let Some(ui) = weak.upgrade() {
                if !(0..=8).contains(&page) || (ui.get_busy() && page != 7) {
                    return;
                }
                let mut nav = read_navigation(&ui);
                nav.visit(page);
                message(
                    &ui,
                    navigate(&ui, &store, nav),
                    "Suas escolhas ficam salvas neste PC.",
                );
                if page == 8
                    && let Some(j) = journal.borrow().as_ref()
                {
                    ui.set_details(j.report().into());
                }
            }
        });
    }
    {
        let weak = ui.as_weak();
        let store = store.clone();
        let journal = journal.clone();
        ui.on_primary(move || {
            let Some(ui) = weak.upgrade() else { return };
            let mut nav = read_navigation(&ui);
            let all_done = journal.borrow().as_ref().is_some_and(|j| {
                !j.tasks.is_empty() && j.tasks.iter().all(|t| t.status == Status::Completed)
            });
            match nav.primary(
                !ui.get_inventory_loading(),
                ui.get_plan_ready(),
                ui.get_busy(),
                all_done,
            ) {
                Next::Navigate => message(
                    &ui,
                    navigate(&ui, &store, nav),
                    "Suas escolhas estão salvas. Continue no seu ritmo.",
                ),
                Next::Review => ui.invoke_prepare(),
                Next::Execute => ui.invoke_execute(),
                Next::Cancel => ui.invoke_cancel(),
                Next::Wait => {}
            }
        });
    }
    {
        let weak = ui.as_weak();
        let store = store.clone();
        ui.on_back(move || {
            if let Some(ui) = weak.upgrade() {
                if ui.get_busy() {
                    return;
                }
                let mut nav = read_navigation(&ui);
                nav.back();
                message(&ui, navigate(&ui, &store, nav), "Seleção preservada.");
            }
        });
    }
    {
        let weak = ui.as_weak();
        let store = store.clone();
        ui.on_jump_step(move |step| {
            if let Some(ui) = weak.upgrade() {
                let mut nav = read_navigation(&ui);
                if ui.get_busy() || !(0..=nav.step).contains(&step) {
                    return;
                }
                nav.step = step;
                nav.page = navigation::step_page(step);
                message(&ui, navigate(&ui, &store, nav), "Seleção preservada.");
            }
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_finish(move || {
            if let Some(ui) = weak.upgrade()
                && !ui.get_busy()
            {
                let _ = ui.hide();
            }
        });
    }
    {
        let weak = ui.as_weak();
        let cancel = cancel.clone();
        ui.window().on_close_requested(move||{if active.load(Ordering::Relaxed){cancel.store(true,Ordering::Relaxed);if let Some(ui)=weak.upgrade(){if ui.get_external_wait(){return slint::CloseRequestResponse::HideWindow;}ui.set_status("Aguarde a operação atual terminar para fechar. Cancelamento da fila solicitado.".into());}slint::CloseRequestResponse::KeepWindowShown}else{slint::CloseRequestResponse::HideWindow}});
    }
    handoff::bind(
        &ui,
        handoff::Context {
            store: store.clone(),
            apps,
            tweaks,
            journal,
            authorized,
            info,
            active: close_active,
            cancel,
            sender,
        },
    );
    ui.invoke_worker_wakeup();
    ui.run().map_err(|e| crate::fail(e.to_string()))
}
enum UiEvent {
    Info(SystemInfo),
    Extensions(String),
    Power(String),
    Updates(Result<Vec<crate::state::UpdateRequest>>),
    Journal(Journal),
    Finished(Result<()>),
    Operation(Result<()>, String),
    Prepared(Result<Option<installer::handoff::Prepared>>),
}
#[derive(Clone)]
struct UiSender {
    weak: slint::Weak<MainWindow>,
    inbox: Arc<std::sync::Mutex<dispatch::Inbox<UiEvent>>>,
    inventory_running: Arc<AtomicBool>,
}
impl UiSender {
    fn new(weak: slint::Weak<MainWindow>) -> Self {
        Self {
            weak,
            inbox: Arc::default(),
            inventory_running: Arc::new(AtomicBool::new(false)),
        }
    }
    fn send(&self, event: UiEvent) -> std::result::Result<(), slint::EventLoopError> {
        let notify = self
            .inbox
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .post(event, |old, new| {
                matches!((old, new), (UiEvent::Journal(_), UiEvent::Journal(_)))
            });
        if notify
            && let Err(error) = self
                .weak
                .upgrade_in_event_loop(|ui| ui.invoke_worker_wakeup())
        {
            self.inbox
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .unschedule();
            return Err(error);
        }
        Ok(())
    }
}

fn send_inventory(sender: UiSender) {
    if sender.inventory_running.swap(true, Ordering::Relaxed) {
        return;
    }
    thread::spawn(move || {
        struct Reset(Arc<AtomicBool>);
        impl Drop for Reset {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Relaxed);
            }
        }
        let _reset = Reset(sender.inventory_running.clone());
        send_extensions(&sender);
        let _ = sender.send(UiEvent::Info(system_info::collect()));
    });
}
fn send_prepared(sender: &UiSender, store: &Store, journal: &Journal) {
    let result = (|| -> Result<_> {
        for task in &journal.tasks {
            if !matches!(
                task.status,
                Status::RequiresConfirmation
                    | Status::ExternalAction
                    | Status::AwaitingVerification
            ) {
                continue;
            }
            let kind = match &task.action {
                crate::state::Action::Atlas => installer::handoff::Kind::Atlas,
                crate::state::Action::Install(key) if key == "litecord" => {
                    installer::handoff::Kind::Litecord
                }
                _ => continue,
            };
            if let Some(file) = installer::handoff::load(store, kind)?
                && file.profile_hash == journal.profile_hash
            {
                file.validate_source()?;
                return Ok(Some(file));
            }
        }
        Ok(None)
    })();
    let _ = sender.send(UiEvent::Prepared(result));
}
fn send_extensions(sender: &UiSender) {
    use tweaks::power::PowerControl;
    let state = match tweaks::power::NativePower.current() {
        Ok(tweaks::power::BALANCED) => "Estado atual: plano Equilibrado.".into(),
        Ok(guid) => format!("Plano atual: {guid:032x}. Será preservado antes da alteração."),
        Err(e) => format!("Plano não consultado: {e}"),
    };
    let _ = sender.send(UiEvent::Power(state));
    let state = match tweaks::extensions_state() {
        Ok(Some(0)) => "Estado atual: extensões visíveis.".into(),
        Ok(Some(_)) => "Estado atual: extensões ocultas.".into(),
        Ok(None) => "Estado atual: preferência padrão do Explorer.".into(),
        Err(error) => format!("Estado não consultado: {error}"),
    };
    let _ = sender.send(UiEvent::Extensions(state));
}
fn update_rows(requests: &[crate::state::UpdateRequest], selected: bool) -> Vec<UpdateRow> {
    requests
        .iter()
        .map(|request| UpdateRow {
            browser: catalog::find(&request.key).is_some_and(|app| app.browser),
            key: request.key.clone().into(),
            name: catalog::find(&request.key)
                .map_or("App desconhecido", |app| app.name)
                .into(),
            from: request.from.clone().into(),
            to: request.to.clone().into(),
            selected,
        })
        .collect()
}

#[cfg(test)]
mod automatic_profile_tests {
    use super::*;
    use slint::platform::{
        Platform, PlatformError, WindowAdapter,
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
    };
    struct TestPlatform;
    impl Platform for TestPlatform {
        fn create_window_adapter(
            &self,
        ) -> std::result::Result<Rc<dyn WindowAdapter>, PlatformError> {
            Ok(MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer))
        }
    }
    #[test]
    fn automatic_review_matches_the_profile_read_by_execution_and_keeps_existing_tweaks() {
        slint::platform::set_platform(Box::new(TestPlatform)).unwrap();
        let ui = MainWindow::new().unwrap();
        let original = Profile {
            tweaks: vec!["balanced".into()],
            ..Profile::default()
        };
        let apps = Rc::new(VecModel::from(presentation::app_rows(&original)));
        let tweaks = Rc::new(VecModel::from(presentation::tweak_rows(
            &original,
            "Estado de teste",
        )));
        ui.set_apps(ModelRc::from(apps.clone()));
        ui.set_tweaks(ModelRc::from(tweaks.clone()));
        let info = SystemInfo {
            build: 26100,
            gpus: vec![system_info::Gpu {
                hardware_id: "PCI\\VEN_1002&DEV_744C".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let reviewed = prepare_automatic_profile(&ui, &apps, &tweaks, &info);
        let execution = read_profile(&ui, &apps, &tweaks);
        assert_eq!(reviewed, execution);
        assert_eq!(reviewed.tweaks, ["extensions", "balanced"]);
        assert!(ui.get_drivers_selected() && ui.get_google_selected());
        assert!(!reviewed.accept_terms && !reviewed.allow_startup && !reviewed.atlas);
        let plan = installer::plan(&reviewed, &info).unwrap();
        assert_eq!(plan.profile_hash, execution.fingerprint().unwrap());
        assert!(installer::approve_reviewed(&execution, &plan).is_ok());

        ui.set_atlas_selected(true);
        ui.set_browser_index(Browser::Firefox.index());
        ui.set_updates(ModelRc::new(VecModel::from(update_rows(
            &[crate::state::UpdateRequest {
                key: "firefox".into(),
                from: "1.0".into(),
                to: "2.0".into(),
            }],
            true,
        ))));
        let reviewed = prepare_automatic_profile(&ui, &apps, &tweaks, &info);
        assert_eq!(reviewed.browser, Some(Browser::Firefox));
        assert!(reviewed.updates.is_empty());
        let queue = installer::plan(&reviewed, &info).unwrap();
        assert!(!queue.tasks.iter().any(|task| task.action == crate::state::Action::Browser || matches!(&task.action, crate::state::Action::Install(key) if key == "firefox")));
        assert!(installer::approve_reviewed(&read_profile(&ui, &apps, &tweaks), &queue).is_ok());
    }
}
