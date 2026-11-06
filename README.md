# Ruhtweaks 0.3.3

Assistente Windows em **Rust + Slint**, sem WebView, servidor local, telemetria, serviço próprio ou tarefa permanente.

## Usar

Extraia o ZIP Windows e abra `ruhtweaks.exe`.

1. Confira seu PC e clique em **Começar**.
2. Atlas vem selecionado em uma configuração nova compatível. Desmarque ou escolha **Manter meu Windows** se não quiser usar.
3. Marque os aplicativos.
4. Confira o plano e clique em **Iniciar e aceitar termos**.

Não há menu lateral. Fontes e opções extras ficam recolhidas. Os detalhes técnicos e logs aparecem na execução. A aprovação final não é substituída pelo perfil sugerido.

Com Atlas selecionado, a escolha de navegador aparece somente no assistente do Atlas. O Ruhtweaks oculta os seletores e não agenda instalação, atualização ou configuração de navegador por conta própria. A preferência anterior fica salva para reaparecer ao recusar Atlas. Isso não significa que o navegador foi instalado: aplicação e escolhas do Atlas continuam assistidas e verificadas pelos limites documentados.

Não existe mais a etapa “Os últimos detalhes”. Ao preparar o plano, o app inclui automaticamente driver AMD quando detectado, Google no Brave fora do fluxo Atlas e extensões de arquivos visíveis, com verificação e reversão onde disponíveis. A revisão informa essas ações antes da aprovação única; não há checkbox de driver/ajustes. Energia, inicialização, Modo de Jogo, taxa do monitor e limpeza ampla não são alterados automaticamente: não existe benefício universal ou automação segura revisada para esses casos. Ferramentas manuais, suporte e reversões ficam em **Logs e detalhes do PC → Ferramentas e reversões**, após entrar na execução.

A revisão 0.3.1 corrige seleções que ficavam marcadas após trocar de opção. Clicar novamente no navegador selecionado retira sua seleção. Preferências existentes, inclusive recusar Atlas, são preservadas; a sugestão é aplicada somente no primeiro inventário de um perfil novo, sem fila anterior. Windows 10 legado mantém o aviso de não recomendado e todos os bloqueios de compatibilidade/segurança.

## Automação real

- **Litecord 1.3.1:** após a aprovação final, baixa/verifica e instala silenciosamente com parâmetros oficiais Inno. Não pede outra confirmação no Ruhtweaks. O EXE não possui assinatura digital; isso é informado antes de iniciar. Windows pode exibir ou bloquear avisos. Nenhum bloqueio é ignorado.
- **AMD:** busca drivers Display atribuídos/recomendados pelo Windows Update, exige correspondência exata com IDs do hardware e instala por WUA. Pode pedir **UAC**. Não solicita o instalador Adrenalin ou chipset. Sem oferta compatível, fica pendente; o app não inventa pacote ou versão. A ferramenta AMD oficial permanece como alternativa assistida em Mais ações.
- **WinGet:** Brave, Firefox, Chrome, Edge, 7-Zip, VLC, VS Code e OBS podem usar modo silencioso. Steam, Epic, Discord, Everything e jogos Riot continuam interativos para revisar componentes/inicialização. Versões aprovadas podem ser atualizadas individualmente.
- **Brave:** Google por política HKCU reversível. Shields e dados pessoais não são modificados. Outros navegadores e a escolha do navegador padrão continuam assistidos.
- **Ajustes:** extensões de arquivos e plano Equilibrado são aplicados/verificados com backup e reversão. Inicialização, monitor, jogos e limpeza do Windows continuam assistidos.

Extensões de navegador não são gerenciadas nesta versão. Preferências antigas são migradas sem reinstalar aplicativos já confirmados e sem ampliar aprovação de execução.

## Atlas

- Windows 11 validado: preparação AME/APBX e aplicação assistida, com as verificações de compatibilidade existentes.
- Windows 10 22H2 x64 validado: opção **Atlas 0.4.1 — legado, não recomendado**. O arquivo indicado pelo usuário foi comparado ao download oficial e o hash foi fixado. Preparação verificada; aplicação assistida, sem confirmação automática do resultado legado.

Nenhum dos fluxos desativa Defender, firewall ou UCPD. Os playbooks revisados exigem alterações de proteção incompatíveis com essas regras em partes do processo oficial; se forem exigidas, interrompa. Backup/instalação limpa e reinício são decisões manuais. Não há formatação nem reboot automático. Consulte `docs/INTEGRACAO-030.md`.

## Eficiência e retomada

UI por eventos, sem timer permanente de consulta. Trabalho lento fora da thread gráfica; uma instalação/download por vez. WUA é acionado somente quando drivers constarem do plano aprovado. A busca do Windows Update pode usar recursos do próprio Windows temporariamente; não permanece monitorando pelo Ruhtweaks.

O helper AMD é o próprio EXE, elevado somente durante aquela operação. Não cria serviço, autorun ou tarefa agendada. O helper não grava arquivos em caminhos do usuário com privilégio administrativo; somente o processo normal grava seus recibos. Resultados são revalidados, e arquivos necessários à retomada não são removidos prematuramente.

Dados em `%LOCALAPPDATA%\Ruhtweaks`. Schema 4 mantém backups de perfis/journals 1–3. Reinicie quando desejar, reabra e use **Verificar conclusão**. Falhas precisam de nova aprovação; instalações confirmadas não são repetidas. Rollback completo de instaladores/driver/Atlas não é fornecido.

## Compilar

Windows x64, toolchain fixada em `rust-toolchain.toml`; para MSVC, Visual Studio Build Tools C++ e Windows SDK:

```powershell
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --release --locked
cargo build --release --locked
.\target\release\ruhtweaks.exe
```

Neste workspace, a toolchain GNU isolada usa LLD:

```powershell
.\scripts\dev.ps1 -CargoArgs @('clippy','--locked','--all-targets','--jobs','2','--','-D','warnings')
.\scripts\dev.ps1 test --release --locked --jobs 2
.\scripts\dev.ps1 build --release --locked --jobs 2
.\scripts\package.ps1
```

O wrapper não instala ferramentas globalmente. Diagnósticos: `--inventory`, `--dry-run examples/profile.json`, `--simulate`. Release Windows não possui console; redirecione stdout para arquivo nesses modos. O modo interno AMD exige solicitação íntegra, recente, plano aprovado e processo autorizador ativo; não é uma CLI geral de instalação.

## Estrutura e dependências

Mantidos `app_ui`, `system_info`, `installer`, `catalog`, `atlas`, `drivers`, `browser`, `tweaks`, `state` e `security`. Novos arquivos principais: `src/drivers/amd_update.rs`, `src/drivers/amd_update/native.rs`, `src/atlas/legacy.rs`, `src/state/legacy.rs`, `tests/wizard_automation.rs`. UI em `ui/app.slint` e componentes/tokens reutilizados.

Slint/winit/software para UI nativa e acessibilidade; serde para estado; reqwest/TLS Windows para HTTPS; windows/windows-sys/WMI/winreg para APIs nativas; sha2, tempfile, fs2, tracing e thiserror para integridade, atomicidade, lock, logs e erros. Não foi adicionada família de dependências. Release usa opt-level s, thin LTO e símbolos removidos, mantendo unwind.

GPL-3.0-only. Licenças de terceiros em `docs/licenses`. Executável sem assinatura comercial. `Cargo.lock` e toolchain fixados permitem repetir o processo de build; identidade byte a byte entre máquinas não foi demonstrada.

Resultados executados e limites de homologação em `docs/VALIDACAO.md`. Instalações reais e ciclos de reboot precisam de VM autorizada; testes de operações usam executores simulados. Não há benchmarking de jogos/FPS nem promessa de ganho universal.
