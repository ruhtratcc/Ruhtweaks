#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use ruhtweaks::{
    Result, fail, installer,
    state::{Profile, Store},
    system_info,
};
fn main() {
    if let Err(error) = start() {
        eprintln!("Ruhtweaks: {error}");
        #[cfg(windows)]
        if std::env::args().len() == 1 || std::env::args().nth(1).as_deref() == Some("--data-dir") {
            use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
            let text: Vec<u16> = format!("{error}").encode_utf16().chain(Some(0)).collect();
            let title: Vec<u16> = "Ruhtweaks".encode_utf16().chain(Some(0)).collect();
            unsafe {
                MessageBoxW(
                    std::ptr::null_mut(),
                    text.as_ptr(),
                    title.as_ptr(),
                    MB_OK | MB_ICONERROR,
                );
            }
        }
        std::process::exit(1)
    }
}
fn start() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if let [flag, request, digest] = args.as_slice()
        && flag == "--amd-update-helper"
    {
        let code = ruhtweaks::drivers::amd_update::helper(std::path::Path::new(request), digest)?;
        std::process::exit(code as i32);
    }
    if args.first().is_some_and(|a| a == "--help") {
        println!(
            "Ruhtweaks\nSem argumentos: interface nativa\n--inventory: inventário JSON (somente leitura)\n--dry-run [perfil.json]: plano JSON, sem alterações\n--simulate: execução simulada em diretório temporário\n--data-dir CAMINHO: diretório local para dados da interface\nNão há modo CLI de instalação sem interface."
        );
        return Ok(());
    }
    if args.first().is_some_and(|a| a == "--inventory") {
        println!("{}", serde_json::to_string_pretty(&system_info::collect())?);
        return Ok(());
    }
    if args.first().is_some_and(|a| a == "--dry-run") {
        let p: Profile = if let Some(path) = args.get(1) {
            ruhtweaks::state::decode_profile(ruhtweaks::state::read_json(std::path::Path::new(
                path,
            ))?)?
        } else {
            Profile::default()
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&installer::plan(&p, &system_info::collect())?)?
        );
        return Ok(());
    }
    if args.first().is_some_and(|a| a == "--simulate") {
        return simulate();
    }
    let store = match args.as_slice() {
        [] => Store::default_location()?,
        [flag, path] if flag == "--data-dir" => Store {
            root: std::path::PathBuf::from(path),
        },
        _ => return Err(fail("Argumentos inválidos; consulte --help")),
    };
    let _lock = store.lock()?;
    let first_run = !store.root.join("profile.json").try_exists()?;
    let profile = store.load_profile()?;
    store.save_profile(&profile)?;
    let journal = store.load_journal()?;
    if let Some(journal) = &journal {
        store.save_journal(journal)?;
    }
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(store.root.join("ruhtweaks.log"))?;
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(std::sync::Mutex::new(log))
        .try_init()
        .map_err(|e| fail(e.to_string()))?;
    ruhtweaks::app_ui::run(store, profile, journal, first_run)
}
fn simulate() -> Result<()> {
    struct Mock;
    impl installer::Executor for Mock {
        fn detect(&mut self, _: &str) -> Result<installer::Detection> {
            Ok(installer::Detection::Absent)
        }
        fn execute(
            &mut self,
            _: &ruhtweaks::state::Task,
            _: &Profile,
            _: &Store,
            _: &std::sync::atomic::AtomicBool,
            _: &mut dyn FnMut(String),
        ) -> Result<(ruhtweaks::state::Status, String)> {
            Ok((
                ruhtweaks::state::Status::Completed,
                "SIMULAÇÃO: nenhuma alteração no sistema".into(),
            ))
        }
    }
    let dir = tempfile::tempdir()?;
    let store = Store {
        root: dir.path().into(),
    };
    let p = Profile {
        apps: vec!["steam".into()],
        ..Profile::default()
    };
    let mut journal = installer::plan(&p, &system_info::SystemInfo::default())?;
    installer::execute_plan(
        &mut Mock,
        &p,
        &mut journal,
        &store,
        &installer::cancellation(),
        |_| {},
    )?;
    println!("SIMULAÇÃO\n{}", journal.report());
    Ok(())
}
