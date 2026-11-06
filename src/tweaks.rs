use crate::{
    Result, fail,
    state::{Store, atomic_json, read_json},
};
use serde::{Deserialize, Serialize};
pub mod power;
pub struct Tweak {
    pub key: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub uri: Option<&'static str>,
}
pub const TWEAKS: &[Tweak] = &[
    Tweak {
        key: "startup",
        title: "Revisar inicialização",
        description: "Requisito: Windows 10/11. Efeito: controle de apps no logon. Risco baixo ao revisar somente apps conhecidos. Estado: consulte a lista do Windows. Ação e reversão: habilitar/desabilitar individualmente nas Configurações.",
        uri: Some("ms-settings:startupapps"),
    },
    Tweak {
        key: "refresh",
        title: "Revisar taxa de atualização",
        description: "Requisito: monitor e modo compatíveis. Efeito: maior fluidez; pode elevar consumo. Estado e máximo por resolução no inventário. Ação/reversão: escolha a taxa nas Configurações; confirme apenas se a imagem permanecer correta.",
        uri: Some("ms-settings:display-advanced"),
    },
    Tweak {
        key: "power",
        title: "Revisar energia",
        description: "Requisito: Windows 10/11. Efeito: consumo, temperatura e autonomia. Estado: plano atual nas Configurações. Notebook: prefira equilíbrio/autonomia na bateria. Ação/reversão: escolha outro modo de energia.",
        uri: Some("ms-settings:powersleep"),
    },
    Tweak {
        key: "game_mode",
        title: "Revisar Modo de Jogo",
        description: "Requisito: Windows 10 1703+. Efeito: prioridades legítimas do Windows para jogos, sem promessa de FPS. Risco baixo. Estado, ação e reversão: opção Modo de Jogo nas Configurações.",
        uri: Some("ms-settings:gaming-gamemode"),
    },
    Tweak {
        key: "storage",
        title: "Revisar temporários do Windows",
        description: "Requisito: Windows 10/11. Revise categorias no armazenamento. Não selecione Downloads ou Windows.old sem backup e decisão própria. Limpeza não tem rollback; o Ruhtweaks somente abre a ferramenta oficial.",
        uri: Some("ms-settings:storagesense"),
    },
    Tweak {
        key: "extensions",
        title: "Mostrar extensões de arquivos",
        description: "Requisito: Explorer do Windows 10/11. Efeito: nomes completos, útil para identificar tipos. Risco baixo. Estado: lido do registro HKCU. Ação: HideFileExt=0, com backup atômico. Reversão: restaura o valor original; pode exigir reabrir o Explorer.",
        uri: None,
    },
    Tweak {
        key: "balanced",
        title: "Usar plano Equilibrado",
        description: "Windows 10/11: ativa o plano Equilibrado existente pela API PowerSetActiveScheme. Pode reduzir consumo e alterar desempenho frente ao plano atual, sem promessa de ganhos. Mantém o GUID original em backup; reversão verifica conflitos. Políticas da máquina e planos ausentes são respeitados.",
        uri: None,
    },
];
pub fn find(key: &str) -> Option<&'static Tweak> {
    TWEAKS.iter().find(|t| t.key == key)
}
#[derive(Serialize, Deserialize)]
struct Backup {
    schema_version: u32,
    original: Option<u32>,
}
#[cfg(windows)]
fn explorer() -> Result<winreg::RegKey> {
    Ok(
        winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey_with_flags(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced",
            winreg::enums::KEY_READ | winreg::enums::KEY_WRITE,
        )?,
    )
}
#[cfg(windows)]
pub fn extensions_state() -> Result<Option<u32>> {
    let k = explorer()?;
    match k.get_value("HideFileExt") {
        Ok(v) => Ok(Some(v)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
#[cfg(not(windows))]
pub fn extensions_state() -> Result<Option<u32>> {
    Err(fail("Requer Windows"))
}
pub fn apply_extensions(store: &Store) -> Result<()> {
    let backup = store.root.join("backups/explorer-extensions.json");
    let current = extensions_state()?;
    if !backup.exists() {
        atomic_json(
            &backup,
            &Backup {
                schema_version: 1,
                original: current,
            },
        )?;
    }
    #[cfg(windows)]
    explorer()?.set_value("HideFileExt", &0u32)?;
    if extensions_state()? != Some(0) {
        return Err(fail("Não foi possível verificar a alteração"));
    }
    Ok(())
}
pub fn rollback_extensions(store: &Store) -> Result<()> {
    let path = store.root.join("backups/explorer-extensions.json");
    let backup: Backup = read_json(&path)?;
    if backup.schema_version != 1 {
        return Err(fail("Backup não suportado"));
    }
    if extensions_state()? != Some(0) {
        return Err(fail(
            "Valor foi alterado por outro programa. Reversão automática bloqueada para preservar sua alteração.",
        ));
    }
    #[cfg(windows)]
    {
        let key = explorer()?;
        match backup.original {
            Some(v) => key.set_value("HideFileExt", &v)?,
            None => key.delete_value("HideFileExt")?,
        }
    }
    if extensions_state()? != backup.original {
        return Err(fail("Reversão não confirmada"));
    }
    std::fs::remove_file(path)?;
    Ok(())
}
