# Fronteiras de segurança

- Processo principal sem elevação. Somente instaladores oficiais solicitam UAC quando necessário. O modo AMD usa `runas` somente para o próprio helper WUA temporário; UAC normal permanece visível. Não há contorno de UAC nem ocultação de prompts.
- Perfis validados por versão, IDs de catálogo e ajustes enumerados. Campos desconhecidos são rejeitados. A aprovação corresponde ao hash do perfil: modificar seleção invalida o plano.
- Apenas um processo por diretório de estado. Escrita em arquivo temporário no mesmo diretório, flush e substituição atômica. Dados corrompidos/versões futuras causam erro, sem reset silencioso.
- WinGet resolvido por identidade registrada `Microsoft.DesktopAppInstaller_8wekyb3d8bbwe`, via PackageManager, em vez de PATH ou diretório atual. Authenticode é consultado em cada processo. Falhas de assinatura/revogação impedem a execução.
- Conteúdo de GitHub só pode ser aberto nos adaptadores revisados Litecord/AME, após confirmação específica. TLS nativo valida certificados; URLs HTTPS e redirects são limitados a hosts conhecidos. O arquivo final só é publicado depois de conferir SHA-256. Limite de tamanho de 1 GiB e metadados de 4 MiB. Digest ausente não é ignorado.
- URLs do catálogo/ajustes são constantes, conferidas antes de ShellExecute. Não há comandos de shell construídos com entradas do usuário, nem busca de EXE no PATH.
- Logs locais contêm nomes de pacotes, estados e saídas limitadas do WinGet. O aplicativo não solicita senhas/tokens. Não há coleta remota de inventário.
- Cancelar não mata instaladores, que poderiam estar numa transação MSI. Downloads/consultas podem ser interrompidos; consultas têm timeout e instalações interativas não são terminadas à força. Fechar durante execução solicita cancelamento e aguarda a operação atual. Na intervenção externa AME, é permitido fechar o Ruhtweaks preservando checkpoints; a ferramenta oficial já aberta pode continuar independentemente.
- Retomada é manual, sem tarefa agendada/Run/serviço. Um trabalho em execução durante falha vira pendência de revalidação. Operações concluídas do mesmo perfil são preservadas.
- Backup do ajuste HKCU antes da escrita e releitura após aplicar/reverter. Reversão recusa conflito com modificação externa. Limpeza do Windows é assistida e não tem promessa de rollback.
- Atualizações WinGet fixam origem e versão no perfil aprovado, verificam a versão anterior e posterior e recusam origem/versão ambígua. Não há atualização global, downgrade presumido ou inclusão de versões desconhecidas.
- Políticas de pesquisa do Brave são restritas a três valores HKCU, com backup anterior, releitura, proteção contra políticas da máquina e reversão com checagem de conflito. Shields e arquivos pessoais não são escritos. Energia usa as APIs documentadas, preservando o plano original.
- AMD Auto-Detect: somente link reconhecido publicado pela AMD, HTTPS/redirects restritos ao fabricante e assinatura/editor AMD válidos. SHA-256 calculado é registrado e não apresentado como hash publicado. EXEs preparados usam Attachment Services do Windows; bloqueios não são ignorados.
- Litecord/AME: apenas release, nome e hash revisados. No Litecord, a aprovação final do plano concede abertura apenas ao arquivo/hash revisado, em memória; AME mantém confirmação específica. Assinatura ausente é informada; assinatura inválida/inconclusiva bloqueia. ShellExecuteEx usa abertura normal, sem bypass de zona, sem ocultar UAC e sem alterar proteções. O arquivo é revalidado e mantido aberto sem compartilhamento de escrita/exclusão durante o lançamento. Veja INTEGRACAO-020.md.
- Preparação e abertura são estados separados. Exit code zero não comprova instalação. Limpeza exige conclusão persistida, propriedade do caminho e hash intacto; arquivos pendentes/reboot são preservados. O journal não contém autorização de lançamento reutilizável.

## Limites de confiança

Hashes e assinaturas validam publicação/integridade, não garantem ausência de falhas nos softwares oficiais. O WinGet utiliza seu catálogo e mecanismos próprios. Termos, software residente e opções do instalador precisam de revisão do usuário. Um administrador ou outro processo sob o mesmo usuário pode alterar dados locais; não existe isolamento contra comprometimento prévio da conta/OS. ACLs herdadas de LocalAppData são usadas; não são relaxadas.

O app não modifica configurações de proteção, componentes de sistema, serviços essenciais, firmware ou políticas de Windows Update. Não se afirma que driver foi atualizado, navegador configurado, extensão instalada ou Atlas aplicado com base em abertura de uma página.

Contrato atual AMD, aprovação Litecord e migração: INTEGRACAO-030.md. O helper elevado não escreve em diretórios de usuário; download/instalação usam WUA para os IDs Display previamente selecionados e confirmados novamente contra o hardware.

