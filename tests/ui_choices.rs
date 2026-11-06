use slint::{
    ComponentHandle, LogicalPosition, LogicalSize,
    platform::{
        Platform, PlatformError, PointerEventButton, WindowAdapter, WindowEvent,
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
    },
};
use std::rc::Rc;

slint::slint! {
    import { Choice } from "ui/theme.slint";
    export component Choices inherits Window {
        width: 320px; height: 180px;
        in-out property <int> selection: -1;
        in-out property <bool> option: false;
        in-out property <bool> enabled: true;
        out property <bool> first-checked: first.checked;
        out property <bool> second-checked: second.checked;
        out property <bool> option-checked: option-choice.checked;
        first := Choice {
            x: 8px; y: 8px; width: 300px; height: 32px;
            text: "Primeiro"; checked: root.selection == 0;
            toggled(value) => { root.selection = value ? 0 : -1; }
        }
        second := Choice {
            x: 8px; y: 48px; width: 300px; height: 32px;
            text: "Segundo"; checked: root.selection == 1;
            toggled(value) => { root.selection = value ? 1 : -1; }
        }
        option-choice := Choice {
            x: 8px; y: 88px; width: 300px; height: 32px;
            text: "Opção"; checked: root.option; enabled: root.enabled;
            toggled(value) => { root.option = value; }
        }
    }
}

struct TestPlatform(Rc<MinimalSoftwareWindow>);
impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.0.clone())
    }
}
fn click(window: &MinimalSoftwareWindow, x: f32, y: f32) {
    let position = LogicalPosition::new(x, y);
    window.dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    window.dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
}
fn render(window: &MinimalSoftwareWindow) -> Vec<slint::Rgb8Pixel> {
    slint::platform::update_timers_and_animations();
    let size = window.size();
    let mut pixels = vec![slint::Rgb8Pixel::default(); (size.width * size.height) as usize];
    window.request_redraw();
    window.draw_if_needed(|renderer| {
        renderer.render(&mut pixels, size.width as usize);
    });
    pixels
}
#[test]
fn choices_follow_the_model_and_can_be_unchecked_without_stale_squares() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(TestPlatform(window.clone()))).unwrap();
    let ui = Choices::new().unwrap();
    ui.show().unwrap();
    window.set_size(LogicalSize::new(320., 180.));
    let mut pixels = vec![slint::Rgb8Pixel::default(); 320 * 180];
    window.draw_if_needed(|renderer| {
        renderer.render(&mut pixels, 320);
    });

    click(&window, 20., 24.);
    assert_eq!(ui.get_selection(), 0);
    assert!(ui.get_first_checked());
    click(&window, 20., 64.);
    assert_eq!(ui.get_selection(), 1);
    assert!(ui.get_second_checked());
    assert!(
        !ui.get_first_checked(),
        "seleção anterior ficou presa no controle"
    );
    click(&window, 20., 64.);
    assert_eq!(ui.get_selection(), -1);
    assert!(!ui.get_first_checked() && !ui.get_second_checked());

    click(&window, 20., 104.);
    assert!(ui.get_option() && ui.get_option_checked());
    click(&window, 20., 104.);
    assert!(!ui.get_option() && !ui.get_option_checked());
    ui.set_option(true);
    assert!(ui.get_option_checked());
    ui.set_option(false);
    assert!(!ui.get_option_checked());
    ui.set_enabled(false);
    click(&window, 20., 104.);
    assert!(!ui.get_option());
    ui.hide().unwrap();

    let app = ruhtweaks::app_ui::MainWindow::new().unwrap();
    app.set_inventory_loading(false);
    app.set_page(1);
    app.show().unwrap();
    window.set_size(LogicalSize::new(620., 500.));
    render(&window);
    click(&window, 32., 181.);
    assert_eq!(
        app.get_browser_index(),
        4,
        "clicar novamente deve retirar Brave"
    );
    render(&window);
    click(&window, 32., 219.);
    assert_eq!(app.get_browser_index(), 1);
    render(&window);
    click(&window, 32., 257.);
    assert_eq!(app.get_browser_index(), 2);
    let pixels = render(&window);
    let at = |y: usize| pixels[y * 620 + 26];
    assert_ne!((at(219).r, at(219).g, at(219).b), (162, 232, 207));
    assert_eq!((at(257).r, at(257).g, at(257).b), (162, 232, 207));

    app.set_atlas_selected(true);
    let pixels = render(&window);
    assert_ne!(
        (
            pixels[257 * 620 + 26].r,
            pixels[257 * 620 + 26].g,
            pixels[257 * 620 + 26].b
        ),
        (162, 232, 207),
        "seletor de navegador deve sumir com Atlas"
    );
    assert_eq!(app.get_browser_index(), 2);
    app.set_atlas_selected(false);
    let pixels = render(&window);
    assert_eq!(
        (
            pixels[257 * 620 + 26].r,
            pixels[257 * 620 + 26].g,
            pixels[257 * 620 + 26].b
        ),
        (162, 232, 207),
        "preferência deve reaparecer ao recusar Atlas"
    );

    app.set_page(2);
    app.set_atlas_compatible(true);
    app.set_atlas_selected(true);
    render(&window);
    click(&window, 32., 181.);
    assert!(
        !app.get_atlas_selected(),
        "Manter meu Windows deve retirar Atlas"
    );
    render(&window);
    click(&window, 32., 219.);
    assert!(app.get_atlas_selected());
    let pixels = render(&window);
    assert_ne!(
        (
            pixels[181 * 620 + 26].r,
            pixels[181 * 620 + 26].g,
            pixels[181 * 620 + 26].b
        ),
        (162, 232, 207)
    );
    click(&window, 32., 219.);
    assert!(!app.get_atlas_selected());
    app.set_atlas_compatible(false);
    app.set_atlas_legacy_available(false);
    app.set_atlas_selected(true);
    render(&window);
    click(&window, 32., 181.);
    assert!(
        !app.get_atlas_selected(),
        "é permitido recusar uma seleção incompatível recuperada"
    );
    app.hide().unwrap();
}
