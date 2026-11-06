use ruhtweaks::{
    Result,
    browser::policy::{self, Registry, Value},
    fail,
    installer::{
        self,
        updates::{self, UpdateBackend},
    },
    state::{Profile, Status, Store, UpdateRequest},
    system_info::SystemInfo,
    tweaks::power::{self, PowerControl},
};
use std::collections::BTreeMap;
fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store {
        root: dir.path().into(),
    };
    (dir, store)
}
fn request() -> UpdateRequest {
    UpdateRequest {
        key: "brave".into(),
        from: "1.0".into(),
        to: "2.0".into(),
    }
}
fn approved() -> Profile {
    Profile {
        accept_terms: true,
        updates: vec![request()],
        ..Profile::default()
    }
}
struct Updates {
    versions: Vec<Option<String>>,
    calls: usize,
    code: i32,
}
impl UpdateBackend for Updates {
    fn installed_version(&mut self, _: &str) -> Result<Option<String>> {
        Ok(self.versions.remove(0))
    }
    fn upgrade(&mut self, _: &UpdateRequest, _: &Profile) -> Result<i32> {
        self.calls += 1;
        Ok(self.code)
    }
}
fn backend(before: Option<&str>, after: Option<&str>) -> Updates {
    Updates {
        versions: vec![before.map(str::to_string), after.map(str::to_string)],
        calls: 0,
        code: 0,
    }
}
#[test]
fn upgrade_requires_exact_consent() {
    let mut b = backend(Some("1.0"), Some("2.0"));
    assert!(updates::apply(&mut b, &request(), &Profile::default()).is_err());
    assert_eq!(b.calls, 0);
    let mut altered = request();
    altered.to = "3.0".into();
    assert!(updates::apply(&mut b, &altered, &approved()).is_err());
    assert_eq!(b.calls, 0);
}
#[test]
fn upgrade_checks_versions_before_and_after() {
    for before in [None, Some("0.9"), Some("3.0")] {
        let mut b = backend(before, Some("2.0"));
        assert_eq!(
            updates::apply(&mut b, &request(), &approved()).unwrap().0,
            Status::RequiresAction
        );
        assert_eq!(b.calls, 0);
    }
    let mut b = backend(Some("1.0"), Some("1.0"));
    assert_eq!(
        updates::apply(&mut b, &request(), &approved()).unwrap().0,
        Status::RequiresAction
    );
    assert_eq!(b.calls, 1);
    let mut b = backend(Some("1.0"), Some("2.0"));
    assert_eq!(
        updates::apply(&mut b, &request(), &approved()).unwrap().0,
        Status::Completed
    );
}
#[test]
fn upgrade_does_not_repeat_and_never_claims_reboot_as_success() {
    let mut b = backend(Some("2.0"), None);
    assert_eq!(
        updates::apply(&mut b, &request(), &approved()).unwrap().0,
        Status::Completed
    );
    assert_eq!(b.calls, 0);
    let mut b = backend(Some("1.0"), Some("2.0"));
    b.code = 3010;
    assert_eq!(
        updates::apply(&mut b, &request(), &approved()).unwrap().0,
        Status::RequiresAction
    );
    let mut b = backend(Some("1.0"), Some("2.0"));
    b.code = -1;
    assert!(updates::apply(&mut b, &request(), &approved()).is_err());
}
#[test]
fn update_plan_contains_one_version_pinned_task_per_app() {
    let p = approved();
    let j = installer::plan(&p, &SystemInfo::default()).unwrap();
    assert!(
        !j.tasks
            .iter()
            .any(|t| t.action == ruhtweaks::state::Action::Install("brave".into()))
    );
    assert_eq!(
        j.tasks
            .iter()
            .filter(|t| matches!(t.action, ruhtweaks::state::Action::Upgrade(_)))
            .count(),
        1
    );
    let args = updates::command("brave", &p, Some(&request())).unwrap();
    assert_eq!(&args[args.len() - 2..], ["--version", "2.0"]);
    assert!(args.contains(&"--silent".into()));
    for bad in [
        "--all",
        "--force",
        "--allow-reboot",
        "--ignore-security-hash",
        "--override",
    ] {
        assert!(!args.iter().any(|a| a == bad));
    }
}
#[test]
fn unreviewed_startup_defaults_keep_installer_interactive() {
    for key in ["discord", "steam", "epic", "everything", "riot_valorant_br"] {
        assert_eq!(updates::mode(key, &Profile::default()), "--interactive");
    }
    for key in ["brave", "7zip", "vscode", "obs", "vlc"] {
        assert_eq!(updates::mode(key, &Profile::default()), "--silent");
    }
    let p = Profile {
        prefer_silent: false,
        ..Profile::default()
    };
    assert_eq!(updates::mode("brave", &p), "--interactive");
}
#[test]
fn json_export_ignores_unknown_versions_and_other_sources() {
    let json = r#"{"Sources":[{"SourceDetails":{"Identifier":"Microsoft.Winget.Source_8wekyb3d8bbwe"},"Packages":[{"PackageIdentifier":"Brave.Brave","Version":"1.0"},{"PackageIdentifier":"Unknown"}]},{"SourceDetails":{"Identifier":"other"},"Packages":[{"PackageIdentifier":"Brave.Brave","Version":"9.0"}]}]}"#;
    assert_eq!(
        updates::parse_export(json).unwrap(),
        BTreeMap::from([("Brave.Brave".into(), "1.0".into())])
    );
    let duplicate = json.replace(
        "{\"PackageIdentifier\":\"Unknown\"}",
        "{\"PackageIdentifier\":\"Brave.Brave\",\"Version\":\"1.1\"}",
    );
    assert!(updates::parse_export(&duplicate).is_err());
}
#[test]
fn localized_version_tables_do_not_guess_downgrades() {
    let versions = updates::parse_versions("Encontrado app\nVersão\n------\n2.0\n1.0\n");
    assert_eq!(
        updates::candidate("brave", "1.0", &versions),
        Some(request())
    );
    assert!(updates::candidate("brave", "3.0", &versions).is_none());
    assert!(updates::candidate("brave", "2.0", &versions).is_none());
    assert!(updates::parse_versions("Falha no servidor 2.0").is_empty());
}
#[test]
fn malformed_updates_and_duplicate_requests_rejected() {
    let mut p = approved();
    p.updates.push(request());
    assert!(p.validate().is_err());
    let mut p = approved();
    p.updates[0].to = "--all".into();
    assert!(p.validate().is_err());
    let mut p = approved();
    p.browser = None;
    assert!(p.validate().is_err());
}
#[derive(Default)]
struct Reg {
    user: BTreeMap<String, Value>,
    machine: BTreeMap<String, Value>,
    writes: usize,
    fail_at: Option<usize>,
}
impl Registry for Reg {
    fn read(&self, machine: bool, key: &str) -> Result<Option<Value>> {
        Ok(if machine { &self.machine } else { &self.user }
            .get(key)
            .cloned())
    }
    fn write(&mut self, key: &str, value: &Option<Value>) -> Result<()> {
        self.writes += 1;
        if self.fail_at == Some(self.writes) {
            return Err(fail("acesso recusado simulado"));
        }
        if let Some(v) = value {
            self.user.insert(key.into(), v.clone());
        } else {
            self.user.remove(key);
        }
        Ok(())
    }
}
#[test]
fn brave_policy_is_idempotent_reversible_and_preserves_shields() {
    let (_dir, store) = store();
    let mut reg = Reg::default();
    reg.user
        .insert("BraveShieldsEnabledForUrls".into(), Value::Text("*".into()));
    let original = reg.user.clone();
    policy::apply_with(&mut reg, &store).unwrap();
    policy::apply_with(&mut reg, &store).unwrap();
    assert_eq!(
        reg.user.get("DefaultSearchProviderName"),
        Some(&Value::Text("Google".into()))
    );
    policy::rollback_with(&mut reg, &store).unwrap();
    assert_eq!(reg.user, original);
}
#[test]
fn brave_policy_preserves_machine_and_existing_user_policies() {
    for machine in [false, true] {
        let (_dir, store) = store();
        let mut reg = Reg::default();
        if machine {
            &mut reg.machine
        } else {
            &mut reg.user
        }
        .insert(
            "DefaultSearchProviderName".into(),
            Value::Text("Company".into()),
        );
        assert!(policy::apply_with(&mut reg, &store).is_err());
        assert_eq!(reg.writes, 0);
    }
}
#[test]
fn brave_policy_partial_failure_can_be_rolled_back() {
    let (_dir, store) = store();
    let mut reg = Reg {
        fail_at: Some(2),
        ..Reg::default()
    };
    assert!(policy::apply_with(&mut reg, &store).is_err());
    assert!(store.root.join("backups/brave-search.json").exists());
    reg.fail_at = None;
    policy::rollback_with(&mut reg, &store).unwrap();
    assert!(reg.user.is_empty());
}
#[test]
fn brave_rollback_never_overwrites_an_external_change() {
    let (_dir, store) = store();
    let mut reg = Reg::default();
    policy::apply_with(&mut reg, &store).unwrap();
    reg.user.insert(
        "DefaultSearchProviderName".into(),
        Value::Text("Other".into()),
    );
    let writes = reg.writes;
    assert!(policy::rollback_with(&mut reg, &store).is_err());
    assert_eq!(reg.writes, writes);
}
struct Power {
    guid: u128,
    refuse: bool,
}
impl PowerControl for Power {
    fn current(&self) -> Result<u128> {
        Ok(self.guid)
    }
    fn set(&mut self, guid: u128) -> Result<()> {
        if self.refuse {
            return Err(fail("política gerenciada"));
        }
        self.guid = guid;
        Ok(())
    }
}
#[test]
fn power_backup_survives_retries_and_restores_original() {
    let (_dir, store) = store();
    let mut api = Power {
        guid: 5,
        refuse: false,
    };
    power::apply_with(&mut api, &store).unwrap();
    power::apply_with(&mut api, &store).unwrap();
    power::rollback_with(&mut api, &store).unwrap();
    assert_eq!(api.guid, 5);
}
#[test]
fn power_rejects_external_changes_and_preserves_failed_backup() {
    let (_dir, store) = store();
    let mut api = Power {
        guid: 5,
        refuse: true,
    };
    assert!(power::apply_with(&mut api, &store).is_err());
    assert!(store.root.join("backups/balanced-power.json").exists());
    api.refuse = false;
    power::apply_with(&mut api, &store).unwrap();
    api.guid = 8;
    assert!(power::rollback_with(&mut api, &store).is_err());
    assert_eq!(api.guid, 8);
}
#[test]
fn old_profile_migrates_without_adding_automatic_consent() {
    let (_dir, store) = store();
    let mut value = serde_json::to_value(Profile::default()).unwrap();
    value["schema_version"] = 1.into();
    value.as_object_mut().unwrap().remove("atlas_legacy");
    value["ublock"] = false.into();
    value.as_object_mut().unwrap().remove("updates");
    value.as_object_mut().unwrap().remove("prefer_silent");
    ruhtweaks::state::atomic_json(&store.root.join("profile.json"), &value).unwrap();
    let p = store.load_profile().unwrap();
    assert_eq!(p.schema_version, 4);
    assert!(!p.prefer_silent);
    assert!(p.updates.is_empty());
    assert!(!p.accept_terms);
    let original: serde_json::Value =
        ruhtweaks::state::read_json(&store.root.join("backups/profile-v1.json")).unwrap();
    assert_eq!(original, value);
    let untouched: serde_json::Value =
        ruhtweaks::state::read_json(&store.root.join("profile.json")).unwrap();
    assert_eq!(untouched, value);
}
#[test]
fn amd_resolver_rejects_lookalikes_and_ambiguous_links() {
    use ruhtweaks::drivers::amd;
    let url = "https://drivers.amd.com/drivers/installer/26.20/beta/amd-software-adrenalin-edition-26.10.2-minimalsetup-261029_web.exe";
    assert_eq!(
        amd::resolve(&format!("<a href=\"{url}\">Download</a>")).unwrap(),
        url
    );
    assert!(amd::resolve(&url.replace("drivers.amd.com", "drivers.amd.com.evil.test")).is_err());
    assert!(amd::resolve(&url.replace("https:", "http:")).is_err());
    assert!(amd::resolve(&format!("'{url}' '{}'", url.replace("261029", "261030"))).is_err());
}
#[test]
fn riot_game_installation_requires_explicit_game_selection() {
    let p = Profile {
        apps: vec!["riot".into()],
        browser: None,
        ..Profile::default()
    };
    let j = installer::plan(&p, &SystemInfo::default()).unwrap();
    assert_eq!(j.tasks.len(), 1);
    assert_eq!(
        j.tasks[0].action,
        ruhtweaks::state::Action::Install("riot".into())
    );
    let p = Profile {
        apps: vec!["riot".into(), "riot_valorant_br".into()],
        ..p
    };
    let j = installer::plan(&p, &SystemInfo::default()).unwrap();
    assert_eq!(j.tasks.len(), 1);
    assert_eq!(
        j.tasks[0].action,
        ruhtweaks::state::Action::Install("riot_valorant_br".into())
    );
    assert!(
        !Profile::default()
            .apps
            .iter()
            .any(|key| key.starts_with("riot_"))
    );
}
#[test]
fn riot_bootstrapper_registration_is_not_game_readiness() {
    use installer::{Detection, products::confirmed_registration};
    assert!(matches!(
        confirmed_registration("riot_valorant_br", Detection::Present),
        Detection::Unknown(_)
    ));
    assert_eq!(
        confirmed_registration("riot_valorant_br", Detection::Absent),
        Detection::Absent
    );
    assert_eq!(
        confirmed_registration("brave", Detection::Present),
        Detection::Present
    );
}
#[test]
fn approved_update_target_cannot_be_replaced_in_the_journal() {
    use ruhtweaks::{
        installer::{Detection, Executor},
        state::{Journal, Task},
    };
    struct Never;
    impl Executor for Never {
        fn detect(&mut self, _: &str) -> Result<Detection> {
            panic!("must reject before querying")
        }
        fn execute(
            &mut self,
            _: &Task,
            _: &Profile,
            _: &Store,
            _: &std::sync::atomic::AtomicBool,
            _: &mut dyn FnMut(String),
        ) -> Result<(Status, String)> {
            panic!("must not execute")
        }
    }
    let p = approved();
    let (_dir, store) = store();
    let mut journal: Journal = installer::plan(&p, &SystemInfo::default()).unwrap();
    if let ruhtweaks::state::Action::Upgrade(request) = &mut journal
        .tasks
        .iter_mut()
        .find(|task| matches!(task.action, ruhtweaks::state::Action::Upgrade(_)))
        .unwrap()
        .action
    {
        request.to = "3.0".into();
    }
    assert!(
        installer::execute_plan(
            &mut Never,
            &p,
            &mut journal,
            &store,
            &std::sync::atomic::AtomicBool::new(false),
            |_| {}
        )
        .is_err()
    );
}
