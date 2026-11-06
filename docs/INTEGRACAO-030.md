# Integração 0.3.0 — assistente simples

## Interface e consentimento

Na 0.3.3, selecionar Atlas delega o navegador ao assistente oficial: seletores e ofertas de atualização de navegador ficam ocultos, e o plano exclui instalação, upgrade e políticas de pesquisa do navegador. Preferências antigas ficam salvas para voltar ao fluxo sem Atlas. Isso vale para os dois adaptadores revisados. Uma fila antiga que ainda inclua navegador precisa de nova revisão, sem executar a operação retirada; nenhum navegador é considerado instalado por essa delegação.

Na revisão 0.3.1, Atlas é sugerido no primeiro inventário de um perfil novo, somente quando o adaptador moderno ou legado reconhecer o sistema. A opção legada mantém o aviso de não recomendado. Perfis existentes, recusas salvas e filas recuperadas não recebem essa sugestão novamente. Desmarcar Atlas ou selecionar Manter meu Windows remove a etapa; não concede consentimento de execução. Os controles de seleção recebem o estado do modelo e emitem o valor solicitado, sem sobrescrever suas próprias bindings.

Na 0.3.2, quatro etapas: PC, Atlas opcional, apps e revisão. A etapa de escolher drivers/ajustes foi removida. O plano inclui driver AMD quando o hardware for reconhecido, Google no Brave e extensões de arquivo visíveis. Somente a aprovação final inicia operações; perfis carregados não são executados automaticamente. Energia, inicialização, monitor, jogos e limpeza ampla não ganham automação presumida. Suporte, ferramentas manuais e reversões permanecem acessíveis nos detalhes após iniciar. Não existe barra lateral nem grade de aplicativos; a execução mostra estado real, sem porcentagem fictícia. Fontes e avisos essenciais continuam acessíveis antes da aprovação.

**Iniciar e aceitar termos** é a aprovação explícita da seleção revisada. `approve_reviewed` valida fingerprint, IDs e ações do plano e altera somente o consentimento aos termos. Seleção diferente exige revisão. A permissão automática Litecord só existe no executor daquela execução; carregar um perfil não a concede.

## Litecord

Mantidos a origem Ak4ai/Litecord, v1.3.1, asset e SHA-256 revisados na 0.2.0. A ausência de Authenticode é informada na revisão. Nova assinatura inválida/inconclusiva bloqueia; alteração da release/hash exige revisão do adaptador.

Após a aprovação única, o instalador usa `/SILENT /NORESTART /RESTARTEXITCODE=3010 /NOCLOSEAPPLICATIONS /NOFORCECLOSEAPPLICATIONS /NORESTARTAPPLICATIONS /MERGETASKS=!startupicon`. Parâmetros [oficiais Inno](https://jrsoftware.org/ishelp/topic_setupcmdline.htm); [script do pacote](https://github.com/Ak4ai/Litecord/blob/v1.3.1/installer.iss). O próprio pacote pode abrir o cliente ao terminar. Nenhum prompt de proteção é contornado. AppId, origem declarada e executável registrado são verificados após o processo; código zero não basta.

## AMD automático

Usa as APIs oficiais [Windows Update Agent](https://learn.microsoft.com/en-us/windows/win32/wua_sdk/searching--downloading--and-installing-updates), sem scripts ou flags presumidas do bootstrapper AMD. A [busca](https://learn.microsoft.com/en-us/windows/win32/api/wuapi/nf-wuapi-iupdatesearcher-search) restringe a drivers não instalados/não ocultos, atribuídos para atualização automática e não opcionais. Políticas/origem de atualização configuradas no Windows são respeitadas; não há troca de servidor, alteração de serviço ou instalação global de updates do SO.

O adaptador usa [IWindowsDriverUpdate](https://learn.microsoft.com/en-us/windows/win32/wua_sdk/iwindowsdriverupdate-properties): classe Display, fabricante AMD e correspondência exata de HardwareID/CompatibleID consultados via PnP. Mais de um candidato diferente para o mesmo dispositivo causa pendência. Versão numérica maior não é critério. Drivers com exigência de interface são excluídos.

Antes de elevar, o processo normal persiste uma solicitação com hash do perfil/binário, IDs/revisões selecionados, boot e validade de dez minutos. O helper é o mesmo executável, aberto com `runas` e UAC normal. Confere hash do pedido, fila canônica, seleção/termos, processo autorizador vivo e mesma imagem, token elevado, hardware atual e oferta WUA. **Não grava arquivos em caminhos de usuário como administrador.** Sua única alteração privilegiada é instalar os pacotes Display selecionados por WUA. O processo normal grava o estado e lê o resultado do processo.

Download/instalação são sequenciais. `IUpdateInstaller2.ForceQuiet` e `AllowSourcePrompts=false` controlam a UI do instalador WUA, sem remover UAC ou assinatura. A aprovação inclui os termos dos drivers selecionados; EULA é aceita pela API documentada. Resultado global e de cada pacote precisam indicar sucesso. Depois, o processo normal consulta IDs/revisões instalados. Reinício solicitado e confirmação ausente permanecem pendentes. O reboot não é executado pelo Ruhtweaks.

Não é uma garantia de obter qualquer versão Adrenalin. Se Windows Update não oferecer pacote recomendado compatível, o app informa a pendência e mantém a ferramenta oficial AMD como alternativa assistida. GPUs de outros fabricantes não herdam sucesso de uma GPU AMD. Componentes internos do pacote são administrados pelo Windows; o Ruhtweaks não solicita Adrenalin/chipset nem BIOS/firmware. Rollback de driver depende das opções mantidas pelo Windows/OEM, sem promessa de reversão integral.

## Atlas Windows 10 legado

Oferecido apenas em Windows 10 22H2 x64 build 19045 e edições revisadas. [Configuração oficial 0.4.1](https://github.com/Atlas-OS/Atlas/blob/0.4.1/src/playbook/playbook.conf), [release](https://github.com/Atlas-OS/Atlas/releases/tag/0.4.1). O arquivo do usuário foi comparado byte a byte por hash com novo download oficial, sem abrir/aplicar o playbook:

`sha256:e2fda94d1b340b2847cc8615f2474fc6952b206953920d32ef42e44afeb88680`

O asset antigo não publica digest na API; esse hash foi calculado na revisão e fixado, não apresentado como assinatura/hash publicado. Origem HTTPS oficial e hash são exigidos. Versões/builds diferentes não são autorizadas por nome de arquivo.

A opção é **legada, não recomendada**. Somente preparação verificada; aplicação assistida e sem confirmação automática do resultado 0.4.1. O marcador 0.5.0 não conclui o fluxo legado. A configuração antiga exige FreshInstall, DefenderToggled e NoAntivirus; preservamos os bloqueios de proteção. Não há modificação de APBX, formatação, desativação de segurança ou aplicação silenciosa. O fluxo moderno mantém suas verificações da 0.2.0.

## Migração e eficiência

Schema 4 remove a extensão de navegador do modelo ativo e acrescenta escolha Atlas legado, falsa por padrão. Leitores v1–v3 ficam restritos à migração com backup. Fingerprints antigos são reconstruídos no formato original; somente tarefas com identidade/ação correspondentes preservam estados. A operação retirada fica no backup e não é executada. Não se concede autorização de abertura ao migrar.

UI por eventos e modelos de lista única, sem pares/colunas clonados. Sem timer ocioso, novas dependências, serviço ou agente próprio. Helper WUA apenas durante operação selecionada; toda thread encerra com o processo. WUA pode consumir memória/rede do Windows enquanto busca/instala; não há monitoramento periódico ou mudanças agressivas para reduzir esse consumo.

## Homologação

Mecanismos de instalação são reais, mas durante desenvolvimento os testes usam backends simulados. Não executar o helper AMD, WUA Install, Litecord ou AME no host sem aprovação. Validar em VM com snapshot: UAC cancelado, pacote compatível/inexistente, política WSUS, falha parcial, reboot e revalidação; depois validar Litecord e Atlas assistido. Não há promessa de desempenho universal ou testes de FPS.
