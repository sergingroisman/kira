# ADR-0025 — O `kira setup` ensina: aula do PAT, menu com pular e validação na hora

- **Status**: aceito (12/Nov/2026)
- **Decisores**: Sérgio (dono)

## Contexto

O setup pedia "PAT" sem explicar nada: campo sem contexto é onde a pessoa desiste
ou cola a senha da conta por medo de errar. O dono relatou: "o usuário não sabe
o que é PAT, tem que ensinar para ele e oferecer as opções que temos".

## Decisão

1. **Cada pergunta ensina**: uma frase do que é (analogia concreta), por que
   pedimos, como fazer (link exato `https://dev.azure.com/<org>/_usersSettings/
   tokens`, escopo **Work Items → Read**, validade 30/90 dias, valor aparece
   **uma única vez**) e a consequência de pular, dita sem drama. Nunca bloqueia.
2. **Menu obrigatório de 3 opções** (1 colar PAT [default em TTY], 2 usar
   `AZURE_DEVOPS_PAT` do ambiente, 3 pular ⇒ modo local `kira triage --file`).
   Não-interativo (`--yes`/sem TTY) usa a env se existir, senão pula — e
   imprime o pulado no resumo.
3. **Validação na hora**, com rótulo explícito (as 4 mensagens exatas da spec:
   200 / 401 / 403 / sem rede) usando uma leitura barata. 401/403 NÃO gravam;
    falha de rede grava e o `kira doctor` completa depois. Entrada de token sem
   eco (melhor esforço via `stty -echo` no Unix; o valor nunca é impresso —
   nada vai para stdout/stderr/log e nunca para prompt de LLM). No Windows não
   há mecanismo sem dependência nova: o prompt **avisa** que o valor pode
   ecoar, em vez de afirmar que está oculto; `SetupOptions::hidden_input`
   controla o pedido e o `stty` só roda quando ele é `true` (nunca em teste).
4. **Passo de backend de IA** com custo/dados em cada opção (ollama local: 0
   custo e código não sai da máquina; copilot: assinatura e política da
   empresa; foundry: BYOK, custo por token) e pré-requisito verificado
   (`ollama list` contém `qwen3.5:4b`; `copilot` no PATH; foundry pede
   endpoint + deployment + chave mascarada e faz UMA chamada de teste barata
   reportando 401/403/404). Regra: **backend caro nunca é default nem fallback
   silencioso**.
5. **Resumo final** no formato `pronto: / pulado (dá para fazer depois): /
   próximo passo:  kira scan` — nunca termina com "configuração concluída".
   `kira doctor` repete a explicação curta do PAT quando ele está ausente ou
   inválido (quem pulou entende o que falta sem rodar o wizard de novo).
6. O fluxo antigo "Você já tem um PAT pronto? [S/n]" + "Abrir no navegador
   agora?" e o `--no-browser` saem: o navegador não é mais aberto pelo setup —
   a aula imprime a URL e o usuário abre quando quiser.

## Alternativas rejeitadas

- **Exigir PAT na instalação** (bloquear o setup sem token): derruba quem está
  só experimentando e empurra o uso da senha da conta. Com a opção de pular,
  o KIRA já funciona em modo local e a retomada é `kira setup --pat`.
- **Continuar com a pergunta "já tem um PAT?" + abertura de navegador**: aula
  pequena demais e um passo a mais (confirmar navegador) que não ensina nada;
  a abertura automática também surpreende em terminais compartilhados.

## Consequências

- Positivas: menos desistência no primeiro contato; PAT errado é diagnosticado
  na hora (401/403/rede), não no primeiro `kira inspect`; modo local é um
  cidadão de primeira classe com resumo honesto do que ficou pendente.
- Negativas: a saída do setup ficou mais longa (ensinar custa linhas). O passo
  de "fonte Nerd" saiu do fluxo: o resumo apontava `kira setup --deps --only
  font`, um comando que o binário nunca teve — retomada inexistente é pior que
  etapa ausente. O eco oculto segue best-effort no Windows (sem dependência
  nova), agora declarado no próprio prompt.
