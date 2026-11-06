use ruhtweaks::{
    Result, atlas,
    catalog::Browser,
    drivers,
    installer::{self, Detection, Executor},
    security,
    state::{Action, Journal, Profile, SCHEMA, Status, Store, Task},
    system_info::{Gpu, SystemInfo},
};
use std::sync::atomic::{AtomicBool, Ordering};

fn info() -> SystemInfo {
    SystemInfo {
        os: "Windows 11".into(),
        build: 26200,
        edition: "Professional".into(),
        architecture: "x86_64 / 64 bits".into(),
        ..Default::default()
    }
}
fn profile() -> Profile {
    Profile {
        apps: vec!["steam".into(), "epic".into()],
        ..Profile::default()
    }
}
struct Mock {
    detect: Detection,
    calls: usize,
    fail: bool,
}
impl Executor for Mock {
    fn detect(&mut self, _: &str) -> Result<Detection> {
        Ok(self.detect.clone())
    }
    fn execute(
        &mut self,
        _: &Task,
        _: &Profile,
        _: &Store,
        _: &AtomicBool,
        _: &mut dyn FnMut(String),
    ) -> Result<(Status, String)> {
        self.calls += 1;
        if self.fail {
            Err(ruhtweaks::fail("falha simulada"))
        } else {
            Ok((Status::Completed, "Instalação simulada e verificada".into()))
        }
    }
}
fn mock() -> Mock {
    Mock {
        detect: Detection::Absent,
        calls: 0,
        fail: false,
    }
}
#[test]
fn proposed_profile_requires_approval() {
    let p = Profile::default();
    assert!(!p.accept_terms);
    assert!(!p.allow_startup);
    assert!(!p.atlas);
    assert_eq!(p.browser, Some(Browser::Brave));
    for key in ["litecord", "steam", "epic", "riot"] {
        assert!(p.apps.contains(&key.into()))
    }
}
#[test]
fn plan_deduplicates() {
    let mut p = profile();
    p.apps.push("steam".into());
    let j = installer::plan(&p, &info()).unwrap();
    assert_eq!(j.tasks.len(), 3);
}
#[test]
fn single_browser() {
    let p = Profile {
        browser: Some(Browser::Firefox),
        ..profile()
    };
    let j = installer::plan(&p, &info()).unwrap();
    assert!(j.tasks.iter().any(|t| t.id == "app:firefox"));
    assert!(!j.tasks.iter().any(|t| t.id == "app:brave"));
}
#[test]
fn atlas_delegates_browser_install_settings_and_updates_for_both_adapters() {
    for legacy in [false, true] {
        for browser in [
            Browser::Brave,
            Browser::Firefox,
            Browser::Chrome,
            Browser::Edge,
        ] {
            let mut p = Profile {
                atlas: true,
                atlas_legacy: legacy,
                browser: Some(browser),
                google_search: true,
                apps: vec!["7zip".into(), "steam".into()],
                updates: vec![
                    ruhtweaks::state::UpdateRequest {
                        key: browser.key().into(),
                        from: "1.0".into(),
                        to: "2.0".into(),
                    },
                    ruhtweaks::state::UpdateRequest {
                        key: "7zip".into(),
                        from: "1.0".into(),
                        to: "2.0".into(),
                    },
                ],
                ..Profile::default()
            };
            p.include_automatic_setup(&info());
            assert_eq!(
                p.browser,
                Some(browser),
                "preferência deve permanecer para desistir do Atlas"
            );
            assert!(p.standalone_browser().is_none());
            let queue = installer::plan(&p, &info()).unwrap();
            assert!(queue.tasks.iter().any(|task| task.action == Action::Atlas));
            assert!(queue.tasks.iter().any(|task| task.id == "upgrade:7zip"));
            assert!(queue.tasks.iter().any(|task| task.id == "app:steam"));
            assert!(!queue.tasks.iter().any(|task| match &task.action {
                Action::Install(key) =>
                    ruhtweaks::catalog::find(key).is_some_and(|app| app.browser),
                Action::Upgrade(request) =>
                    ruhtweaks::catalog::find(&request.key).is_some_and(|app| app.browser),
                Action::Browser => true,
                _ => false,
            }));
        }
    }
}
#[test]
fn declining_atlas_restores_the_saved_browser_choice_without_resetting_preferences() {
    let mut p = Profile {
        atlas: true,
        browser: Some(Browser::Firefox),
        google_search: true,
        ..Profile::default()
    };
    let copy = ruhtweaks::state::decode_profile(serde_json::to_value(&p).unwrap()).unwrap();
    assert_eq!(copy, p);
    assert!(p.standalone_browser().is_none());
    p.atlas = false;
    assert_eq!(p.standalone_browser(), Some(Browser::Firefox));
    let queue = installer::plan(&p, &info()).unwrap();
    assert!(queue.tasks.iter().any(|task| task.id == "app:firefox"));
    assert!(
        queue
            .tasks
            .iter()
            .any(|task| task.action == Action::Browser)
    );
    assert!(!queue.tasks.iter().any(|task| task.action == Action::Atlas));
}
#[test]
fn atlas_refuses_a_stale_queue_that_would_install_a_second_browser() {
    let p = Profile {
        atlas: true,
        ..Profile::default()
    };
    let mut queue = installer::plan(&p, &info()).unwrap();
    let standalone = installer::plan(&Profile::default(), &info()).unwrap();
    queue.tasks.push(
        standalone
            .tasks
            .into_iter()
            .find(|task| task.id == "app:brave")
            .unwrap(),
    );
    assert!(installer::approve_reviewed(&p, &queue).is_err());
}
#[test]
fn discord_available() {
    assert_eq!(
        ruhtweaks::catalog::find("discord").unwrap().winget,
        Some("Discord.Discord")
    );
}
#[test]
fn litecord_identity_is_explicit() {
    assert_eq!(
        ruhtweaks::catalog::find("litecord").unwrap().official,
        Some("https://github.com/Ak4ai/Litecord/releases")
    );
}
#[test]
fn unknown_apps_rejected() {
    let p = Profile {
        apps: vec!["powershell -evil".into()],
        ..profile()
    };
    assert!(p.validate().is_err());
}
#[test]
fn browser_in_apps_rejected() {
    let p = Profile {
        apps: vec!["brave".into()],
        ..profile()
    };
    assert!(p.validate().is_err());
}
#[test]
fn future_profile_not_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store {
        root: dir.path().into(),
    };
    let mut p = profile();
    p.schema_version = 999;
    assert!(store.save_profile(&p).is_err());
    assert!(!store.root.join("profile.json").exists());
}
#[test]
fn legacy_atlas_requires_selection() {
    let p = Profile {
        atlas_legacy: true,
        ..profile()
    };
    assert!(p.validate().is_err());
    assert!(Profile { atlas: true, ..p }.validate().is_ok());
}
#[test]
fn unknown_tweak_rejected() {
    assert!(
        Profile {
            tweaks: vec!["disable_defender".into()],
            ..profile()
        }
        .validate()
        .is_err()
    );
}
#[test]
fn atlas_supported_builds() {
    for build in [26100, 26200] {
        assert!(atlas::compatibility(&SystemInfo { build, ..info() }).is_ok())
    }
}
#[test]
fn atlas_rejects_old_and_new_builds() {
    for build in [19045, 22631, 28000, 0] {
        assert!(atlas::compatibility(&SystemInfo { build, ..info() }).is_err())
    }
}
#[test]
fn atlas_rejects_home_and_unknown_edition() {
    for edition in ["Core", "CoreSingleLanguage", ""] {
        assert!(
            atlas::compatibility(&SystemInfo {
                edition: edition.into(),
                ..info()
            })
            .is_err()
        )
    }
}
#[test]
fn atlas_rejects_arm() {
    assert!(
        atlas::compatibility(&SystemInfo {
            architecture: "aarch64".into(),
            ..info()
        })
        .is_err()
    )
}
#[test]
fn atlas_rejects_unreviewed_release() {
    let r = atlas::Release {
        tag_name: "future".into(),
        html_url: "https://github.com/Atlas-OS/Atlas".into(),
        assets: vec![],
    };
    assert!(atlas::verified_playbook(&r).is_err());
}
#[test]
fn hardware_id_routes_each_vendor() {
    for (id, host) in [
        ("PCI\\VEN_1002&DEV_1", "amd.com"),
        ("PCI\\VEN_10DE&DEV_2", "nvidia.com"),
        ("PCI\\VEN_8086&DEV_3", "intel.com"),
    ] {
        assert!(
            drivers::official_url(&Gpu {
                hardware_id: id.into(),
                ..Default::default()
            })
            .unwrap()
            .contains(host)
        );
    }
    assert!(drivers::official_url(&Gpu::default()).is_none());
}
#[test]
fn exact_detection_only() {
    assert_eq!(
        installer::parse_detection(0, "Name Id Version\nSteam Valve.Steam 1.0", "Valve.Steam"),
        Detection::Present
    );
    assert!(matches!(
        installer::parse_detection(0, "Valve.SteamHelper 1.0", "Valve.Steam"),
        Detection::Unknown(_)
    ));
    assert!(matches!(
        installer::parse_detection(-1, "Valve.Steam", "Valve.Steam"),
        Detection::Unknown(_)
    ));
}
#[test]
fn absent_is_explicit_hresult() {
    assert_eq!(
        installer::parse_detection(0x8A150014u32 as i32, "", "Valve.Steam"),
        Detection::Absent
    );
}
#[test]
fn completed_is_terminal() {
    assert!(!Status::Completed.permits(Status::Pending));
    assert!(!Status::Pending.permits(Status::Completed));
    assert!(Status::Installing.permits(Status::Failed));
}
#[test]
fn recovery_requires_revalidation() {
    let mut j = installer::plan(&profile(), &info()).unwrap();
    j.tasks[0].status = Status::Installing;
    j.recover();
    assert_eq!(j.tasks[0].status, Status::RequiresAction);
    assert_eq!(j.tasks[1].status, Status::Pending);
}
#[test]
fn atomic_profile_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store {
        root: dir.path().into(),
    };
    let mut p = profile();
    store.save_profile(&p).unwrap();
    p.browser = None;
    store.save_profile(&p).unwrap();
    assert_eq!(store.load_profile().unwrap(), p);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
#[test]
fn corrupt_profile_is_not_reset() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store {
        root: dir.path().into(),
    };
    std::fs::write(dir.path().join("profile.json"), "invalid").unwrap();
    assert!(store.load_profile().is_err());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("profile.json")).unwrap(),
        "invalid"
    );
}
#[test]
fn lock_prevents_two_writers() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store {
        root: dir.path().into(),
    };
    let first = store.lock().unwrap();
    assert!(store.lock().is_err());
    drop(first);
    assert!(store.lock().is_ok());
}
#[test]
fn mock_execution_persists_only_verified_completion() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store {
        root: dir.path().into(),
    };
    let p = profile();
    let mut j = installer::plan(&p, &info()).unwrap();
    let mut e = mock();
    installer::execute_plan(&mut e, &p, &mut j, &store, &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(e.calls, 3);
    assert!(j.tasks.iter().all(|t| t.status == Status::Completed));
    assert!(store.root.join("report.txt").is_file());
}
#[test]
fn existing_apps_are_never_reinstalled() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store {
        root: dir.path().into(),
    };
    let p = profile();
    let mut j = installer::plan(&p, &info()).unwrap();
    let mut e = Mock {
        detect: Detection::Present,
        ..mock()
    };
    installer::execute_plan(&mut e, &p, &mut j, &store, &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(e.calls, 0);
}
#[test]
fn unknown_detection_blocks_install() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store {
        root: dir.path().into(),
    };
    let p = profile();
    let mut j = installer::plan(&p, &info()).unwrap();
    let mut e = Mock {
        detect: Detection::Unknown("offline".into()),
        ..mock()
    };
    installer::execute_plan(&mut e, &p, &mut j, &store, &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(e.calls, 0);
    assert!(j.tasks.iter().all(|t| t.status == Status::RequiresAction));
}
#[test]
fn cancel_before_start_does_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store {
        root: dir.path().into(),
    };
    let p = profile();
    let mut j = installer::plan(&p, &info()).unwrap();
    let mut e = mock();
    installer::execute_plan(&mut e, &p, &mut j, &store, &AtomicBool::new(true), |_| {}).unwrap();
    assert_eq!(e.calls, 0);
    assert!(j.tasks.iter().all(|t| t.status == Status::Cancelled));
}
#[test]
fn cancellation_does_not_kill_current_install() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store {
        root: dir.path().into(),
    };
    let p = profile();
    let mut j = installer::plan(&p, &info()).unwrap();
    let mut e = mock();
    let cancel = AtomicBool::new(false);
    installer::execute_plan(&mut e, &p, &mut j, &store, &cancel, |j| {
        if j.tasks[0].status == Status::Installing {
            cancel.store(true, Ordering::Relaxed)
        }
    })
    .unwrap();
    assert_eq!(j.tasks[0].status, Status::Completed);
    assert_eq!(e.calls, 1);
    assert_eq!(j.tasks[1].status, Status::Cancelled);
}
#[test]
fn completed_operations_not_repeated() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store {
        root: dir.path().into(),
    };
    let p = profile();
    let mut j = installer::plan(&p, &info()).unwrap();
    j.tasks[0].status = Status::Completed;
    let mut e = mock();
    installer::execute_plan(&mut e, &p, &mut j, &store, &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(e.calls, 2);
}
#[test]
fn changed_profile_cannot_reuse_approval() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store {
        root: dir.path().into(),
    };
    let p = profile();
    let mut j = installer::plan(&p, &info()).unwrap();
    let mut changed = p;
    changed.drivers = true;
    let mut e = mock();
    assert!(
        installer::execute_plan(
            &mut e,
            &changed,
            &mut j,
            &store,
            &AtomicBool::new(false),
            |_| {}
        )
        .is_err()
    );
    assert_eq!(e.calls, 0);
}
#[test]
fn failure_is_persisted_and_not_success() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store {
        root: dir.path().into(),
    };
    let p = profile();
    let mut j = installer::plan(&p, &info()).unwrap();
    let mut e = Mock {
        fail: true,
        ..mock()
    };
    installer::execute_plan(&mut e, &p, &mut j, &store, &AtomicBool::new(false), |_| {}).unwrap();
    let saved = store.load_journal().unwrap().unwrap();
    assert!(saved.tasks.iter().all(|t| t.status == Status::Failed));
}
#[test]
fn atlas_is_a_barrier() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store {
        root: dir.path().into(),
    };
    let p = Profile {
        atlas: true,
        ..profile()
    };
    let mut j = installer::plan(&p, &info()).unwrap();
    j.tasks[0].status = Status::RequiresAction;
    let mut e = mock();
    installer::execute_plan(&mut e, &p, &mut j, &store, &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(e.calls, 0);
    assert!(matches!(j.tasks[0].action, Action::Atlas));
}
#[test]
fn resume_can_confirm_app_without_reinstall() {
    let p = profile();
    let mut j = installer::plan(&p, &info()).unwrap();
    j.tasks[0].status = Status::RequiresAction;
    let mut e = Mock {
        detect: Detection::Present,
        ..mock()
    };
    installer::revalidate(&mut e, &mut j).unwrap();
    assert_eq!(j.tasks[0].status, Status::Completed);
    assert_eq!(e.calls, 0);
}
#[test]
fn digest_format_is_strict() {
    for bad in ["", "sha1:abc", "sha256:bad", "sha256:../../evil"] {
        assert!(security::validate_digest(bad).is_err())
    }
    assert!(security::validate_digest(&format!("sha256:{}", "a".repeat(64))).is_ok());
}
#[test]
fn hash_computes_known_sha256() {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), b"abc").unwrap();
    assert_eq!(
        security::digest_file(file.path()).unwrap(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}
#[test]
fn untrusted_download_is_rejected_before_network() {
    let dir = tempfile::tempdir().unwrap();
    let result = security::download_verified(
        "https://evil.example/installer.exe",
        &format!("sha256:{}", "a".repeat(64)),
        &dir.path().join("a.exe"),
        &AtomicBool::new(false),
        |_, _| {},
    );
    assert!(result.is_err());
}
#[test]
fn shell_injection_uri_rejected() {
    for url in [
        "cmd.exe /c del",
        "http://github.com/Atlas-OS",
        "https://github.com.evil.example/",
        "file:///C:/Windows/System32/cmd.exe",
        "ms-settings:unknown",
    ] {
        assert!(security::open_official(url).is_err())
    }
}
#[test]
fn journal_schema_checked() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store {
        root: dir.path().into(),
    };
    let j = Journal {
        schema_version: SCHEMA + 1,
        profile_hash: String::new(),
        tasks: vec![],
    };
    store.save_journal(&j).unwrap();
    assert!(store.load_journal().is_err());
}

#[test]
fn reviewing_same_profile_preserves_completed_tasks() {
    let p = profile();
    let mut old = installer::plan(&p, &info()).unwrap();
    old.tasks[0].status = Status::Completed;
    let fresh = installer::review_plan(&p, &info(), Some(&old)).unwrap();
    assert_eq!(fresh.tasks[0].status, Status::Completed);
}
#[test]
fn journal_cannot_inject_unselected_action() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store {
        root: dir.path().into(),
    };
    let p = profile();
    let mut j = installer::plan(&p, &info()).unwrap();
    j.tasks[0].action = Action::Tweak("extensions".into());
    let mut e = mock();
    assert!(
        installer::execute_plan(&mut e, &p, &mut j, &store, &AtomicBool::new(false), |_| {})
            .is_err()
    );
    assert_eq!(e.calls, 0);
}
