use ruhtweaks::{
    Result,
    atlas::verification::{self, Evidence},
    installer::{
        self, Detection, Executor,
        handoff::{self, Consent, InstallBackend, Kind, Prepared},
    },
    security::{self, Signature},
    state::{self, Action, Journal, Profile, SCHEMA, Status, Store, Task},
    system_info::SystemInfo,
};
use std::{
    io::{self, Read},
    sync::atomic::{AtomicBool, Ordering},
};
fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store {
        root: dir.path().into(),
    };
    (dir, store)
}
fn file() -> (Profile, Prepared, Consent) {
    let p = Profile::default();
    let f = Prepared {
        schema_version: 1,
        kind: Kind::Litecord,
        profile_hash: p.fingerprint().unwrap(),
        source: "https://github.com/Ak4ai/Litecord/releases/download/v1.3.1/Litecord-Setup-x64.exe"
            .into(),
        signature: Signature::Unsigned,
        started_ms: None,
        boot_before: None,
        launch_attempted: false,
        baseline_applied_ms: None,
    };
    let consent = Consent {
        kind: Kind::Litecord,
        profile_hash: f.profile_hash.clone(),
        digest: Kind::Litecord.digest().into(),
        acknowledge: true,
        silent: false,
    };
    (p, f, consent)
}
struct Backend {
    present: Vec<bool>,
    exit: Option<u32>,
    calls: usize,
    args: String,
}
impl InstallBackend for Backend {
    fn launch(&mut self, _: &Prepared, args: &str) -> Result<Option<u32>> {
        self.calls += 1;
        self.args = args.into();
        Ok(self.exit)
    }
    fn litecord_present(&mut self) -> Result<bool> {
        Ok(self.present.remove(0))
    }
}
fn backend(exit: Option<u32>, after: bool) -> Backend {
    Backend {
        present: vec![false, after],
        exit,
        calls: 0,
        args: String::new(),
    }
}
#[test]
fn unsigned_needs_file_specific_consent_and_does_not_mean_malware() {
    let (p, f, mut c) = file();
    assert!(c.validate(&f, &p).is_ok());
    c.acknowledge = false;
    let mut b = backend(Some(0), true);
    assert!(handoff::run_litecord(&mut b, &f, &c, &p).is_err());
    assert_eq!(b.calls, 0);
    c.acknowledge = true;
    c.digest = "sha256:00".into();
    assert!(c.validate(&f, &p).is_err());
    c.digest = Kind::Litecord.digest().into();
    c.profile_hash = "other".into();
    assert!(c.validate(&f, &p).is_err());
}
#[test]
fn invalid_signature_is_not_downgraded_to_unsigned() {
    let (p, mut f, c) = file();
    f.signature = Signature::Unverifiable(0x80096010);
    let mut b = backend(Some(0), true);
    assert!(handoff::run_litecord(&mut b, &f, &c, &p).is_err());
    assert_eq!(b.calls, 0);
}
#[test]
fn existing_product_skips_even_confirmed_installer() {
    let (p, f, c) = file();
    let mut b = backend(Some(0), true);
    b.present = vec![true];
    assert_eq!(
        handoff::run_litecord(&mut b, &f, &c, &p).unwrap().0,
        Status::Completed
    );
    assert_eq!(b.calls, 0);
}
#[test]
fn exit_zero_without_detection_is_not_completion() {
    let (p, f, c) = file();
    let mut b = backend(Some(0), false);
    assert_eq!(
        handoff::run_litecord(&mut b, &f, &c, &p).unwrap().0,
        Status::AwaitingVerification
    );
    let mut b = backend(Some(0), true);
    assert_eq!(
        handoff::run_litecord(&mut b, &f, &c, &p).unwrap().0,
        Status::Completed
    );
}
#[test]
fn cancel_reboot_failure_and_untracked_process_are_distinct() {
    let (p, f, c) = file();
    for code in [1223, 2, 5] {
        assert_eq!(
            handoff::run_litecord(&mut backend(Some(code), false), &f, &c, &p)
                .unwrap()
                .0,
            Status::Cancelled
        );
    }
    assert_eq!(
        handoff::run_litecord(&mut backend(Some(3010), false), &f, &c, &p)
            .unwrap()
            .0,
        Status::AwaitingReboot
    );
    assert!(handoff::run_litecord(&mut backend(Some(1), false), &f, &c, &p).is_err());
    assert_eq!(
        handoff::run_litecord(&mut backend(None, false), &f, &c, &p)
            .unwrap()
            .0,
        Status::AwaitingVerification
    );
}
#[test]
fn documented_inno_modes_preserve_cancel_security_and_no_forced_close() {
    for silent in [false, true] {
        let args = handoff::parameters(Kind::Litecord, silent);
        for flag in [
            "/NOCLOSEAPPLICATIONS",
            "/NOFORCECLOSEAPPLICATIONS",
            "/NORESTART",
            "/NORESTARTAPPLICATIONS",
            "/MERGETASKS=!startupicon",
        ] {
            assert!(args.split_whitespace().any(|arg| arg == flag));
        }
        for forbidden in [
            "/FORCECLOSEAPPLICATIONS",
            "/CLOSEAPPLICATIONS",
            "/SUPPRESSMSGBOXES",
            "/NOCANCEL",
        ] {
            assert!(!args.split_whitespace().any(|arg| arg == forbidden));
        }
        assert_eq!(args.contains("/SILENT"), silent);
    }
    assert_eq!(handoff::parameters(Kind::Atlas, true), "");
}
fn evidence() -> Evidence {
    Evidence {
        version: "0.5.0".into(),
        name: "AtlasOS".into(),
        error_level: 0,
        selected_options: vec![
            "defender-enable".into(),
            "mitigations-default".into(),
            "auto-updates-default".into(),
        ],
        applied_ms: 2000,
    }
}
#[test]
fn atlas_requires_new_official_success_reboot_and_protections() {
    let e = evidence();
    assert_eq!(
        verification::evaluate(None, 1000, true, false, true).0,
        Status::AwaitingVerification
    );
    assert_eq!(
        verification::evaluate(Some(&e), 3000, true, false, true).0,
        Status::AwaitingVerification
    );
    assert_eq!(
        verification::evaluate(Some(&e), 1000, false, false, true).0,
        Status::AwaitingReboot
    );
    assert_eq!(
        verification::evaluate(Some(&e), 1000, true, true, true).0,
        Status::AwaitingReboot
    );
    assert_eq!(
        verification::evaluate(Some(&e), 1000, true, false, false).0,
        Status::AwaitingVerification
    );
    assert_eq!(
        verification::evaluate(Some(&e), 1000, true, false, true).0,
        Status::Completed
    );
}
#[test]
fn atlas_rejects_partial_errors_wrong_versions_and_insecure_options() {
    for level in [1, 2] {
        let e = Evidence {
            error_level: level,
            ..evidence()
        };
        assert_ne!(
            verification::evaluate(Some(&e), 1000, true, false, true).0,
            Status::Completed
        );
    }
    let e = Evidence {
        version: "0.6.0".into(),
        ..evidence()
    };
    assert_ne!(
        verification::evaluate(Some(&e), 1000, true, false, true).0,
        Status::Completed
    );
    let mut e = evidence();
    e.selected_options.push("disable-core-isolation".into());
    assert_ne!(
        verification::evaluate(Some(&e), 1000, true, false, true).0,
        Status::Completed
    );
}
#[test]
fn official_dotnet_utc_timestamp_is_decoded_without_kind_bits() {
    assert_eq!(
        verification::decode_dotnet_utc(
            0x4000_0000_0000_0000 | (621_355_968_000_000_000 + 20_000_000)
        )
        .unwrap(),
        2000
    );
    assert!(verification::decode_dotnet_utc(0).is_err());
}
const ABC: &str = "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
#[test]
fn transfer_only_publishes_valid_complete_bytes() {
    let (_dir, store) = store();
    let dest = store.root.join("downloads/file.bin");
    security::transfer::copy_verified(
        &mut &b"abc"[..],
        &dest,
        ABC,
        Some(3),
        &AtomicBool::new(false),
        |_, _| {},
    )
    .unwrap();
    assert_eq!(std::fs::read(dest).unwrap(), b"abc");
}
struct Interrupted(bool);
impl Read for Interrupted {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.0 {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "interrompido",
            ));
        }
        self.0 = true;
        buffer[..3].copy_from_slice(b"abc");
        Ok(3)
    }
}
#[test]
fn interrupted_or_cancelled_or_bad_hash_never_publishes_and_can_resume() {
    let (_dir, store) = store();
    let dest = store.root.join("downloads/file.bin");
    assert!(
        security::transfer::copy_verified(
            &mut Interrupted(false),
            &dest,
            ABC,
            None,
            &AtomicBool::new(false),
            |_, _| {}
        )
        .is_err()
    );
    assert!(!dest.exists());
    assert!(
        security::transfer::copy_verified(
            &mut &b"different"[..],
            &dest,
            ABC,
            None,
            &AtomicBool::new(false),
            |_, _| {}
        )
        .is_err()
    );
    assert!(!dest.exists());
    let cancel = AtomicBool::new(false);
    assert!(
        security::transfer::copy_verified(
            &mut &b"abc"[..],
            &dest,
            ABC,
            Some(3),
            &cancel,
            |_, _| cancel.store(true, Ordering::Relaxed)
        )
        .is_err()
    );
    assert!(!dest.exists());
    security::transfer::copy_verified(
        &mut &b"abc"[..],
        &dest,
        ABC,
        Some(3),
        &AtomicBool::new(false),
        |_, _| {},
    )
    .unwrap();
    assert!(dest.is_file());
    assert_eq!(
        std::fs::read_dir(dest.parent().unwrap()).unwrap().count(),
        1
    );
}
#[test]
fn crash_partial_is_reclaimed_only_by_valid_ownership_receipt() {
    let (_dir, store) = store();
    let parent = store.root.join("downloads");
    std::fs::create_dir_all(&parent).unwrap();
    std::fs::write(parent.join("ruhtweaks-owned.part"), b"partial").unwrap();
    std::fs::write(parent.join("user.txt"), b"keep").unwrap();
    state::atomic_json(
        &parent.join(".ruhtweaks-transfer.json"),
        &serde_json::json!({"schema_version":1,"digest":ABC,"partial":"../user.txt"}),
    )
    .unwrap();
    assert!(security::transfer::recover(&parent, ABC).is_err());
    assert!(parent.join("user.txt").exists());
    state::atomic_json(
        &parent.join(".ruhtweaks-transfer.json"),
        &serde_json::json!({"schema_version":1,"digest":ABC,"partial":"ruhtweaks-owned.part"}),
    )
    .unwrap();
    security::transfer::recover(&parent, ABC).unwrap();
    assert!(!parent.join("ruhtweaks-owned.part").exists());
    assert!(parent.join("user.txt").exists());
}
#[test]
fn cleanup_retains_reboot_pending_failed_and_unverified_files() {
    let (_dir, store) = store();
    let root = store.root.join("downloads");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("owned.exe");
    std::fs::write(&path, b"abc").unwrap();
    for state in [
        Status::AwaitingReboot,
        Status::Failed,
        Status::AwaitingVerification,
        Status::RequiresConfirmation,
    ] {
        assert!(!handoff::cleanup_file(&path, &root, ABC, state).unwrap());
        assert!(path.exists());
    }
    assert!(handoff::cleanup_file(&path, &root, ABC, Status::Completed).unwrap());
    assert!(!path.exists());
}
#[test]
fn cleanup_never_deletes_outside_root_or_substituted_files() {
    let (_dir, store) = store();
    let root = store.root.join("downloads");
    std::fs::create_dir_all(&root).unwrap();
    let outside = store.root.join("backup.txt");
    std::fs::write(&outside, b"abc").unwrap();
    assert!(handoff::cleanup_file(&outside, &root, ABC, Status::Completed).is_err());
    assert!(outside.exists());
    let inside = root.join("owned.exe");
    std::fs::write(&inside, b"changed").unwrap();
    assert!(handoff::cleanup_file(&inside, &root, ABC, Status::Completed).is_err());
    assert!(inside.exists());
}
struct Pipeline {
    calls: usize,
    cleanup: usize,
}
impl Executor for Pipeline {
    fn detect(&mut self, _: &str) -> Result<Detection> {
        Ok(Detection::Absent)
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
        Ok((Status::RequiresConfirmation, "arquivo pronto".into()))
    }
    fn cleanup(&mut self, _: &Task, _: &Store) -> Result<()> {
        self.cleanup += 1;
        Ok(())
    }
}
#[test]
fn confirmation_is_a_queue_barrier_and_never_cleans_unverified_files() {
    let (_dir, store) = store();
    let p = Profile {
        browser: None,
        apps: vec!["litecord".into(), "steam".into()],
        ..Profile::default()
    };
    let mut j = installer::plan(&p, &SystemInfo::default()).unwrap();
    let mut e = Pipeline {
        calls: 0,
        cleanup: 0,
    };
    installer::execute_plan(&mut e, &p, &mut j, &store, &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(e.calls, 1);
    assert_eq!(e.cleanup, 0);
    assert_eq!(j.tasks[1].status, Status::Pending);
    let serialized = serde_json::to_string(&j).unwrap();
    assert!(!serialized.contains("acknowledge"));
    assert!(!serialized.contains("silent"));
}
#[test]
fn new_execution_phases_recover_without_repeating() {
    let mut j = Journal {
        schema_version: SCHEMA,
        profile_hash: "test".into(),
        tasks: vec![Task {
            id: "atlas".into(),
            title: "Atlas".into(),
            action: Action::Atlas,
            status: Status::Executing,
            detail: String::new(),
            admin: String::new(),
        }],
    };
    j.recover();
    assert_eq!(j.tasks[0].status, Status::RequiresAction);
    assert!(!j.tasks[0].status.active());
}
#[test]
fn v2_profile_and_queue_migrate_without_changing_choices_or_completion() {
    let (_dir, store) = store();
    let original = r#"{"schema_version":2,"browser":"brave","apps":["litecord","steam","epic","riot"],"atlas":false,"drivers":false,"google_search":false,"ublock":false,"tweaks":[],"accept_terms":true,"allow_startup":false,"prefer_silent":false,"updates":[]}"#;
    use sha2::{Digest, Sha256};
    let hash = format!("{:x}", Sha256::digest(original.as_bytes()));
    state::atomic_write(&store.root.join("profile.json"), original.as_bytes()).unwrap();
    let current = store.load_profile().unwrap();
    let mut j = installer::plan(&current, &SystemInfo::default()).unwrap();
    j.schema_version = 2;
    j.profile_hash = hash;
    j.tasks[0].status = Status::Completed;
    state::atomic_json(&store.root.join("journal.json"), &j).unwrap();
    let migrated = store.load_journal().unwrap().unwrap();
    assert_eq!(migrated.profile_hash, current.fingerprint().unwrap());
    assert_eq!(migrated.tasks[0].status, Status::Completed);
    assert_eq!(current.apps, Profile::default().apps);
    assert!(current.accept_terms);
    assert!(!current.prefer_silent);
    assert!(store.root.join("backups/profile-v2.json").is_file());
    assert!(store.root.join("backups/journal-v2.json").is_file());
}
#[test]
fn cancelled_preparation_does_not_report_failed_installation() {
    struct Cancel;
    impl Executor for Cancel {
        fn stage(&self, _: &Task) -> Option<Status> {
            Some(Status::Preparing)
        }
        fn detect(&mut self, _: &str) -> Result<Detection> {
            Ok(Detection::Absent)
        }
        fn execute(
            &mut self,
            _: &Task,
            _: &Profile,
            _: &Store,
            cancel: &AtomicBool,
            _: &mut dyn FnMut(String),
        ) -> Result<(Status, String)> {
            cancel.store(true, Ordering::Relaxed);
            Err(ruhtweaks::fail("leitura interrompida"))
        }
    }
    let (_dir, store) = store();
    let p = Profile {
        browser: None,
        apps: vec!["litecord".into()],
        ..Profile::default()
    };
    let mut j = installer::plan(&p, &SystemInfo::default()).unwrap();
    installer::execute_plan(
        &mut Cancel,
        &p,
        &mut j,
        &store,
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert_eq!(j.tasks[0].status, Status::Cancelled);
}
#[test]
fn executable_presence_alone_does_not_accept_incomplete_pe() {
    let (_dir, store) = store();
    let file = store.root.join("incomplete.exe");
    std::fs::write(&file, b"MZ").unwrap();
    assert!(security::verify_pe_header(&file).is_err());
    let mut data = vec![0; 96];
    data[..2].copy_from_slice(b"MZ");
    data[60..64].copy_from_slice(&64u32.to_le_bytes());
    data[64..68].copy_from_slice(b"PE\0\0");
    std::fs::write(&file, data).unwrap();
    assert!(security::verify_pe_header(&file).is_ok());
}
