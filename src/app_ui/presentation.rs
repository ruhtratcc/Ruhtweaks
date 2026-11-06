use super::{AppRow, GpuRow, MainWindow, TweakRow};
use crate::{
    atlas, catalog, drivers,
    state::{Journal, Profile, Status},
    system_info::SystemInfo,
    tweaks,
};
use slint::{ModelRc, VecModel};

pub fn app_rows(profile: &Profile) -> Vec<AppRow> {
    catalog::APPS.iter().filter(|app| !app.browser).enumerate().map(|(index, app)| {
        let (category, mark, description) = match app.key {
            "litecord" => ("Comunicação", "L", "Cliente leve · instalação automática"),
            "discord" => ("Comunicação", "D", "Comunidades, voz e conversa"),
            "steam" => ("Jogos", "S", "Sua biblioteca de jogos"),
            "epic" => ("Jogos", "E", "Biblioteca Epic Games"),
            "riot" => ("Jogos", "R", "Jogos da Riot · assistido"),
            "riot_lol_br" => ("Jogos", "L", "Opcional · jogo + Riot Client + Vanguard"),
            "riot_valorant_br" => ("Jogos", "V", "Opcional · jogo + Riot Client + Vanguard"),
            "7zip" => ("Utilitários", "7", "Compactar e extrair arquivos"),
            "vlc" => ("Utilitários", "V", "Vídeos e músicas"),
            "vscode" => ("Utilitários", "<> ", "Código e projetos"),
            "obs" => ("Utilitários", "O", "Gravar e transmitir"),
            _ => ("Utilitários", "E", "Encontrar seus arquivos"),
        };
        AppRow { index: index as i32, name: app.name.into(), info: description.into(), mark: mark.into(), category: category.into(),
            detail: format!("{}\nFonte: {}\n{}", app.note, app.official.unwrap_or("Fonte não validada"), app.winget.map_or("Instalação assistida; não é considerada concluída pelo download.".into(), |id| format!("WinGet: {id}. A disponibilidade e a instalação são verificadas pelo motor."))).into(),
            selected: profile.apps.iter().any(|key| key == app.key) }
    }).collect()
}

pub fn tweak_rows(profile: &Profile, extensions_state: &str) -> Vec<TweakRow> {
    tweaks::TWEAKS
        .iter()
        .map(|tweak| {
            let (category, benefit) = match tweak.key {
                "startup" => (
                    "Inicialização",
                    "Escolha quais aplicativos acompanham o início do Windows.",
                ),
                "game_mode" => ("Jogos", "Revise as opções oficiais de jogos do Windows."),
                "power" => (
                    "Energia",
                    "Encontre o equilíbrio entre autonomia, consumo e desempenho.",
                ),
                "balanced" => (
                    "Energia",
                    "Ative o plano Equilibrado existente, com backup e reversão.",
                ),
                "refresh" => (
                    "Monitor",
                    "Confira a maior taxa disponível para a resolução atual.",
                ),
                "storage" => (
                    "Limpeza",
                    "Revise temporários no Windows. A limpeza não tem reversão.",
                ),
                _ => ("Arquivos", "Veja o tipo de cada arquivo com mais clareza."),
            };
            TweakRow {
                name: tweak.title.into(),
                info: tweak.description.into(),
                benefit: benefit.into(),
                category: category.into(),
                state: if tweak.key == "extensions" {
                    extensions_state.into()
                } else {
                    "Estado e escolha disponíveis nas Configurações do Windows.".into()
                },
                selected: profile.tweaks.iter().any(|key| key == tweak.key),
                assisted: tweak.uri.is_some(),
            }
        })
        .collect()
}

pub fn selection_summary(profile: &Profile) -> (i32, String) {
    let mut names = Vec::new();
    if let Some(browser) = profile.standalone_browser() {
        names.push(
            catalog::find(browser.key())
                .expect("navegador do catálogo")
                .name,
        );
    }
    for app in catalog::APPS.iter().filter(|app| !app.browser) {
        if profile.apps.iter().any(|key| key == app.key) {
            names.push(app.name);
        }
    }
    (
        names.len() as i32,
        if names.is_empty() {
            "Nenhum aplicativo selecionado.".into()
        } else {
            names.join(" · ")
        },
    )
}

pub fn status_code(status: Status) -> i32 {
    match status {
        Status::Pending => 0,
        Status::Downloading => 1,
        Status::Verifying => 2,
        Status::Installing => 3,
        Status::Completed => 4,
        Status::Failed => 5,
        Status::RequiresAction => 6,
        Status::Cancelled => 7,
        Status::Preparing => 2,
        Status::Executing => 3,
        Status::RequiresConfirmation
        | Status::ExternalAction
        | Status::AwaitingReboot
        | Status::AwaitingVerification => 6,
    }
}

pub fn run_summary(journal: &Journal) -> (String, String) {
    let completed = journal
        .tasks
        .iter()
        .filter(|task| task.status == Status::Completed)
        .count();
    let failed = journal
        .tasks
        .iter()
        .filter(|task| task.status == Status::Failed)
        .count();
    let action = journal
        .tasks
        .iter()
        .filter(|task| task.status.needs_action())
        .count();
    let cancelled = journal
        .tasks
        .iter()
        .filter(|task| task.status == Status::Cancelled)
        .count();
    let title = if journal.tasks.iter().any(|task| task.status.active()) {
        "Preparando seu PC."
    } else if journal.tasks.is_empty() {
        "Seu plano está vazio."
    } else if completed == journal.tasks.len() {
        "Tudo conferido."
    } else if failed > 0 {
        "Algumas etapas precisam de atenção."
    } else if action > 0 {
        "Agora é com você."
    } else {
        "Sua fila de operações."
    };
    (
        title.into(),
        format!(
            "{completed} de {} operações confirmadas · {action} requerem ação · {failed} falharam · {cancelled} canceladas",
            journal.tasks.len()
        ),
    )
}

pub fn populate_inventory(ui: &MainWindow, info: &SystemInfo) {
    fn known(value: &str) -> String {
        if value.trim().is_empty() {
            "Não identificado".into()
        } else {
            value.trim().into()
        }
    }
    ui.set_inventory_loading(false);
    ui.set_pc_supported(info.supported());
    ui.set_cpu_name(known(&info.cpu).into());
    ui.set_gpu_name(
        info.gpus
            .first()
            .map_or_else(|| "Não identificada".into(), |gpu| known(&gpu.name))
            .into(),
    );
    ui.set_gpu_caption(
        if info.gpus.len() > 1 {
            format!("{} GPUs detectadas · detalhes em Drivers", info.gpus.len())
        } else {
            "Identificada pelo Windows".into()
        }
        .into(),
    );
    ui.set_ram_size(
        if info.ram_bytes == 0 {
            "Não identificada".into()
        } else {
            format!("{:.1} GB", info.ram_bytes as f64 / 1_073_741_824.)
        }
        .into(),
    );
    ui.set_storage_count(
        format!(
            "{} {}",
            info.disks.len(),
            if info.disks.len() == 1 {
                "unidade"
            } else {
                "unidades"
            }
        )
        .into(),
    );
    ui.set_storage_caption(
        if info.disks.is_empty() {
            "Sem informação disponível".into()
        } else {
            info.disks
                .iter()
                .filter_map(|disk| disk.split(" · ").nth(1))
                .collect::<Vec<_>>()
                .join(" + ")
        }
        .into(),
    );
    ui.set_pc_label(format!("{} · {}\n{}", info.os, info.edition, info.architecture).into());
    let compatible = atlas::compatibility(info).is_ok();
    ui.set_atlas_compatible(compatible);
    ui.set_atlas_legacy_available(atlas::legacy::compatibility(info).is_ok());
    ui.set_atlas_title(
        if compatible {
            "Compatível e pronto para preparação."
        } else {
            "Este PC não atende à versão revisada."
        }
        .into(),
    );
    let reason = if compatible {
        "AME e playbook podem ser preparados. Aplicação é assistida, com requisitos oficiais externos; o Ruhtweaks não desativa proteções."
    } else if !info.architecture.starts_with("x86_64") {
        "A integração revisada é exclusiva para PCs x64. Esta arquitetura não foi validada."
    } else if ![26100, 26200].contains(&info.build) {
        "O Atlas revisado exige Windows 11 24H2 ou 25H2. A versão deste Windows não é compatível."
    } else {
        "Esta edição do Windows não foi validada. O Atlas revisado não oferece suporte ao Windows Home."
    };
    ui.set_atlas_reason(reason.into());
    ui.set_hardware(info.summary().into());
    ui.set_atlas_info(
        if info.build == 19045 {
            atlas::legacy::description(info)
        } else {
            atlas::description(info)
        }
        .into(),
    );
    ui.set_driver_info(drivers::description(info).into());
    ui.set_gpus(ModelRc::new(VecModel::from(info.gpus.iter().map(|gpu| {
        let vendor=match drivers::official_url(gpu) {Some(url) if url.contains("amd.com")=>"AMD",Some(url) if url.contains("nvidia.com")=>"NVIDIA",Some(_)=>"Intel",None=>"Não identificado"};
        GpuRow{name:known(&gpu.name).into(),vendor:vendor.into(),driver:format!("Driver detectado: {}",known(&gpu.driver)).into(),detail:format!("{}\n{}\n{}\nEscolha um driver compatível com este modelo e com o Windows. Abrir a fonte não confirma uma atualização.",gpu.hardware_id,info.os,drivers::official_url(gpu).unwrap_or("Consulte o fabricante do computador.")).into(),available:drivers::official_url(gpu).is_some()}
    }).collect::<Vec<_>>())));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn categories_preserve_every_catalog_index_and_selection() {
        let profile = Profile::default();
        let rows = app_rows(&profile);
        assert_eq!(
            rows.len(),
            catalog::APPS.iter().filter(|app| !app.browser).count()
        );
        for row in rows {
            let app = catalog::APPS
                .iter()
                .filter(|app| !app.browser)
                .nth(row.index as usize)
                .unwrap();
            assert_eq!(row.name, app.name);
            assert_eq!(row.selected, profile.apps.iter().any(|key| key == app.key));
        }
    }
    #[test]
    fn default_selection_includes_exactly_the_proposed_apps() {
        let (count, names) = selection_summary(&Profile::default());
        assert_eq!(count, 5);
        assert!(names.contains("Litecord"));
        assert!(!names.contains("Discord"));
    }
    #[test]
    fn atlas_summary_omits_the_browser_managed_by_atlas() {
        let profile = Profile {
            atlas: true,
            ..Profile::default()
        };
        let (count, names) = selection_summary(&profile);
        assert_eq!(count, 4);
        assert!(!names.contains("Brave"));
        assert!(names.contains("Litecord"));
        assert_eq!(profile.browser, Some(catalog::Browser::Brave));
    }
    #[test]
    fn empty_selection_is_honest() {
        let p = Profile {
            browser: None,
            apps: vec![],
            ..Profile::default()
        };
        assert_eq!(
            selection_summary(&p),
            (0, "Nenhum aplicativo selecionado.".into())
        );
    }
    #[test]
    fn pending_actions_are_never_reported_as_complete() {
        let mut journal =
            crate::installer::plan(&Profile::default(), &SystemInfo::default()).unwrap();
        journal.tasks[0].status = Status::RequiresAction;
        let (title, summary) = run_summary(&journal);
        assert_ne!(title, "Tudo conferido.");
        assert!(summary.contains("0 de"));
        assert_eq!(status_code(Status::RequiresAction), 6);
    }
    #[test]
    fn tweak_presentation_keeps_every_existing_action() {
        let rows = tweak_rows(&Profile::default(), "Estado de teste");
        assert_eq!(rows.len(), tweaks::TWEAKS.len());
        assert!(rows.iter().any(|t| t.category == "Arquivos" && !t.assisted));
    }
}
