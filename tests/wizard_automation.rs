use ruhtweaks::{
    Result,
    drivers::amd_update::{self, Backend, Candidate, Device},
    installer,
    state::{Profile, Status, Store},
    system_info::SystemInfo,
};
use std::sync::atomic::{AtomicBool, Ordering};
fn candidate() -> Candidate {
    Candidate {
        id: "12345678-1234-1234-1234-123456789abc".into(),
        revision: 3,
        title: "AMD Display".into(),
        hardware_id: "PCI\\VEN_1002&DEV_744C&SUBSYS_0001".into(),
        class: "Display".into(),
        manufacturer: "Advanced Micro Devices, Inc.".into(),
        requires_ui: false,
    }
}
#[test]
fn automatic_setup_is_hardware_scoped_idempotent_and_preserves_approval_boundaries() {
    let mut info = SystemInfo {
        build: 26100,
        architecture: "x86_64".into(),
        edition: "Professional".into(),
        gpus: vec![ruhtweaks::system_info::Gpu {
            hardware_id: "PCI\\VEN_1002&DEV_744C".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut profile = Profile::default();
    let original_apps = profile.apps.clone();
    profile.include_automatic_setup(&info);
    let once = profile.clone();
    profile.include_automatic_setup(&info);
    assert_eq!(profile, once);
    assert!(profile.drivers && profile.google_search);
    assert_eq!(profile.tweaks, ["extensions"]);
    assert_eq!(profile.apps, original_apps);
    assert!(
        !profile.atlas && !profile.atlas_legacy && !profile.accept_terms && !profile.allow_startup
    );
    assert!(profile.updates.is_empty());
    let plan = installer::plan(&profile, &info).unwrap();
    for key in ["drivers", "browser", "tweak:extensions"] {
        assert_eq!(plan.tasks.iter().filter(|task| task.id == key).count(), 1);
    }
    assert!(
        !plan
            .tasks
            .iter()
            .any(|task| task.id == "tweak:balanced" || task.id == "tweak:storage")
    );

    for hardware in [
        "PCI\\VEN_10DE&DEV_1234",
        "PCI\\VEN_8086&DEV_1234",
        "FAKE_VEN_1002",
    ] {
        info.gpus[0].hardware_id = hardware.into();
        let mut other = Profile {
            browser: None,
            ..Profile::default()
        };
        other.include_automatic_setup(&info);
        assert!(!other.drivers && !other.google_search);
        assert_eq!(other.tweaks, ["extensions"]);
    }
    let mut unknown = Profile::default();
    let untouched = unknown.clone();
    unknown.include_automatic_setup(&SystemInfo::default());
    assert_eq!(unknown, untouched);
}
#[test]
fn automatic_setup_does_not_repeat_completed_tasks_or_replace_saved_manual_choices() {
    let info = SystemInfo {
        build: 26100,
        ..Default::default()
    };
    let mut profile = Profile {
        tweaks: vec!["balanced".into()],
        ..Profile::default()
    };
    profile.include_automatic_setup(&info);
    assert_eq!(profile.tweaks, ["balanced", "extensions"]);
    let mut journal = installer::plan(&profile, &info).unwrap();
    for task in &mut journal.tasks {
        task.status = Status::Completed;
    }
    profile.include_automatic_setup(&info);
    let resumed = installer::review_plan(&profile, &info, Some(&journal)).unwrap();
    assert_eq!(resumed.profile_hash, journal.profile_hash);
    assert!(
        resumed
            .tasks
            .iter()
            .all(|task| task.status == Status::Completed)
    );
}

#[test]
fn one_approved_plan_executes_the_automatic_tasks_with_a_mock_backend() {
    struct AutomaticMock(Vec<String>);
    impl installer::Executor for AutomaticMock {
        fn detect(&mut self, _: &str) -> Result<installer::Detection> {
            Ok(installer::Detection::Present)
        }
        fn execute(
            &mut self,
            task: &ruhtweaks::state::Task,
            profile: &Profile,
            _: &Store,
            _: &AtomicBool,
            _: &mut dyn FnMut(String),
        ) -> Result<(Status, String)> {
            assert!(profile.accept_terms);
            self.0.push(task.id.clone());
            Ok((
                Status::Completed,
                "Executor simulado; nenhuma alteração no host.".into(),
            ))
        }
    }
    let info = SystemInfo {
        build: 26100,
        gpus: vec![ruhtweaks::system_info::Gpu {
            hardware_id: "PCI\\VEN_1002&DEV_744C".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut profile = Profile {
        apps: vec![],
        ..Profile::default()
    };
    profile.include_automatic_setup(&info);
    let journal = installer::plan(&profile, &info).unwrap();
    assert!(!profile.accept_terms);
    let (profile, mut journal) = installer::approve_reviewed(&profile, &journal).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let store = Store {
        root: temp.path().into(),
    };
    let mut executor = AutomaticMock(vec![]);
    installer::execute_plan(
        &mut executor,
        &profile,
        &mut journal,
        &store,
        &installer::cancellation(),
        |_| {},
    )
    .unwrap();
    assert_eq!(executor.0, ["drivers", "browser", "tweak:extensions"]);
    assert!(
        journal
            .tasks
            .iter()
            .all(|task| task.status == Status::Completed)
    );
    installer::execute_plan(
        &mut executor,
        &profile,
        &mut journal,
        &store,
        &installer::cancellation(),
        |_| {},
    )
    .unwrap();
    assert_eq!(
        executor.0.len(),
        3,
        "retomada não deve repetir os ajustes concluídos"
    );
}
#[test]
fn suggested_atlas_is_selected_only_on_reviewed_systems_without_approving_execution() {
    let modern = SystemInfo {
        architecture: "x86_64".into(),
        edition: "Professional".into(),
        build: 26100,
        ..SystemInfo::default()
    };
    let profile = Profile::suggested_for(&modern);
    assert!(profile.atlas && !profile.atlas_legacy && !profile.accept_terms);
    assert_eq!(profile.apps, Profile::default().apps);
    let legacy = SystemInfo {
        build: 19045,
        ..modern.clone()
    };
    let profile = Profile::suggested_for(&legacy);
    assert!(profile.atlas && profile.atlas_legacy && !profile.accept_terms);
    for incompatible in [
        SystemInfo {
            edition: "Core".into(),
            ..modern.clone()
        },
        SystemInfo {
            architecture: "aarch64".into(),
            ..modern.clone()
        },
        SystemInfo {
            build: 22000,
            ..modern.clone()
        },
        SystemInfo::default(),
    ] {
        let profile = Profile::suggested_for(&incompatible);
        assert!(!profile.atlas && !profile.atlas_legacy && !profile.accept_terms);
    }
}
#[test]
fn saved_atlas_opt_out_is_preserved_on_reload() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store {
        root: dir.path().into(),
    };
    let info = SystemInfo {
        architecture: "x86_64".into(),
        edition: "Professional".into(),
        build: 26100,
        ..SystemInfo::default()
    };
    let mut profile = Profile::suggested_for(&info);
    profile.atlas = false;
    profile.atlas_legacy = false;
    store.save_profile(&profile).unwrap();
    assert_eq!(store.load_profile().unwrap(), profile);
    assert!(!store.load_profile().unwrap().atlas);
}
fn device() -> Device {
    Device {
        instance: "PCI\\VEN_1002&DEV_744C\\INSTANCE".into(),
        ids: vec![candidate().hardware_id],
    }
}
struct Mock {
    offers: Vec<Candidate>,
    calls: usize,
    code: u32,
    present: bool,
}
impl Backend for Mock {
    fn search(&mut self) -> Result<Vec<Candidate>> {
        Ok(self.offers.clone())
    }
    fn install(&mut self, _: &[Candidate]) -> Result<u32> {
        self.calls += 1;
        Ok(self.code)
    }
    fn installed(&mut self, _: &[Candidate]) -> Result<bool> {
        Ok(self.present)
    }
}
fn api() -> Mock {
    Mock {
        offers: vec![candidate()],
        calls: 0,
        code: 0,
        present: true,
    }
}
#[test]
fn amd_requires_exact_display_hardware_and_manufacturer() {
    let original = candidate();
    for field in 0..6 {
        let mut wrong = original.clone();
        match field {
            0 => wrong.class = "System".into(),
            1 => wrong.manufacturer = "Other".into(),
            2 => wrong.hardware_id = "PCI\\VEN_1002&DEV_OTHER".into(),
            3 => wrong.requires_ui = true,
            4 => wrong.id = "id' injection".into(),
            _ => wrong.revision = -1,
        }
        assert!(
            amd_update::select(&[wrong], &[device()])
                .unwrap()
                .is_empty()
        );
    }
    assert_eq!(
        amd_update::select(std::slice::from_ref(&original), &[device()]).unwrap(),
        vec![original]
    );
}
#[test]
fn amd_refuses_ambiguous_updates_and_deduplicates_identical_offers() {
    let first = candidate();
    let mut second = first.clone();
    second.revision = 4;
    assert!(amd_update::select(&[first.clone(), second], &[device()]).is_err());
    assert_eq!(
        amd_update::select(&[first.clone(), first], &[device()])
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn amd_mixed_gpus_and_different_hardware_do_not_get_the_same_package() {
    let wrong = Device {
        instance: "OTHER".into(),
        ids: vec!["PCI\\VEN_10DE&DEV_1234".into()],
    };
    assert!(
        amd_update::select(&[candidate()], &[wrong])
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        amd_update::select(&[candidate()], &[device(), device()])
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn amd_runs_without_installer_clicks_and_verifies_completion() {
    let mut mock = api();
    let mut phases = vec![];
    let result = amd_update::run_with(
        &mut mock,
        &[device()],
        &AtomicBool::new(false),
        &mut |phase, _| phases.push(phase),
    )
    .unwrap();
    assert_eq!(result.0, Status::Completed);
    assert_eq!(mock.calls, 1);
    assert_eq!(phases, vec![Status::Verifying, Status::Installing]);
}
#[test]
fn amd_never_accepts_exit_zero_without_evidence() {
    let mut mock = api();
    mock.present = false;
    assert_eq!(
        amd_update::run_with(
            &mut mock,
            &[device()],
            &AtomicBool::new(false),
            &mut |_, _| {}
        )
        .unwrap()
        .0,
        Status::AwaitingVerification
    );
}
#[test]
fn amd_distinguishes_uac_cancel_reboot_and_failure() {
    for (code, expected) in [(1223, Status::Cancelled), (3010, Status::AwaitingReboot)] {
        let mut mock = api();
        mock.code = code;
        assert_eq!(
            amd_update::run_with(
                &mut mock,
                &[device()],
                &AtomicBool::new(false),
                &mut |_, _| {}
            )
            .unwrap()
            .0,
            expected
        );
    }
    let mut mock = api();
    mock.code = 0x80240022;
    assert!(
        amd_update::run_with(
            &mut mock,
            &[device()],
            &AtomicBool::new(false),
            &mut |_, _| {}
        )
        .is_err()
    );
}
#[test]
fn amd_no_offer_is_pending_and_cancellation_never_installs() {
    let mut mock = api();
    mock.offers.clear();
    assert_eq!(
        amd_update::run_with(
            &mut mock,
            &[device()],
            &AtomicBool::new(false),
            &mut |_, _| {}
        )
        .unwrap()
        .0,
        Status::RequiresAction
    );
    assert_eq!(mock.calls, 0);
    let cancel = AtomicBool::new(false);
    let mut mock = api();
    assert_eq!(
        amd_update::run_with(&mut mock, &[device()], &cancel, &mut |_, _| cancel
            .store(true, Ordering::Relaxed))
        .unwrap()
        .0,
        Status::Cancelled
    );
    assert_eq!(mock.calls, 0);
}
#[test]
fn legacy_atlas_matches_only_reviewed_windows10_and_editions() {
    let mut info = SystemInfo {
        build: 19045,
        architecture: "x86_64".into(),
        edition: "Professional".into(),
        ..Default::default()
    };
    assert!(ruhtweaks::atlas::legacy::compatibility(&info).is_ok());
    assert!(ruhtweaks::atlas::compatibility(&info).is_err());
    info.build = 19044;
    assert!(ruhtweaks::atlas::legacy::compatibility(&info).is_err());
    info.build = 19045;
    info.edition = "Core".into();
    assert!(ruhtweaks::atlas::legacy::compatibility(&info).is_err());
    info.edition = "Professional".into();
    info.architecture = "aarch64".into();
    assert!(ruhtweaks::atlas::legacy::compatibility(&info).is_err());
}
#[test]
fn retiring_extension_preserves_completed_apps_and_original_documents() {
    use sha2::{Digest, Sha256};
    let original = r#"{"schema_version":3,"browser":"firefox","apps":["steam"],"atlas":false,"drivers":false,"google_search":false,"ublock":true,"tweaks":[],"accept_terms":false,"allow_startup":false,"prefer_silent":true,"updates":[]}"#;
    let dir = tempfile::tempdir().unwrap();
    let store = Store {
        root: dir.path().into(),
    };
    std::fs::write(store.root.join("profile.json"), original).unwrap();
    let profile = store.load_profile().unwrap();
    let mut journal = installer::plan(&profile, &SystemInfo::default()).unwrap();
    journal.tasks[0].status = Status::Completed;
    journal.tasks.push(ruhtweaks::state::Task {
        id: "browser".into(),
        title: "retired".into(),
        action: ruhtweaks::state::Action::Browser,
        status: Status::RequiresAction,
        detail: "retired".into(),
        admin: String::new(),
    });
    journal.schema_version = 3;
    journal.profile_hash = format!("{:x}", Sha256::digest(original.as_bytes()));
    ruhtweaks::state::atomic_json(&store.root.join("journal.json"), &journal).unwrap();
    store.save_profile(&profile).unwrap();
    let migrated = store.load_journal().unwrap().unwrap();
    assert_eq!(migrated.schema_version, 4);
    assert_eq!(migrated.profile_hash, profile.fingerprint().unwrap());
    assert_eq!(migrated.tasks[0].status, Status::Completed);
    assert!(
        !migrated
            .tasks
            .iter()
            .any(|t| t.action == ruhtweaks::state::Action::Browser)
    );
    assert!(!serde_json::to_string(&profile).unwrap().contains("ublock"));
    assert!(store.root.join("backups/profile-v3.json").exists());
    assert!(store.root.join("backups/journal-v3.json").exists());
    assert!(!profile.accept_terms);
    assert!(!profile.atlas_legacy);
}
#[test]
fn new_profile_does_not_accept_removed_feature_or_unknown_fields() {
    let mut value = serde_json::to_value(Profile::default()).unwrap();
    value["ublock"] = true.into();
    assert!(ruhtweaks::state::decode_profile(value).is_err());
}
#[test]
fn helper_rejects_tampered_request_before_any_update_api() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("request.json");
    std::fs::write(&path, b"{}").unwrap();
    assert!(amd_update::helper(&path, &format!("sha256:{}", "0".repeat(64))).is_err());
}
#[test]
fn final_approval_only_changes_terms_and_preserves_reviewed_actions() {
    let profile = Profile::default();
    let mut queue = installer::plan(&profile, &SystemInfo::default()).unwrap();
    queue.tasks[0].status = Status::Completed;
    let (approved, updated) = installer::approve_reviewed(&profile, &queue).unwrap();
    assert!(approved.accept_terms);
    assert!(!profile.accept_terms);
    assert_eq!(updated.profile_hash, approved.fingerprint().unwrap());
    assert_eq!(updated.tasks[0].status, Status::Completed);
    assert_eq!(
        updated.tasks.iter().map(|t| &t.action).collect::<Vec<_>>(),
        queue.tasks.iter().map(|t| &t.action).collect::<Vec<_>>()
    );
    let changed = Profile {
        drivers: true,
        ..profile.clone()
    };
    assert!(installer::approve_reviewed(&changed, &queue).is_err());
    queue.tasks[0].action = ruhtweaks::state::Action::Tweak("balanced".into());
    assert!(installer::approve_reviewed(&profile, &queue).is_err());
}
