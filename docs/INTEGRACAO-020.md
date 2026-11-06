# AtlasOS e Litecord — revisão de 02/11/2026

Registro histórico da 0.2.0; o contrato atual está em INTEGRACAO-030.md. A 0.3.0 usa aprovação única para Litecord e AMD automático via WUA.

Esta revisão substitui o bloqueio local genérico por preparação, confirmação por arquivo e verificação de resultado. Não elimina requisitos das ferramentas oficiais nem autoriza desativar proteções.

## Arquivos revisados

| Componente | Versão / artefato | SHA-256 publicado e fixado |
|---|---|---|
| Litecord | Ak4ai/Litecord v1.3.1 — Litecord-Setup-x64.exe | `3628b3610c933c6bfdfe766d9882f2c8d7747f3f3e89e2078a34d978130ec012` |
| AME | Ameliorated-LLC/trusted-uninstaller-cli 0.8.4 — AME-Beta-v0.8.4.exe | `a637f32b64e1493b315db9139065105656c84518dca20ce3b18c89750ee40c82` |
| Atlas | 0.5.0-hotfix — AtlasPlaybook_v0.5.0-hotfix.apbx | `13c4d8e7fc0ed38c37c5942ebcabf7f636389972e6de391a192ae649967602d8` |

Releases são consultadas pela API GitHub dos repositórios oficiais. Tag, nome e digest devem corresponder à revisão; uma nova versão não herda parâmetros ou propriedades da anterior. Arquivos preparados são reutilizados somente depois de validação. Download EXE passa pelo Attachment Services do Windows; se ele bloquear/remover o arquivo, não há exclusão de antivírus nem contorno.

Os dois EXEs consultados não possuem assinatura Authenticode. Isso não equivale a classificá-los como maliciosos. O hash verifica correspondência com a publicação e a revisão; não autentica o editor como uma assinatura digital. A UI distingue assinatura válida, ausente e inconclusiva/inválida. Somente ausência pode prosseguir mediante confirmação específica; falha de confiança não é convertida em ausência.

## Litecord

O [installer.iss da v1.3.1](https://github.com/Ak4ai/Litecord/blob/v1.3.1/installer.iss) confirma Inno Setup, editor declarado Litecord Project, AppId `{6E4F5D7B-9A3C-4D2E-8B1F-5C7D9A2B4E6F}`, privilégio mínimo e tarefas opcionais desmarcadas. O fechamento original é `CloseApplications=force` com filtro **litecord.exe**; não é um comando para encerrar indiscriminadamente outros programas.

O fluxo baixa, confere hash/formato/assinatura e pede confirmação vinculada ao perfil e ao digest. Essa permissão só existe em memória durante a sessão. A abertura passa pela API normal do Windows, sem `runas`, flags de bypass ou remoção de MOTW. Windows pode exibir ou bloquear avisos conforme suas políticas.

Parâmetros usados são [documentados pelo Inno](https://jrsoftware.org/ishelp/topic_setupcmdline.htm): `/NORESTART`, `/RESTARTEXITCODE=3010`, `/NOCLOSEAPPLICATIONS`, `/NOFORCECLOSEAPPLICATIONS`, `/NORESTARTAPPLICATIONS` e `/MERGETASKS=!startupicon`. O modo silencioso acrescenta `/SILENT` somente se escolhido na confirmação. Não usamos `/SUPPRESSMSGBOXES`, `/NOCANCEL` nem fechamento forçado.

O `[Run]` do pacote pode abrir Litecord ao finalizar, também em modo silencioso; não existe parâmetro oficial validado para retirar essa entrada do instalador sem modificá-lo. O aviso permanece visível. A inicialização com Windows começa desmarcada; uma escolha interativa posterior é do usuário. Nenhum arquivo interno de perfil é editado.

O processo é acompanhado por handle. Resultado zero exige nova detecção do AppId, nome/editor/URL declarados e executável PE registrado. Cancelamento, falha, reboot e ausência de prova têm estados distintos. Uma instalação existente confirmada não é executada novamente. Registro com executável ausente/inválido é pendência, não ausência presumida.

## AtlasOS / AME

A cobertura do adaptador continua sendo Windows 11 x64, builds 26100/26200, Pro/N, Enterprise/N e Education/N. Isso descreve o adaptador revisado, não uma alegação de que o Atlas oficial nunca possa oferecer outra arquitetura/versão. Home, versões não revisadas e novas releases permanecem bloqueadas para evitar incompatibilidade.

A preparação verifica identidade do Windows, espaço de trabalho para os downloads, disponibilidade das fontes pela própria conexão HTTPS e reinício pendente informado pelo Windows Update Agent. Antes de abrir AME, verifica alimentação AC ou ausência de bateria. O limite de 512 MiB é da área própria de preparação, não é requisito publicado para reinstalar Windows nem garantia de espaço para todos os drivers/apps.

Ausência de reboot pendente não comprova ausência de todas as atualizações oferecidas. A checagem final de requisitos do playbook permanece no AME; não iniciamos buscas/instalações pesadas de Windows Update nem modificamos serviços para fazer uma verificação passar.

O [playbook revisado](https://github.com/Atlas-OS/Atlas/blob/0.5.0-hotfix/src/playbook/playbook.conf) exige **DefenderToggled, NoAntivirus e UCPDDisabled durante a preparação**. A opção `defender-enable` trata o resultado posterior e não elimina esses requisitos. O Ruhtweaks não os satisfaz desativando controles. Se a ferramenta exigir uma mudança incompatível com as regras do usuário, o fluxo deve ser interrompido. Backup e instalação limpa continuam decisões e procedimentos manuais.

O AME 0.8.4 é preparado e pode ser aberto após confirmação de origem/avisos, backup e leitura do guia. Abre **sem argumentos**. O usuário seleciona o APBX preparado e as opções na UI oficial; nada é injetado no playbook. São orientados `defender-enable`, `mitigations-default`, `auto-updates-default`, sem `disable-core-isolation`; o padrão oficial de atualizações automáticas não é adotado cegamente. Brave pode ser escolhido como `browser-brave`, e depois é detectado pelo WinGet para evitar redundância.

O repositório oficial possui um CLI e comandos internos/IPC. A revisão de [CommandLine.cs](https://github.com/Ameliorated-LLC/trusted-uninstaller-cli/blob/0.8.4/TrustedUninstaller.CLI/CommandLine.cs) não forneceu um contrato público validado de aplicação completa do playbook que preserve estes controles. Não tratamos a existência de um CLI de execução interna como autorização para fabricar parâmetros de instalação ou ignorar o wizard.

## Verificação e continuidade

O core oficial [grava o resultado](https://github.com/Ameliorated-LLC/trusted-uninstaller-cli/blob/0.8.4/TrustedUninstaller.Shared/AmeliorationUtil.cs#L1159) em:

```text
HKLM\SOFTWARE\AME\Playbooks\Applied\{00000000-0000-4000-6174-6C6173203A33}
Name, Version, ErrorLevel, SelectedOptions, AppliedTimeUTC
```

O Ruhtweaks registra o marcador anterior e o boot antes da tentativa de abertura. Para concluir, exige resultado novo/posterior, AtlasOS 0.5.0, ErrorLevel zero, opções seguras, boot posterior, nenhum reboot pendente e confirmação de Defender em tempo real, firewall dos perfis ativos e UCPD ativo. As leituras usam WMI/COM/registro; nenhuma configuração é corrigida à força. Permissões insuficientes, provedor ausente, opções inseguras, erro parcial ou resultado antigo mantêm a etapa pendente.

Esse registro é evidência do resultado reportado pelo AME, **não uma atestação criptográfica**. Não distingue a variante hotfix nem identifica por hash o pacote efetivamente arrastado pelo usuário. O APBX preparado pelo Ruhtweaks permanece fixado e validado; o relatório delimita o que foi e não foi comprovado. Fechar o AME ou receber exit code zero nunca basta.

Reiniciar é uma escolha externa/autorizada; o app não chama reboot nem se cadastra no logon. Reabra o Ruhtweaks e use a verificação explícita. Uma nova sessão exige nova aprovação antes de qualquer abertura. Se o Atlas continuar pendente, **Continuar com apps sem concluir Atlas** cria outra revisão, preserva a pendência no histórico e exige aprovação; não altera o Atlas para “concluído”.

## Estado e homologação

Schema 3 acrescenta estados Preparing, Executing, RequiresConfirmation, ExternalAction, AwaitingReboot e AwaitingVerification. Perfis/journals v1/v2 são preservados em backup e migrados sem ampliar termos, seleção ou consentimento. A navegação Slint continua no sidecar independente. Não há permissão de abrir EXE salva em disco. Para usar o executável antigo, restaure seus backups em um diretório de dados separado, com o app fechado.

Os testes de lançamento/instalação usam backends simulados. A inspeção real de arquivos é somente leitura. Execução de Litecord/AME, comportamento de SmartScreen/UAC, aplicação Atlas, reboot e validação de resultado real ainda precisam de homologação em Windows isolado com snapshot. Nenhum instalador real foi executado no host de desenvolvimento.

