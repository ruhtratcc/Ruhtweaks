# AtlasOS / AME — fluxo híbrido e restrições

Atualização 0.3.0: Windows 10 22H2 x64 pode preparar o Atlas 0.4.1 legado, não recomendado, sem aplicação automática. Veja INTEGRACAO-030.md para o contrato e hash calculado da fonte oficial. O restante desta página descreve o fluxo moderno.

Revisão: 02/11/2026. O contrato completo, hashes, resultado oficial e limitações estão em [INTEGRACAO-020](INTEGRACAO-020.md).

O adaptador revisado usa [Atlas 0.5.0-hotfix](https://github.com/Atlas-OS/Atlas/releases/tag/0.5.0-hotfix), playbook 0.5.0, e AME 0.8.4. A [configuração oficial](https://github.com/Atlas-OS/Atlas/blob/0.5.0-hotfix/src/playbook/playbook.conf) limita builds a 26100/26200. O gate local aceita Windows 11 x64, Professional/Enterprise/Education e variantes N listadas no código; Home, arquitetura/build desconhecida e novas releases são bloqueadas. Isso não comprova instalação limpa, licença nem todos os requisitos finais do wizard.

## Automático após aprovação do plano

- Verificar versão/edição/arquitetura, espaço próprio e reboot pendente do Windows Update Agent.
- Consultar fontes oficiais e obter os dois artefatos revisados, em sequência, validando nome, tag, digest, formato e assinatura quando disponível.
- Recuperar downloads parciais próprios e reutilizar arquivos íntegros, sem downloads duplicados.
- Persistir preparação e pendências antes de abrir uma ferramenta externa.
- Após solicitação de revalidação, consultar o resultado oficial do AME, boot e proteções; nunca concluir pela simples abertura de uma janela.

## Assistido e confirmado separadamente

Antes de abrir AME são verificados alimentação AC/ausência de bateria e arquivo preparado. A confirmação exige reconhecer origem, ausência de assinatura Authenticode, avisos do Windows e necessidade de backup/guia. A autorização é específica por hash/perfil, existe só na sessão e não é salva no journal. AME abre sem argumentos pela API normal do Windows. Não há CLI pública de aplicação completa validada, injeção de opções, OCR ou cliques automatizados.

Selecione o APBX preparado na ferramenta oficial. O app orienta preservar Defender, mitigações e atualizações automáticas; não adota padrões oficiais potencialmente incompatíveis. A opção oficial `browser-brave` pode instalar Brave, que será detectado antes de qualquer instalação redundante. O [guia oficial](https://docs.atlasos.net/getting-started/install/install-playbook/) continua necessário, incluindo instalação limpa quando exigida.

**Restrição essencial:** o playbook exige `DefenderToggled`, `NoAntivirus` e `UCPDDisabled` na preparação. O Ruhtweaks não desativa essas proteções nem ensina a contorná-las. A opção posterior `defender-enable` não remove os requisitos anteriores. Se a ferramenta exigir uma mudança incompatível, interrompa a aplicação. Preparar/abrir AME não significa que seja possível concluir Atlas dentro dessas regras.

## Resultado e retomada

O core oficial grava `HKLM\SOFTWARE\AME\Playbooks\Applied\{00000000-0000-4000-6174-6C6173203A33}`. O app exige evidência nova, versão 0.5.0, ErrorLevel zero, opções seguras, reboot posterior, nenhum reboot pendente e leituras positivas de Defender em tempo real, firewall ativo e UCPD. Falhas de leitura/permissão ou resultados parciais mantêm pendência.

O marcador reporta o resultado do AME; não é atestação criptográfica nem identifica o hash/hotfix do APBX aplicado. Essa limitação aparece na documentação e no fluxo. Não há rollback integral de Atlas, formatação ou reboot automático. A retomada é manual, sem serviço ou tarefa de logon. Arquivos necessários ficam preservados até conclusão persistida e verificada.

Se não quiser prosseguir, use **Continuar com apps sem concluir Atlas**. O app exige revisão/aprovação do plano restante e preserva a pendência no histórico. Nunca transforma a desistência em sucesso.

