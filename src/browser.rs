use crate::{catalog::Browser, state::Profile};
pub mod policy;
pub fn apply(
    profile: &Profile,
    store: &crate::state::Store,
) -> crate::Result<(crate::state::Status, String)> {
    if profile.browser == Some(Browser::Brave) && profile.google_search {
        policy::apply_with(&mut policy::NativeRegistry, store)?;
        return Ok((crate::state::Status::Completed, "Google configurado por políticas oficiais HKCU do Brave, relidas e verificadas. Shields e dados pessoais não foram alterados. O padrão fica gerenciado até a reversão em Aplicativos; confira brave://policy (recarregue as políticas se o navegador estiver aberto). A leitura confirma o registro, não a interface do navegador.".into()));
    }
    Ok((crate::state::Status::RequiresAction, guidance(profile)))
}
pub fn rollback(store: &crate::state::Store) -> crate::Result<()> {
    policy::rollback_with(&mut policy::NativeRegistry, store)
}
pub fn guidance(profile: &Profile) -> String {
    let Some(browser) = profile.browser else {
        return "Nenhum navegador selecionado.".into();
    };
    let mut message=match browser{
        Browser::Brave=>"Brave: Google por políticas oficiais HKCU, com backup e reversão. O padrão fica gerenciado enquanto a política existir. Shields não é alterado. Verificação em brave://policy.",
        Browser::Firefox=>"Firefox: Configurações > Pesquisa > Mecanismo de pesquisa padrão > Google. A preferência é persistida pelo próprio navegador.",
        Browser::Chrome=>"Chrome: Configurações > Mecanismo de pesquisa > Google.",
        Browser::Edge=>"Edge: Configurações > Privacidade, pesquisa e serviços > Barra de endereços e pesquisa > Google.",
    }.to_string();
    message.push_str("\nUse Configurações do Windows > Aplicativos padrão para definir o navegador. Esta etapa permanece 'Requer ação': não há verificação automática da preferência pessoal nesta versão.");
    message
}
pub fn help_url(browser: Browser) -> &'static str {
    match browser {
        Browser::Brave => "https://support.brave.app/hc/en-us",
        Browser::Firefox => "https://support.mozilla.org/pt-BR/",
        Browser::Chrome => "https://support.google.com/chrome/",
        Browser::Edge => "https://support.microsoft.com/microsoft-edge",
    }
}
