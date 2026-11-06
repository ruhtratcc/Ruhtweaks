//! Renderiza a interface real com dados de teste, sem consultar ou modificar o host.
use ruhtweaks::{
    app_ui::{MainWindow, presentation},
    state::{Profile, Status},
    system_info::{Gpu, SystemInfo},
};
use slint::{
    ComponentHandle, ModelRc, VecModel,
    platform::{
        Platform, PlatformError, WindowAdapter,
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
    },
};
use std::{fs, io::Write, path::Path, rc::Rc};

struct RenderPlatform {
    window: Rc<MinimalSoftwareWindow>,
}
impl Platform for RenderPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.window.clone())
    }
}
fn bitmap(
    path: &Path,
    pixels: &[slint::Rgb8Pixel],
    width: u32,
    height: u32,
) -> std::io::Result<()> {
    let stride = (width * 3 + 3) & !3;
    let size = 54 + stride * height;
    let mut file = std::io::BufWriter::new(fs::File::create(path)?);
    file.write_all(b"BM")?;
    file.write_all(&size.to_le_bytes())?;
    file.write_all(&[0; 4])?;
    file.write_all(&54u32.to_le_bytes())?;
    file.write_all(&40u32.to_le_bytes())?;
    file.write_all(&width.to_le_bytes())?;
    file.write_all(&height.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&24u16.to_le_bytes())?;
    file.write_all(&[0; 24])?;
    let padding = vec![0; (stride - width * 3) as usize];
    for y in (0..height).rev() {
        for x in 0..width {
            let pixel = pixels[(y * width + x) as usize];
            file.write_all(&[pixel.b, pixel.g, pixel.r])?;
        }
        file.write_all(&padding)?;
    }
    file.flush()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args().nth(1).unwrap_or("work/ui-qa".into());
    fs::create_dir_all(&output)?;
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(RenderPlatform {
        window: window.clone(),
    }))?;
    let info = SystemInfo {
        os: "Windows 11 Pro".into(),
        build: 26200,
        edition: "Professional".into(),
        architecture: "x86_64".into(),
        cpu: "AMD Ryzen 7 7700 · 8 núcleos".into(),
        ram_bytes: 32 * 1_073_741_824,
        gpus: vec![
            Gpu {
                name: "NVIDIA GeForce RTX 4070".into(),
                hardware_id: "PCI\\VEN_10DE&DEV_TEST".into(),
                driver: "Detectado no Windows".into(),
            },
            Gpu {
                name: "AMD Radeon Graphics".into(),
                hardware_id: "PCI\\VEN_1002&DEV_TEST".into(),
                driver: "Detectado no Windows".into(),
            },
        ],
        disks: vec![
            "SSD principal · 1000.0 GB · NVMe".into(),
            "Unidade de dados · 2000.0 GB · SATA".into(),
        ],
        ..Default::default()
    };
    for (label, width, height, scale) in [
        ("desktop", 1100, 800, 1.),
        ("compact", 800, 600, 1.),
        ("minimum", 620, 500, 1.),
        ("dpi200", 950, 700, 2.),
    ] {
        for scenario in 0..=22 {
            let page = match scenario {
                9 | 12 => 0,
                20 | 21 => 2,
                22 => 1,
                10 => 1,
                11 | 13 | 14 | 16..=19 => 7,
                3..=5 | 15 => 6,
                page => page,
            };
            let ui = MainWindow::new()?;
            let mut profile = Profile {
                atlas: scenario == 4 || scenario >= 17,
                atlas_legacy: scenario == 20,
                ..Profile::default()
            };
            if page == 6 {
                profile.include_automatic_setup(&info);
            }
            ui.set_atlas_selected(profile.atlas);
            ui.set_atlas_legacy(profile.atlas_legacy);
            ui.set_drivers_selected(profile.drivers);
            ui.set_google_selected(profile.google_search);
            ui.set_changes_summary(
                profile
                    .tweaks
                    .iter()
                    .filter_map(|key| ruhtweaks::tweaks::find(key).map(|t| t.title))
                    .collect::<Vec<_>>()
                    .join(" · ")
                    .into(),
            );
            presentation::populate_inventory(&ui, &info);
            let rows = presentation::app_rows(&profile);
            ui.set_apps(ModelRc::new(VecModel::from(rows.clone())));
            ui.set_tweaks(ModelRc::new(VecModel::from(presentation::tweak_rows(
                &profile,
                "Estado atual: extensões visíveis.",
            ))));
            let (count, summary) = presentation::selection_summary(&profile);
            ui.set_selected_count(count);
            ui.set_selected_summary(summary.into());
            ui.set_page(page);
            if page == 1 {
                ui.set_updates(ModelRc::new(VecModel::from(vec![
                    ruhtweaks::app_ui::UpdateRow {
                        browser: true,
                        key: "brave".into(),
                        name: "Brave".into(),
                        from: "155.1.97.59".into(),
                        to: "156.1.98.63".into(),
                        selected: false,
                    },
                ])));
                ui.set_updates_status(
                    "Dados de teste: marque uma atualização e revise o plano.".into(),
                );
            }
            ui.set_wizard((1..=6).contains(&page));
            ui.set_wizard_step(if page == 0 {
                0
            } else if page == 2 {
                1
            } else if page == 1 {
                2
            } else {
                3
            });
            let mut journal = ruhtweaks::installer::plan(&profile, &info)?;
            if (16..=19).contains(&scenario) {
                ui.set_handoff_kind(if scenario == 16 { 0 } else { 1 });
                ui.set_handoff_allowed(scenario < 19);
                ui.set_handoff_description("DADOS DE TESTE · fonte, versão e hash aparecem aqui. Nenhum instalador será iniciado pelo renderizador.".into());
                journal.tasks[0].status = if scenario == 18 {
                    Status::AwaitingReboot
                } else {
                    Status::RequiresConfirmation
                };
                if scenario == 18 {
                    ui.set_handoff_kind(-1);
                }
            }
            if page == 7 && scenario < 16 {
                journal.tasks[0].status = Status::Completed;
                journal.tasks[1].status = Status::RequiresAction;
            }
            if scenario == 10 {
                journal.tasks.clear();
                ui.set_browser_index(4);
                ui.set_selected_count(0);
                ui.set_selected_summary("Nenhum aplicativo selecionado.".into());
                let empty = Profile {
                    browser: None,
                    apps: vec![],
                    ..Profile::default()
                };
                let rows = presentation::app_rows(&empty);
                ui.set_apps(ModelRc::new(VecModel::from(rows.clone())));
            }
            if scenario == 11 {
                journal.tasks[1].status = Status::Failed;
                ui.set_notice_tone(3);
                ui.set_status("Falha de teste: a assinatura do instalador não pôde ser verificada. Nenhuma instalação foi executada.".into());
            }
            if scenario == 13 {
                for task in &mut journal.tasks {
                    task.status = Status::Completed;
                }
            }
            if scenario == 14 {
                journal.tasks[2].status = Status::Installing;
                ui.set_busy(true);
                ui.set_status("Steam · Instalando · Progresso indeterminado".into());
            }
            ui.set_queue(ModelRc::new(VecModel::from(
                journal
                    .tasks
                    .iter()
                    .map(|t| ruhtweaks::app_ui::QueueRow {
                        title: t.title.clone().into(),
                        status: t.status.label().into(),
                        detail: t.detail.clone().into(),
                        code: presentation::status_code(t.status),
                    })
                    .collect::<Vec<_>>(),
            )));
            let (title, summary) = presentation::run_summary(&journal);
            ui.set_run_title(title.into());
            ui.set_run_summary(summary.into());
            ui.set_details(journal.report().into());
            ui.set_plan_ready(page == 6);
            ui.set_approval(false);
            ui.set_data_path("Dados de teste · sem acesso ao host".into());
            if scenario == 9 {
                ui.set_inventory_loading(true);
                ui.set_cpu_name("Verificando…".into());
                ui.set_gpu_name("Verificando…".into());
                ui.set_ram_size("—".into());
                ui.set_storage_count("—".into());
                ui.set_atlas_reason("Aguarde a consulta local do Windows.".into());
            }
            if scenario == 12 {
                ui.set_more_open(true);
            }
            if scenario == 15 {
                let mut unknown = info.clone();
                unknown.gpus = vec![Gpu {
                    name: "Adaptador sem fabricante identificado".into(),
                    ..Default::default()
                }];
                presentation::populate_inventory(&ui, &unknown);
            }
            ui.set_needs_check(journal.tasks.iter().any(|t| {
                matches!(
                    t.status,
                    Status::AwaitingReboot | Status::AwaitingVerification
                )
            }));
            ui.set_remaining_pending(journal.tasks.iter().any(|t| t.status == Status::Pending));
            if scenario == 20 || scenario == 21 {
                let legacy = SystemInfo {
                    build: 19045,
                    edition: if scenario == 20 {
                        "Professional".into()
                    } else {
                        "Core".into()
                    },
                    ..info.clone()
                };
                presentation::populate_inventory(&ui, &legacy);
            }
            ui.show()?;
            window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged {
                scale_factor: scale,
            });
            window.set_size(slint::LogicalSize::new(width as f32, height as f32));
            slint::platform::update_timers_and_animations();
            let size = window.size();
            let mut pixels = vec![slint::Rgb8Pixel::default(); (size.width * size.height) as usize];
            ui.window().request_redraw();
            assert!(
                window.draw_if_needed(|renderer| {
                    renderer.render(&mut pixels, size.width as usize);
                }),
                "a janela precisa renderizar"
            );
            bitmap(
                &Path::new(&output).join(format!("{label}-{scenario}.bmp")),
                &pixels,
                size.width,
                size.height,
            )?;
            ui.hide()?;
        }
    }
    println!(
        "92 capturas renderizadas com dados de teste. Nenhuma instalação ou ajuste foi executado."
    );
    Ok(())
}
