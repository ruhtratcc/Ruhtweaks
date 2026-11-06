# Catálogo e mecanismo de instalação

Fontes consultadas em 01/11/2026. IDs conferidos no repositório oficial [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs/tree/master/manifests). A consulta de disponibilidade é repetida pelo aplicativo antes de instalar; o catálogo não fixa uma URL de instalador que pode expirar.

| Aplicativo | ID exato / fonte |
|---|---|
| Brave | `Brave.Brave` |
| Firefox | `Mozilla.Firefox` |
| Chrome | `Google.Chrome` |
| Edge | `Microsoft.Edge` |
| Discord | `Discord.Discord` |
| Steam | `Valve.Steam` |
| Epic Games Launcher | `EpicGames.EpicGamesLauncher` |
| 7-Zip | `7zip.7zip` |
| VLC | `VideoLAN.VLC` |
| Visual Studio Code | `Microsoft.VisualStudioCode` |
| OBS Studio | `OBSProject.OBSStudio` |
| Everything | `voidtools.Everything` |
| Litecord | [Ak4ai/Litecord](https://github.com/Ak4ai/Litecord/releases), identidade confirmada pelo usuário |
| Riot Client | [Riot Games](https://www.riotgames.com/en), assistido; pacote independente `RiotGames.RiotClient` não encontrado |
| League of Legends Brasil (opcional) | `RiotGames.LeagueOfLegends.BR` — inclui jogo, Riot Client e componentes oficiais |
| VALORANT Brasil (opcional) | `RiotGames.Valorant.BR` — inclui jogo, Riot Client e Vanguard |

Para cada pacote WinGet, o motor consulta `list --id ID --exact --source winget --disable-interactivity`. Sucesso exige o ID completo nos tokens da saída; ausência é reconhecida somente pelo HRESULT documentado do cliente (`0x8A150014`). Outro código, falha de rede ou ID truncado viram detecção inconclusiva. Isso evita instalar novamente por uma interpretação insegura de saída localizada.

Com autorização do usuário aos termos, `show` consulta disponibilidade e a instalação usa:

```text
winget install --id ID --exact --source winget (--silent OU --interactive) --no-upgrade
  --accept-source-agreements --accept-package-agreements --disable-interactivity
```

Os parênteses acima indicam escolha do motor, não argumentos literais. Argumentos individuais são passados a `Command`, sem shell. Brave, Firefox, Chrome, Edge, 7-Zip, VLC, VS Code e OBS podem usar `--silent`; os demais permanecem interativos para revisar inicialização/componentes. Desabilitar a preferência silenciosa mantém todos interativos. WinGet resolve os switches do instalador pelo manifesto, sem overrides locais. `--disable-interactivity` impede prompts de console, mantendo UAC normal. Não usamos `--force`, bypass de hash/segurança, `--allow-reboot`, parâmetros privados nem fontes substitutas.

Antes de aceitar os termos, use o link da fonte oficial para ler licenças e componentes. Ao não autorizar os termos, a instalação fica pendente. O instalador pode exigir ações adicionais; a UI não anuncia progresso percentual quando o CLI não fornece um contrato estável para isso. Código zero do processo sozinho não basta: uma nova detecção confirma o produto instalado.

Tudo é sequencial. Atualizações usam o JSON oficial de `export --include-versions --source winget` e a ordem de `show --versions`; somente versões instaladas reconhecidas abaixo da primeira versão anunciada são oferecidas. A aprovação fixa `from/to` no perfil. `upgrade --id ID --exact --source winget --version VERSAO` atualiza um pacote por vez; o resultado só é concluído quando a versão alvo é relida. Uma atualização substitui a tarefa de instalação do mesmo app, evitando dois instaladores. Não há atualização global, downgrade presumido ou reinstalação automática.

Litecord usa o asset `Litecord-Setup-x64.exe` v1.3.1, digest oficial fixado e cache por hash. Foi inspecionado sem execução: SHA-256 correspondeu, Authenticode retornou NotSigned. O `installer.iss` confirma Inno Setup, AppId `{6E4F5D7B-9A3C-4D2E-8B1F-5C7D9A2B4E6F}`, editor declarado e URL do projeto. A detecção consulta o produto e seu executável PE em HKCU/HKLM, nas duas visões. Na 0.3.0, a aprovação final do plano autoriza a instalação silenciosa e informa a ausência de assinatura; assinatura inválida/inconclusiva bloqueia. Parâmetros oficiais impedem fechamento forçado e reboot. O pacote pode abrir o cliente ao terminar. Código zero exige nova detecção; hash GitHub não equivale a assinatura Authenticode. Consulte [integração atual](INTEGRACAO-030.md) para o contrato completo.

Riot: marque explicitamente LoL Brasil ou VALORANT Brasil se quiser instalar também o jogo. Esses IDs e URLs riotcdn foram conferidos nos manifestos Microsoft, sem inventar download independente do cliente. A tarefa genérica Riot é omitida quando um jogo Riot foi escolhido; cada jogo usa o instalador oficial interativo e pode exigir Vanguard/reinício. Nenhum jogo vem marcado por padrão, nenhum componente é removido e nenhum requisito TPM/Secure Boot é contornado. Apenas Riot Client continua assistido. Everything, Steam, Epic e Discord mantêm revisão de componentes/inicialização no instalador.

Os jogos Riot usam o próprio atualizador e não entram na atualização WinGet do Ruhtweaks. A presença do registro do bootstrapper fica como Requer ação até a conclusão de login/download/componentes pelo usuário, sem repetição automática do instalador.

Na 0.3.0, Litecord usa a aprovação final do plano para instalação silenciosa, sem uma segunda confirmação no Ruhtweaks. Os controles do Windows e a verificação posterior permanecem. Extensões de navegador não são gerenciadas. Contrato atual: INTEGRACAO-030.md.

