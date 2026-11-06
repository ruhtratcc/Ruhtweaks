use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Browser {
    Brave,
    Firefox,
    Chrome,
    Edge,
}
impl Browser {
    pub fn key(self) -> &'static str {
        match self {
            Self::Brave => "brave",
            Self::Firefox => "firefox",
            Self::Chrome => "chrome",
            Self::Edge => "edge",
        }
    }
    pub fn from_index(index: i32) -> Option<Self> {
        [Self::Brave, Self::Firefox, Self::Chrome, Self::Edge]
            .get(index as usize)
            .copied()
    }
    pub fn index(self) -> i32 {
        match self {
            Self::Brave => 0,
            Self::Firefox => 1,
            Self::Chrome => 2,
            Self::Edge => 3,
        }
    }
}
#[derive(Clone, Debug)]
pub struct App {
    pub key: &'static str,
    pub name: &'static str,
    pub winget: Option<&'static str>,
    pub official: Option<&'static str>,
    pub note: &'static str,
    pub browser: bool,
}
pub fn supports_updates(key: &str) -> bool {
    find(key).is_some_and(|app| app.winget.is_some())
        && !matches!(key, "riot_lol_br" | "riot_valorant_br")
}
pub const APPS: &[App] = &[
    App {
        key: "riot_lol_br",
        name: "League of Legends · Brasil",
        winget: Some("RiotGames.LeagueOfLegends.BR"),
        official: Some("https://www.leagueoflegends.com/pt-br/download/"),
        note: "Opcional: instala o jogo, Riot Client e pode exigir Vanguard (driver de anticheat) e reinício. Revise os componentes no instalador oficial. Não é uma instalação apenas do cliente.",
        browser: false,
    },
    App {
        key: "riot_valorant_br",
        name: "VALORANT · Brasil",
        winget: Some("RiotGames.Valorant.BR"),
        official: Some("https://playvalorant.com/pt-br/download/"),
        note: "Opcional: instala o jogo e Riot Client; Vanguard inclui driver de anticheat e pode exigir reinício e recursos de segurança compatíveis. Revise no instalador oficial. Sem contornar TPM/Secure Boot.",
        browser: false,
    },
    App {
        key: "brave",
        name: "Brave",
        winget: Some("Brave.Brave"),
        official: Some("https://brave.com/download/"),
        note: "Navegador · Shields permanece ativo",
        browser: true,
    },
    App {
        key: "firefox",
        name: "Firefox",
        winget: Some("Mozilla.Firefox"),
        official: Some("https://www.mozilla.org/firefox/"),
        note: "Navegador",
        browser: true,
    },
    App {
        key: "chrome",
        name: "Chrome",
        winget: Some("Google.Chrome"),
        official: Some("https://www.google.com/chrome/"),
        note: "Navegador",
        browser: true,
    },
    App {
        key: "edge",
        name: "Edge",
        winget: Some("Microsoft.Edge"),
        official: Some("https://www.microsoft.com/edge"),
        note: "Navegador · pode já estar instalado",
        browser: true,
    },
    App {
        key: "litecord",
        name: "Litecord",
        winget: None,
        official: Some("https://github.com/Ak4ai/Litecord/releases"),
        note: "Ak4ai/Litecord · instalação silenciosa verificada",
        browser: false,
    },
    App {
        key: "discord",
        name: "Discord",
        winget: Some("Discord.Discord"),
        official: Some("https://discord.com/download"),
        note: "Cliente oficial · revisar inicialização no instalador",
        browser: false,
    },
    App {
        key: "steam",
        name: "Steam",
        winget: Some("Valve.Steam"),
        official: Some("https://store.steampowered.com/about/"),
        note: "Loja de jogos · revisar inicialização no primeiro uso",
        browser: false,
    },
    App {
        key: "epic",
        name: "Epic Games Launcher",
        winget: Some("EpicGames.EpicGamesLauncher"),
        official: Some("https://store.epicgames.com/download"),
        note: "Loja de jogos · revisar inicialização no primeiro uso",
        browser: false,
    },
    App {
        key: "riot",
        name: "Riot Client",
        winget: None,
        official: Some("https://www.riotgames.com/en"),
        note: "Assistido · escolha o jogo; pode incluir Vanguard",
        browser: false,
    },
    App {
        key: "7zip",
        name: "7-Zip",
        winget: Some("7zip.7zip"),
        official: Some("https://www.7-zip.org/"),
        note: "Compactação de arquivos",
        browser: false,
    },
    App {
        key: "vlc",
        name: "VLC",
        winget: Some("VideoLAN.VLC"),
        official: Some("https://www.videolan.org/vlc/"),
        note: "Reprodutor de mídia",
        browser: false,
    },
    App {
        key: "vscode",
        name: "Visual Studio Code",
        winget: Some("Microsoft.VisualStudioCode"),
        official: Some("https://code.visualstudio.com/"),
        note: "Editor de código",
        browser: false,
    },
    App {
        key: "obs",
        name: "OBS Studio",
        winget: Some("OBSProject.OBSStudio"),
        official: Some("https://obsproject.com/"),
        note: "Gravação e transmissão",
        browser: false,
    },
    App {
        key: "everything",
        name: "Everything",
        winget: Some("voidtools.Everything"),
        official: Some("https://www.voidtools.com/"),
        note: "Pesquisa · serviço de indexação opcional do fabricante",
        browser: false,
    },
];
pub fn find(key: &str) -> Option<&'static App> {
    APPS.iter().find(|app| app.key == key)
}
