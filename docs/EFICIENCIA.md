# Eficiência e ciclo de vida — Ruhtweaks 0.3.0

O aplicativo prepara o computador e encerra. Os ajustes persistem pelo próprio Windows; não existe agente, serviço, tarefa agendada, servidor local, WebView ou componente próprio que precise permanecer executando.

## Interface e trabalho ocioso

- O timer de 150 ms que consultava um canal permanentemente foi removido. Workers publicam notificações pelo event loop nativo do Slint.
- Uma caixa de entrada agrega notificações e substitui progresso consecutivo por seu valor mais recente. Conclusão, erro e confirmação são preservados em ordem. O teste de 100 mil atualizações verifica que uma única notificação pendente basta.
- Modelos da fila são reutilizados; somente linhas alteradas notificam a interface. Textos longos têm uma prévia limitada, e o relatório completo é montado para a página técnica quando solicitada. O relatório persistido continua disponível.
- Não há consulta periódica de hardware, drivers, versões, temperatura ou Atlas. Inventário ocorre na abertura e por solicitação explícita; pedidos sobrepostos de inventário são agrupados por uma trava em memória.
- Ícones continuam locais e vetoriais. Spinners existem apenas durante carregamento/operação; na espera pela interface externa AME, a animação é retirada. Não há animação ociosa decorativa.

## Concorrência, rede e processos

- A fila executa **uma operação de instalação e um download por vez**, sequencialmente. Não existe pré-download de apps desmarcados. O inventário pode executar em uma única thread separada.
- Downloads, processos, consultas WinGet, inventário, verificação de assinatura e reversões de registro/energia executam fora da thread gráfica. Preferências locais pequenas continuam sendo gravadas atomicamente ao usuário alterar a seleção.
- Um cliente HTTP é criado somente quando a operação requer rede, reutilizado dentro do executor e descartado ao terminar. A confirmação de um arquivo já preparado não precisa criar cliente quando os arquivos estão íntegros.
- Cópia de downloads usa buffer de 64 KiB e SHA-256 incremental. Não carrega o instalador inteiro na RAM nem lê o arquivo inteiro uma segunda vez só para calcular o hash durante a transferência. A inspeção do Windows pode exigir nova verificação posterior, preservada por segurança.
- Saída de processos tem canais e buffers limitados, inclusive linhas sem quebra. O monitor de instalações espera notificações de processo/EOF, sem `try_wait` periódico. Consultas canceláveis usam espera de até 200 ms e limite total de 120 s para observar cancelamento; esse mecanismo existe somente durante a consulta.
- Instaladores iniciados diretamente pelo Ruhtweaks usam `ShellExecuteExW` e espera nativa de processo, preservando MOTW, avisos e UAC. O cancelamento da fila nunca mata o instalador atual.
- Não foi introduzida nova família de dependências. Foram habilitados recursos das crates Windows existentes. As poucas operações `unsafe` adicionais são FFI Windows para COM, status de energia/disco, assinatura e lançamento/espera de processo, com escopos e handles explícitos.

## Temporários e retomada

- Transferências usam nomes próprios `ruhtweaks-*.part`. Um recibo atômico registra o arquivo antes de receber o corpo. Interrupção, cancelamento e hash divergente não publicam um destino final.
- Ao retomar, somente o parcial identificado pelo recibo válido pode ser descartado. Nomes com travessia de diretório ou redirecionamento são recusados. Não há varredura de Downloads, Temp global, Windows.old ou perfis pessoais.
- Um instalador/playbook preparado só é removido após conclusão **verificada e gravada** no journal. A limpeza valida pasta, identidade e hash e nunca recorre por diretórios. Arquivos substituídos ou em uso são mantidos, com registro.
- Falha, confirmação pendente, reboot pendente e verificação inconclusiva preservam os arquivos necessários. A retomada também tenta concluir a limpeza de arquivos próprios já confirmados, sem repetir a instalação.
- Logs, perfis, backups e recibos pequenos permanecem para auditoria/retomada; não são cache de monitoramento. O cache administrado pelo próprio WinGet não é apagado pelo Ruhtweaks.
- Documentos JSON têm limite de 16 MiB, impedindo alocação sem limite por um arquivo de estado inválido. Arquivos recusados não são sobrescritos.

## Fechamento

Sem operação em andamento, fechar encerra o processo e suas threads. Durante um instalador controlado, o app solicita cancelamento das próximas etapas e espera a operação atual terminar. Na etapa de intervenção externa AME, é permitido fechar o Ruhtweaks: a ferramenta oficial já aberta pode continuar independentemente; recibos e checkpoints são preservados. Ao reabrir, a verificação é explícita e não existe tarefa de inicialização para reativar o app.

Aplicativos ou instaladores de terceiros que o usuário decidiu executar podem permanecer abertos por suas próprias regras. O Litecord v1.3.1 pode abrir o cliente ao concluir; isso é avisado antes da confirmação, inclusive no modo silencioso. Não é um componente residente do Ruhtweaks.

## Release e medição

Mantidos `opt-level = "s"`, LTO thin, uma unidade de geração e símbolos removidos. O objetivo é equilibrar tamanho, inicialização e execução real. `panic = "unwind"` foi mantido para preservar confiabilidade; não trocamos segurança por uma pequena redução de tamanho. Não foi adicionada instrumentação permanente nem benchmark de jogos/FPS. As medições técnicas de processo do build distribuído, quando executadas, são registradas no relatório da entrega com suas condições e limites; não constituem garantia de consumo em todos os PCs.

A revisão 0.3.0 removeu barra lateral e modelos de pares/colunas. AMD usa um helper WUA temporário apenas quando selecionado; a UI aguarda eventos e o helper encerra ao retornar. O próprio Windows Update pode consumir recursos durante a busca/instalação, sem monitoramento ocioso pelo app. Nenhuma nova família de dependências foi adicionada.

