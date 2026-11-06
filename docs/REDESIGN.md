# Redesign da interface — Ruhtweaks

Registro da revisão visual 0.1.0. A versão 0.2.0 acrescenta confirmação Litecord/AME, novos estados e despacho por eventos; veja INTEGRACAO-020.md, EFICIENCIA.md e a seção mais recente de VALIDACAO.md. A descrição abaixo do bloqueio de lançamento Atlas corresponde à versão anterior.

## Alterações

- `ui/app.slint`: telas, navegação lateral, assistente, revisão e progresso dedicados. O rodapé passou a ter uma ação principal contextual e, no assistente, a ação de voltar.
- `ui/theme.slint`: tokens de cores/espaçamento/raio, botões e checkboxes com foco de teclado e propriedades de acessibilidade, badges e detalhes expansíveis.
- `ui/components.slint`: cards reutilizados para hardware, apps, navegadores, ajustes e tarefas.
- `ui/icons/*.svg`: seis ícones vetoriais locais, sem downloads ou dependências.
- `src/app_ui.rs`: ligações com os callbacks existentes, seleção, telas de resultado e consulta do estado atual do ajuste reversível.
- `src/app_ui/navigation.rs`: posição do assistente, ida/volta e ação contextual. Usa `ui-preferences.json` versionado e gravação atômica já existente.
- `src/app_ui/presentation.rs`: apresentação dos dados existentes, categorias, resumo da seleção, GPUs e estados. Mantém o índice original dos apps e ajustes ao criar os cards.
- `examples/render_ui.rs`: renderização determinística do componente real Slint, com dados de teste e sem executar instaladores ou ajustes.
- `README.md`: instruções e arquitetura atualizadas.
- `scripts/package.ps1`: aceita um nome de pacote para gerar a entrega em outra pasta quando uma versão anterior estiver aberta; o nome é limitado a um componente de caminho.

## Comportamento preservado

Os nove módulos de domínio (`atlas`, `browser`, `catalog`, `drivers`, `installer`, `security`, `state`, `system_info` e `tweaks`) foram comparados por SHA-256 com a versão anterior e permanecem **idênticos byte a byte**. Cargo.toml, Cargo.lock, perfil, journal e esquema de domínio não foram alterados. Nenhuma dependência foi adicionada.

O arquivo novo de navegação não contém aprovação, permissões administrativas nem comandos. Abrir ou retomar uma tela nunca inicia instalações. Uma alteração de seleção continua invalidando o plano e a aprovação. O botão de execução continua passando pelos mesmos gates e pela confirmação explícita do plano. Repetir falhas exige revisão e nova aprovação; revalidar usa a detecção existente.

A seleção sugerida continua Brave, Litecord, Steam, Epic e Riot. Discord continua opcional. A apresentação por categorias não troca IDs nem fontes. Cada GPU abre somente a opção oficial correspondente; a UI diferencia modo assistido e fabricante indisponível, sem anunciar atualização que não tenha sido verificada.

AtlasOS mantém a incompatibilidade de SO/edição/arquitetura e o bloqueio da aplicação por requisitos de segurança. A escolha disponível, quando compatível, é somente o download revisado. As etapas do roteiro são orientações, não uma barra fictícia de instalação. Não há botões que executem AME, reiniciem ou modifiquem proteções.

Os ajustes assistidos continuam abrindo as mesmas configurações oficiais. O ajuste de extensões mantém backup, verificação e reversão. Limpeza sem rollback é identificada na descrição curta. A UI não oculta avisos essenciais em detalhes técnicos.

## Verificações

Antes da alteração: 42 testes existentes aprovados. Após a alteração: **52 testes aprovados**, sendo 12 unitários e 40 de integração. Os 10 casos novos verificam navegação, volta/retomada, persistência separada do perfil, bloqueio durante carregamento/operação, categorias/índices, seleção sugerida, estado vazio e honestidade dos estados de conclusão. Não instalam apps nem alteram configurações do host.

Comandos de validação:

```powershell
.\scripts\dev.ps1 -CargoArgs @('fmt','--all','--','--check')
.\scripts\dev.ps1 -CargoArgs @('clippy','--locked','--all-targets','--','-D','warnings')
.\scripts\dev.ps1 test --locked
.\scripts\dev.ps1 build --release --locked
.\scripts\dev.ps1 run --locked --example render_ui
```

O exemplo de renderização usa o mesmo MainWindow e componentes compilados do aplicativo. Gera 48 capturas: 16 telas/estados em 1100×800, 800×600 e 950×700 pontos lógicos a 200% de escala. Inclui navegação normal, carregamento, seleção vazia, erro, menu, sucesso, execução e GPU sem fabricante validado. Os dados dessas imagens são fixtures, não resultados de instalação.

As imagens foram usadas para inspecionar hierarquia, cores, espaçamento, largura disponível, scroll e controles persistentes. Durante a inspeção foram corrigidos badges largos, alinhamento dos ícones e o encolhimento da área principal quando os controles tinham largura máxima. A interface pode rolar em janelas menores; o botão contextual fica fora do scroll. Dimensão mínima: 760×500 pontos lógicos.

Não foram executados instaladores, ajustes, reinicializações ou mudanças de proteção no computador. As capturas não substituem uma homologação de ponta a ponta de instaladores/UAC ou testes com leitor de tela e diferentes monitores físicos. A validação visual desta entrega é feita pelo renderer software do Slint; não depende do helper externo de captura de janelas.
