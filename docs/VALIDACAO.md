# Validação desta entrega

> Para a interface redesenhada, consulte **REDESIGN.md**, com os novos testes e a verificação por renderização Slint. Os registros abaixo descrevem a primeira entrega.

Ambiente: Windows x64. Data: 01/11/2026 (America/Sao_Paulo). Rust `1.99.1`, Slint `1.13.1`, alvo `x86_64-pc-windows-gnu`. Toolchain e LLVM-MinGW ficam isolados em `work/`; nenhuma instalação global de ferramentas foi realizada.

## Comandos executados

```powershell
.\scripts\dev.ps1 check --locked
.\scripts\dev.ps1 fmt --all
.\scripts\dev.ps1 -CargoArgs @('fmt','--all','--','--check')
.\scripts\dev.ps1 -CargoArgs @('clippy','--locked','--all-targets','--','-D','warnings')
.\scripts\dev.ps1 test --locked
```

Resultados: checagem Rust/Slint aprovada; formatação aprovada; Clippy aprovado com avisos tratados como erros; **42 testes aprovados: 2 unitários e 40 de integração**, sem falhas ou casos ignorados. Os testes não instalam aplicativos nem aplicam tweaks no computador. Usam executores simulados e diretórios temporários.

A suíte cobre seleção e deduplicação, IDs exatos, Discord, identidade do Litecord, builds/edições/arquitetura Atlas, roteamento por hardware ID, saída de detecção inconclusiva, transições inválidas, perfis inválidos, hash de perfil, proteção contra injeção de ações no journal, persistência e substituição atômica, lock de instância, cancelamento conservador, recuperação, preservação de tarefas concluídas, revalidação sem reinstalar, hashes e rejeição de URLs não autorizadas.

## Diagnóstico em execução

Foram executados com sucesso no binário compilado:

```powershell
.\target\debug\ruhtweaks.exe --help
.\target\debug\ruhtweaks.exe --inventory
.\target\debug\ruhtweaks.exe --dry-run .\examples\profile.json
.\target\debug\ruhtweaks.exe --simulate
```

Todos retornaram código zero. O inventário real identificou SO/build/edição, CPU, RAM, GPU com PNP hardware ID e driver, discos, adaptadores ativos e modo atual do monitor. O App Installer registrado foi localizado e sua assinatura validada. O perfil padrão foi planejado sem alterações do Windows. A simulação completou somente operações falsas e declarou isso no relatório.

Também foi aberta a janela Slint com `--data-dir` apontando para `work/ui-data`. O processo expôs a janela `Ruhtweaks`, permaneceu responsivo e salvou o perfil padrão correto. Nenhuma execução foi aprovada. O helper de Computer Use falhou por timeout em `list_windows`, inclusive após reinicialização: **não foi possível verificar visualmente o layout nem testar os cliques**, e isso não é registrado como aprovação visual.

## Falhas encontradas durante a preparação

A toolchain GNU inicialmente não tinha `dlltool` acessível. As ferramentas locais foram preparadas sem modificar PATH global. Depois, o linker GNU produziu um executável que falhava antes de `main` com `STATUS_ACCESS_VIOLATION`. Um programa Rust mínimo funcionava; religar o mesmo aplicativo com LLVM LLD fez `--help` executar normalmente. `scripts/dev.ps1` seleciona LLD por um alias local em `work/toolchain`, e todos os testes acima foram executados novamente com essa configuração. Isso não constitui uma alegação de defeito geral do Rust/Windows; é o resultado observado desta combinação local de ferramentas e import libraries.

Erros de sintaxe/configuração do Slint e quatro sugestões de Clippy foram corrigidos antes das verificações aprovadas. A execução inicial dos testes que falhou antes da listagem não foi contabilizada como sucesso.

## Não verificado por decisão de segurança/ambiente

- Instalações reais WinGet, UAC e comportamento dos instaladores: exigem consentimento em VM ou PC alvo. Nenhum app do catálogo foi instalado ou removido durante o desenvolvimento.
- Aplicação/reversão real de ajustes: a suíte não modifica o registro do host. A implementação de backup/releitura existe, mas requer homologação autorizada em VM.
- Execução AME/Atlas, instalação de drivers, preferências efetivas do navegador, extensões e reinicializações: não executadas. Atlas continua bloqueado pelo conflito de requisitos de segurança.
- ARM64, compilação MSVC, execução em outras builds de Windows e leitores de tela: não testados. O workflow GitHub está preparado, sem execução remota nesta entrega.
- Ganho de performance, FPS e comparações antes/depois: não medidos nem incluídos no aplicativo.
- Reprodutibilidade byte a byte e assinatura comercial do Ruhtweaks: não verificadas/fornecidas.

Para homologar: use uma VM com snapshot, execute primeiro o dry-run, confira os termos e opções de inicialização de um único app, aprove essa instalação e verifique detecção posterior, cancelamento e retomada. Só depois amplie para outros pacotes e ajustes.

## Release e distribuição

`cargo build --release --locked` executado com sucesso pelo script de empacotamento. Executável Windows x64 otimizado: 7.294.464 bytes (aproximadamente 7 MiB). A tabela de imports do PE foi inspecionada: somente DLLs do próprio Windows, sem DLLs GNU de runtime adicionais.

No executável **do pacote final**, `--help`, `--simulate` e `--inventory` retornaram código zero. A saída de simulação continuou explicitamente marcada como simulada. Foi executado também o exemplo de consulta somente leitura do motor WinGet: Brave retornou `Present` e Discord `Absent`, sem aceitar novos termos nem instalar nenhum dos dois.

O pacote portátil inclui documentação e hashes SHA-256; o arquivo de código-fonte inclui Cargo.lock, testes, UI, módulos e scripts. Dependências de terceiros possuem inventário e textos de licença disponíveis em `TERCEIROS.md` e `licenses/`. Builds remotos e assinatura comercial não foram executados.


## Revisão de automação — 01/11/2026

A revisão atual preserva os testes anteriores e acrescenta atualização versionada, políticas Brave, energia, migração, fontes AMD e identidade de produtos.

- `cargo fmt --all -- --check`: aprovado, usando o wrapper isolado.
- `cargo clippy --locked --all-targets -- -D warnings`: aprovado.
- `cargo test --locked`: 72 aprovados (13 unitários, 19 automação e 40 integração), sem falhas. Nenhum teste instala ou aplica ajustes no host.
- Renderizador da interface real: 48 capturas com fixtures, em 1100×800, 800×600 e escala 2; páginas de Aplicativos/Otimizações inspecionadas visualmente.
- Consultas WinGet nativas export/show: executadas sem aceitar novos termos e sem instalar. Falta de correlação de Brave no exportador foi preservada como incerteza, não como ausência/versão presumida.
- Litecord v1.3.1 baixado apenas para inspeção: hash confere, Authenticode NotSigned; nenhum instalador executado.
- AMD Auto-Detect baixado apenas para inspeção a partir da página oficial: assinatura válida; verificador Rust confirmou editor Advanced Micro Devices. SHA local é registro, não hash publicado pelo fabricante.
- Script de empacotamento PowerShell analisado pelo parser: sem erros de sintaxe. Aceita um nome separado de arquivo fonte para preservar entregas anteriores.

Não houve instalação/atualização real de apps, aplicação de políticas, energia, drivers ou Atlas. Nenhuma VM/Sandbox foi utilizada; esses efeitos exigem homologação isolada. Veja AUTOMACAO.md para arquitetura, contratos oficiais e limites de rollback.

## Revisão 0.2.0 — 02/11/2026 (histórico)

Com a toolchain GNU isolada e LLD deste workspace:

- `cargo fmt --all -- --check`: aprovado após as alterações finais.
- `cargo clippy --locked --all-targets --jobs 2 -- -D warnings`: aprovado, sem avisos.
- `cargo build --release --locked --jobs 2`: aprovado. EXE final: 9.798.144 bytes (9,34 MiB). Imports PE: somente DLLs Windows.
- `cargo test --release --locked --jobs 2`: **94 aprovados**, 0 falhas/ignorados: 16 unitários, 19 automação, 19 handoff e 40 workflows. Repetidos após a correção visual final.
- `cargo test --locked`: também passou nos 94 em uma revisão intermediária 0.2.0. A repetição de depuração após os últimos ajustes foi interrompida por compilação prolongada, sem resultado de testes. Uma compilação do exemplo de inspeção também foi interrompida nessa situação. Não foram contabilizadas como aprovação; a validação final usa release e inclui os exemplos executados.
- `inspect_prepared` em release: Litecord v1.3.1 e AME 0.8.4 conferidos por SHA-256 fixado e cabeçalho PE básico. Ambos retornaram Unsigned no verificador Rust. Nenhum EXE inspecionado foi lançado.
- `render_ui` em release: **60 capturas**, 20 cenários em 1100×800, 800×600 e 950×700 lógico a 200% DPI. Confirmações Litecord/AME e pendência de reinício inspecionadas visualmente. O texto do botão principal desabilitado teve o contraste corrigido. Capturas são fixtures declaradas, sem operações no host; não equivalem a teste de cliques/instaladores reais.
- Release: `--help`, `--dry-run` e `--simulate` retornaram 0. O dry-run final confirmou schema 3 e cinco tarefas pendentes; a simulação declarou explicitamente ausência de alterações.
- Medição técnica do processo final com perfil vazio e sem aprovar operações: Windows 10 x64, 10 segundos após inventário e estabilização; Working Set **85,47 MiB**, Private Bytes **71,69 MiB**, 12 threads e incremento de CPU **0 ms** dentro da resolução da API. Uma amostra local não garante zero CPU ou o mesmo consumo em todos os PCs. Inventário foi sinalizado em aproximadamente 6 s; isso não mede tempo até o primeiro frame da UI.
- Fechamento normal da janela: exit code 0; nenhum processo filho próprio remanescente. Sem serviços/tarefas/autorun criados pelo aplicativo.

Os testes incluem fila/consentimento por digest e perfil, assinatura inválida versus ausente, ausência de prova após exit code zero, cancelamento/reboot, evidência Atlas antiga/parcial/insegura, migração v2, transferências interrompidas e limpeza restrita. Há executores simulados e um subprocesso fixture que somente imprime dados para testar buffers.

**Não homologado em VM:** instalações reais Litecord/AME/WinGet, UAC/SmartScreen, aplicação Atlas, resultados após reboot, drivers e aplicação/reversão real de políticas/ajustes. MSVC, ARM64 e workflow remoto também não foram executados. Não houve benchmarking de jogos/FPS. Contratos e fontes oficiais: INTEGRACAO-020.md; eficiência: EFICIENCIA.md.

## Revisão 0.3.0 — 02/11/2026 (histórico)

Toolchain GNU isolada/LLD do workspace, Windows 10 x64:

- `cargo fmt --all -- --check`: aprovado após as alterações finais.
- `cargo clippy --locked --all-targets --jobs 2 -- -D warnings`: aprovado sem avisos.
- `cargo test --release --locked --jobs 2`: **107 aprovados**, 0 falhas/ignorados: 17 unitários, 19 automação, 19 handoff, 12 wizard/AMD/legado/migração e 40 workflows. Repetidos após o ajuste visual final.
- `cargo build --release --locked --jobs 2`: aprovado. EXE final: **8.693.248 bytes (8,29 MiB)**. Imports PE inspecionados: somente DLLs Windows, sem runtime GNU externo adicional.
- `render_ui` em release: **88 capturas**, 22 cenários em 1100×800, 800×600, 620×500 e 950×700 lógico a 200% DPI. Inspecionadas visualmente oito combinações representativas: início, aplicativos, ajustes, revisão, Atlas legado/Windows incompatível, execução e Atlas a 200%. O alinhamento vertical foi corrigido para não distribuir espaços grandes entre textos em telas altas. Renderização com fixtures não equivale a teste de todos os cliques ou instaladores.
- Executável final: `--help`, `--dry-run examples/profile.json` e `--simulate` retornaram 0. Dry-run confirmou schema 4 e cinco tarefas Pending. A simulação declarou explicitamente SIMULAÇÃO, sem alterações no sistema.
- Amostra técnica do processo final com perfil vazio e sem aprovar operações: **82,57 MiB Working Set, 69,57 MiB Private Bytes, 12 threads e 0 ms de incremento de CPU em 10,003 segundos**, dentro da resolução da API. Inventário sinalizado em 628 ms nesta execução após outros diagnósticos; não é medição de cold start ou do primeiro frame, nem garantia universal de consumo/CPU zero.
- Fechamento normal da janela: exit code 0; zero processos filhos próprios restantes. Nenhum serviço, autorun ou tarefa agendada própria foi criado.
- O APBX 0.4.1 indicado pelo usuário correspondeu por SHA-256 ao novo download oficial. Nenhum playbook foi aberto/aplicado; a API da release antiga não publica digest. O hash fixado foi calculado durante a revisão.

Os testes novos cobrem correspondência exata de GPU e fabricante, classe Display, ofertas ambíguas/duplicadas, múltiplas GPUs, falta de oferta, UAC cancelado, reboot, falha, exit code zero sem prova, pedido do helper adulterado antes de chamar WUA, gates Atlas 10, migração preservando backups/concluídos e aprovação final que altera somente termos. A permissão Litecord é restrita à execução aprovada e não nasce ao carregar um perfil.

Um teste de navegação inicialmente esperava a antiga página 4; foi corrigido para a normalização intencional na etapa 3, preservando perfil. A primeira checagem de formatação final encontrou uma linha em branco excedente no exemplo; corrigida e repetida com sucesso. Essas tentativas não são contabilizadas como aprovação. Recompilações de depuração anteriores interrompidas por demora também não são resultados de teste.

**Não executado no host nem homologado em VM:** Litecord real, WUA search/download/install, elevação real do helper, UAC/SmartScreen, driver funcional após reboot, AME/Atlas, instalação/upgrade WinGet e aplicação/reversão de ajustes/políticas. AMD confirma pacote WUA instalado; saúde e comportamento do driver em uso requerem homologação no hardware. MSVC, ARM64, acessibilidade completa e workflow remoto não foram testados. Nenhuma modificação profunda, instalação ou reinicialização foi feita no desenvolvimento. Não há benchmarking de jogos/FPS ou alegação de 100% de desempenho.

Contrato atual e fontes: INTEGRACAO-030.md. README contém comandos exatos e limites dos recursos automáticos/assistidos.

## Correção 0.3.1 — 02/11/2026 (histórico)

O bug de seleção foi reproduzido antes da alteração, com eventos de ponteiro do Slint: o segundo controle ficava selecionado junto do primeiro porque a escrita em `Choice.checked` removia sua binding com o modelo. O teste falhou com “seleção anterior ficou presa no controle”. A correção torna `checked` entrada controlada pelo modelo; o callback informa o novo valor ao dono do estado. Todos os consumidores, incluindo consentimentos, foram atualizados. Clicar novamente no navegador selecionado retira a seleção.

Atlas é sugerido somente para perfil novo sem fila recuperada, após o primeiro inventário confirmar compatibilidade moderna ou legada. Não é autorização de aplicação. A recusa salva é respeitada nas próximas aberturas e atualizações do inventário. No Windows 10, o aviso de legado/não recomendado permanece. Uma seleção incompatível recuperada também pode ser recusada.

Verificações executadas na toolchain Windows GNU/LLD isolada:

- `cargo fmt --all -- --check` e `cargo clippy --locked --all-targets --jobs 2 -- -D warnings`: aprovados após o último ajuste.
- `cargo test --release --locked --jobs 2`: **110 testes aprovados**, zero falhas/ignorados: 17 unitários, 19 automação, 19 handoff, 1 UI, 14 wizard/AMD/legado e 40 workflows. O teste de UI foi repetido com sucesso após o último ajuste da possibilidade de recusar seleção incompatível.
- `tests/ui_choices.rs`: eventos de clique no componente compartilhado e na interface MainWindow real, sem iniciar o executor. Verifica desmarcar, trocar navegador, atualização por modelo, controles desabilitados, retirar Atlas e recusar seleção incompatível. Confere também pixels renderizados para detectar quadrados visuais presos, além do estado.
- Novos testes de sugestão Atlas cobrem Windows 11, Windows 10 legado, edições/builds/arquiteturas incompatíveis e ausência de consentimento automático. Recusa é gravada/recarregada em diretório temporário.
- `cargo build --release --locked --jobs 2`: aprovado. `--help`, `--dry-run examples/profile.json` e `--simulate`: exit code 0; schema 4, cinco tarefas Pending no perfil de exemplo, simulação explícita.
- Smoke da janela Windows com diretório de dados novo isolado: sugestão Atlas confere com inventário real, fechamento normal; após gravar recusa nesse perfil de teste, reabertura preserva a escolha e os termos continuam não aceitos. Nenhuma fila de execução foi criada. Apenas inventário e arquivos próprios de teste; sem instalação, ajustes, Atlas ou reboot.

Não foram repetidas as 88 capturas nem a medição de consumo da revisão anterior. A regressão atual usa a UI real com cliques injetados e renderização em software; não cobre toda interação por teclado/leitor de tela ou todos os DPIs. Nenhum instalador foi executado e a homologação de instalações em VM continua pendente, conforme os limites anteriores. Não foram adicionadas dependências, serviços ou monitoramento.

## Revisão 0.3.2 — 02/11/2026 (histórico)

Fluxo reduzido a PC → Atlas → aplicativos → revisão. A etapa “Os últimos detalhes” e os seletores de driver/ajustes foram removidos. Ao preparar um novo plano em Windows suportado, são incluídos driver AMD quando seu PCI vendor for detectado, Google no Brave e extensões de arquivo visíveis. O perfil e os modelos da UI são sincronizados antes de salvar o fingerprint; a execução usa a mesma seleção que foi apresentada. A aprovação final continua obrigatória e Atlas permanece uma barreira assistida real.

Não são incluídos por suposição plano Equilibrado, limpeza ampla, taxa de atualização, Modo de Jogo ou controle de inicialização. Preferências antigas explícitas permanecem no perfil; suporte oficial, ferramentas manuais e reversões ficam nos detalhes após entrar na execução. Não há serviço ou monitoramento em segundo plano permanente.

Verificações executadas em Windows GNU/LLD isolado:

- `cargo fmt --all -- --check`: aprovado; `cargo clippy --locked --all-targets --jobs 2 -- -D warnings`: aprovado após os últimos ajustes.
- `cargo test --release --locked --jobs 2`: **114 testes aprovados**, zero falhas/ignorados: 18 unitários, 19 automação, 19 handoff, 1 UI, 17 wizard/AMD/legado e 40 workflows.
- Testes novos: plano automático condicionado ao hardware/Windows, deduplicação/idempotência, navegador ausente, recusa Atlas preservada, ausência de aprovação implícita, manutenção de escolhas manuais antigas, retomada de concluídos, e execução de drivers/browser/extensões uma única vez por executor simulado após aprovação única. Um teste da UI verifica que o perfil de revisão e o lido pela execução têm o mesmo conteúdo/fingerprint, inclusive com plano Equilibrado selecionado em perfil antigo.
- Build release aprovado; EXE: **8.600.576 bytes**. `--help`, `--dry-run examples/profile.json` e `--simulate`: exit code 0, plano de exemplo schema 4 com cinco tarefas Pending e simulação explícita. O perfil de exemplo é explícito; os itens automáticos são acrescentados pela preparação do novo plano na UI.
- Smoke da janela real com diretório próprio de teste: primeira abertura e recusa Atlas preservada na reabertura; nenhum termo aprovado ou fila criada. Sem operações reais de instalação/ajuste.
- Renderizador final: **88 capturas**, quatro tamanhos/escalas. Inspecionadas quatro combinações representativas: aplicativos em 620×500, revisão com e sem Atlas em 620×500 e revisão Atlas em 800×600. Os itens automáticos e avisos essenciais cabem sem sobreposição; listas longas continuam com scroll. A fixture da revisão inclui os itens automáticos, sem executar o host.

Homologação real de WinGet/Litecord, WUA/UAC/driver, políticas/ajustes, AME/Atlas e reboot continua pendente em VM autorizada. Não houve nova medição de RAM/CPU, ensaio de todo teclado/acessibilidade ou benchmarking de jogos/FPS. Nenhuma nova dependência foi adicionada. A ausência de pacote AMD compatível permanece pendência; NVIDIA/Intel e integrações sem automação revisada continuam assistidas. A documentação de integração detalha esses limites.

## Revisão 0.3.3 — 02/11/2026 (estado atual)

Atlas selecionado delega o navegador ao assistente oficial. A UI oculta seletores e ofertas de atualização de navegador, resumo/contagem excluem a instalação própria, e o plano omite instalação, upgrade e políticas do navegador. O campo de preferência é preservado para voltar ao modo sem Atlas; não é uma confirmação de instalação pelo Atlas. Schema 4 mantido, sem dependências ou processos novos.

Verificações executadas com a toolchain Windows GNU/LLD isolada:

- `cargo fmt --all -- --check` e `cargo clippy --locked --all-targets --jobs 2 -- -D warnings`: aprovados. Um erro de pattern guard no novo teste foi corrigido antes da execução aprovada; essa tentativa não é contada como sucesso.
- `cargo test --release --locked --jobs 2`: **118 aprovados**, zero falhas/ignorados: 19 unitários, 19 automação, 19 handoff, 1 UI, 17 wizard/AMD/legado e 43 workflows.
- Novos testes cobrem os quatro navegadores, Atlas moderno e legado, supressão de instalação/configuração/upgrade próprios, preservação de atualização de outros apps, retorno da preferência após recusar Atlas e rejeição de fila antiga com navegador indevido. O teste de revisão/UI confirma que atualizações de navegador não entram no perfil de execução com Atlas, e que aprovação/fingerprint permanecem válidos.
- Regressão de cliques/renderização: navegador desaparece ao selecionar Atlas e reaparece com a escolha anterior ao recusar; seleção/desmarcação anterior continua aprovada. A apresentação deixa de contar Brave como app próprio no modo Atlas.
- `cargo build --release --locked --jobs 2`: aprovado. EXE **8.670.208 bytes**. Help, dry-run e simulate retornaram 0; perfil explícito de exemplo permanece schema 4 com cinco tarefas Pending, simulação identificada.
- Smoke da janela real em diretório próprio: primeira abertura e recusa Atlas preservada na reabertura, sem termos aprovados nem fila de execução criada. Sem instalações/ajustes reais.
- Renderizador final: **92 capturas**, 23 cenários em quatro tamanhos/escalas. Inspecionadas quatro combinações: aplicativos com Atlas e sem Atlas em 620×500, revisão Atlas em 620×500 e aplicativos Atlas a 200% DPI. Sem seleção de navegador no modo Atlas; scroll e ação principal acessíveis.

Aplicação do Atlas e instalação efetiva de seu navegador não foram executadas. Homologação de instaladores/driver/UAC/reboot em VM continua pendente; não houve nova medição de RAM/CPU, teste completo de acessibilidade ou benchmarking de jogos/FPS. Nenhuma proteção, playbook ou requisito oficial foi alterado por esta revisão.
