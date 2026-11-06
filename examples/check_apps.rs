//! Consulta somente leitura. Não aceita termos nem executa instalações.
use ruhtweaks::{
    installer::{Executor, NativeExecutor},
    system_info::SystemInfo,
};
fn main() {
    let mut executor = NativeExecutor::new(SystemInfo::default());
    let mut keys = std::env::args().skip(1).collect::<Vec<_>>();
    if keys.is_empty() {
        keys.push("brave".into());
    }
    for key in keys {
        match executor.detect(&key) {
            Ok(detection) => println!("{key}: {detection:?}"),
            Err(error) => {
                eprintln!("{key}: {error}");
                std::process::exit(1);
            }
        }
        if ruhtweaks::catalog::find(&key).is_some_and(|app| app.winget.is_some()) {
            match executor.version(&key) {
                Ok(version) => println!("{key} versão WinGet: {version:?}"),
                Err(error) => eprintln!("Versão não consultada: {error}"),
            }
        }
    }
}
