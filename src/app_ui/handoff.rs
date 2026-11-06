use super::*;
pub(super) struct Context {
    pub store: Store,
    pub apps: Rc<VecModel<AppRow>>,
    pub tweaks: Rc<VecModel<TweakRow>>,
    pub journal: Rc<RefCell<Option<Journal>>>,
    pub authorized: Rc<RefCell<Option<String>>>,
    pub info: Rc<RefCell<SystemInfo>>,
    pub active: Arc<AtomicBool>,
    pub cancel: Arc<AtomicBool>,
    pub sender: UiSender,
}
pub(super) fn bind(ui: &MainWindow, context: Context) {
    let ctx = Rc::new(context);
    let completed = ctx.journal.borrow().as_ref().map(|j| j.tasks.iter().filter(|t| t.status == Status::Completed && (t.action == crate::state::Action::Atlas || matches!(&t.action, crate::state::Action::Install(key) if key == "litecord"))).cloned().collect::<Vec<_>>()).unwrap_or_default();
    if !completed.is_empty() {
        let store = ctx.store.clone();
        thread::spawn(move || {
            let mut executor = NativeExecutor::new(SystemInfo::default());
            for task in completed {
                if let Err(error) = installer::Executor::cleanup(&mut executor, &task, &store) {
                    tracing::warn!("Temporário mantido após retomada: {error}");
                }
            }
        });
    }
    let weak = ui.as_weak();
    ui.on_continue_without_atlas(move || {
        if let Some(ui) = weak.upgrade() && !ui.get_busy() {
            ui.set_atlas_selected(false); ui.invoke_changed(); ui.invoke_prepare();
            ui.set_status("Atlas removido do novo plano. A pendência anterior será preservada no histórico. Revise e aprove para continuar somente as demais tarefas.".into());
        }
    });
    if let Some(saved) = ctx.journal.borrow().as_ref() {
        send_prepared(&ctx.sender, &ctx.store, saved);
    }
    let weak = ui.as_weak();
    ui.on_confirm_handoff(move || {
        let Some(ui) = weak.upgrade() else { return };
        if !ui.get_handoff_ack()
            || !ui.get_handoff_allowed()
            || ctx.active.swap(true, Ordering::Relaxed)
        {
            return;
        }
        let result = (|| -> Result<_> {
            let profile = read_profile(&ui, &ctx.apps, &ctx.tweaks);
            let hash = profile.fingerprint()?;
            if ctx.authorized.borrow().as_ref() != Some(&hash) {
                return Err(crate::fail(
                    "Revise e aprove o plano nesta sessão antes da abertura.",
                ));
            }
            let kind = match ui.get_handoff_kind() {
                0 => installer::handoff::Kind::Litecord,
                1 => installer::handoff::Kind::Atlas,
                _ => return Err(crate::fail("Arquivo não selecionado")),
            };
            let file = installer::handoff::load(&ctx.store, kind)?
                .ok_or_else(|| crate::fail("Arquivo preparado ausente"))?;
            let consent = installer::handoff::Consent {
                kind,
                profile_hash: hash.clone(),
                digest: kind.digest().into(),
                acknowledge: true,
                silent: ui.get_handoff_silent(),
            };
            consent.validate(&file, &profile)?;
            let mut saved = ctx
                .journal
                .borrow()
                .clone()
                .ok_or_else(|| crate::fail("Fila ausente"))?;
            if saved.profile_hash != hash {
                return Err(crate::fail("A fila pertence a outro perfil"));
            }
            let task = saved
                .tasks
                .iter_mut()
                .find(|task| match &task.action {
                    crate::state::Action::Atlas => kind == installer::handoff::Kind::Atlas,
                    crate::state::Action::Install(key) => {
                        kind == installer::handoff::Kind::Litecord && key == "litecord"
                    }
                    _ => false,
                })
                .ok_or_else(|| crate::fail("Tarefa não selecionada no plano"))?;
            task.transition(
                Status::Pending,
                "Abertura específica confirmada nesta sessão; revalidar arquivo antes de executar.",
            )?;
            Ok((profile, saved, consent))
        })();
        let (profile, mut saved, consent) = match result {
            Ok(value) => value,
            Err(e) => {
                ctx.active.store(false, Ordering::Relaxed);
                message(&ui, Err(e), "");
                return;
            }
        };
        ui.set_busy(true);
        ui.set_handoff_ack(false);
        ui.set_handoff_allowed(false);
        ctx.cancel.store(false, Ordering::Relaxed);
        let sender = ctx.sender.clone();
        let store = ctx.store.clone();
        let cancel = ctx.cancel.clone();
        let info = ctx.info.borrow().clone();
        thread::spawn(move || {
            let result = (|| -> Result<()> {
                store.save_journal(&saved)?;
                let mut executor = NativeExecutor::new(info);
                executor.grant_confirmation(consent);
                executor.approve_litecord(&profile);
                installer::execute_plan(&mut executor, &profile, &mut saved, &store, &cancel, |j| {
                    let _ = sender.send(UiEvent::Journal(j.clone()));
                })
            })();
            send_prepared(&sender, &store, &saved);
            send_extensions(&sender);
            let _ = sender.send(UiEvent::Finished(result));
        });
    });
}
