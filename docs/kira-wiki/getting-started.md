# Getting started — a primeira conversa com o KIRA

Aqui é você e o KIRA, do zero ao primeiro ciclo completo. Cada passo tem o
comando literal e o que acontece — nada de "logo mais". Os termos canônicos
estão em [[glossario]]; o porquê de cada mecanismo, em [[engenharia-kira]].

## 0. Instale com hash verificado

Ainda não há binário publicado em release — hoje o caminho é compilar do
fonte (`cargo install --path .`, ou `cargo run -- <comando>` direto do repo).
Quando existir release, a instalação é **verificada e fail-closed**:

```bash
curl -fsSLO https://github.com/sergingroisman/kira/releases/latest/download/install.sh
curl -fsSLO https://github.com/sergingroisman/kira/releases/latest/download/SHA256SUMS
sha256sum -c --ignore-missing SHA256SUMS   # no macOS: shasum -a 256 -c …
bash install.sh
```

Se o hash não bater, o instalador **pára antes de gravar qualquer arquivo**.
O que o `SHA256SUMS` prova — e o que não prova: como ele vem do mesmo host do
binário, garante integridade de transporte (download truncado, troca parcial),
não autoria. Para exigir autoria, fixe o hash fora do canal:

```bash
curl -fsSLO https://github.com/sergingroisman/kira/releases/download/v0.1.0/install.sh
KIRA_PINNED_SHA256=<hash que você conferiu no repo> bash install.sh
```

(`-PinnedSha256` no PowerShell.) Hash publicado divergindo do fixado:
instalação para. No Windows: `.\install.ps1` (sem admin,
`%LOCALAPPDATA%\kira\bin`).

## 1. `kira setup` — ele pergunta só o que a máquina não sabe

```bash
kira setup
```

Em TTY grande abre um **wizard de tela cheia** com trilha à esquerda
(`Ambiente → Projeto → IA → Resumo`). A etapa **Ambiente** só mostra o que foi
detectado — SO, shell, keyring, caminhos de config/store, ferramentas no PATH
—, então você não responde o que a máquina já sabe. A etapa **Projeto**
mostra a **raiz detectada** (climb até `.git`/`Cargo.toml`/`package.json`) e
você só confirma. Na **IA**, 4 opções com custo e dados em cada uma e o
pré-requisito verificado na hora: `1) Ollama local` (custo zero, o código não
sai da máquina), `2) Copilot CLI`, `3) Microsoft Foundry` (BYOK) e
`4) nenhuma por agora` — o default quando o Ollama não está instalado. O caro
nunca é default nem fallback silencioso.

Repare no que o setup **não** pede: PAT. O KIRA funciona completo sem
integração externa — a credencial nasce em `kira connect`, no momento em que
faz sentido. No fim, o resumo diz a verdade:

```
pronto:
  dev: <nome>
  ia:  <backend ou "nenhuma">
  store: <caminho>

funciona agora:  scan · wiki · context · pr draft · dashboard
desativado:      judge de item do board · read (board) · publicar PR
ativar depois:   kira connect azure   (leitura de work items)
                 kira setup --backend ollama
próximo passo:   kira scan
```

"Próximo passo" e nada de "configuração concluída" — o KIRA não finge fim de
jornada. Não-interativo: `kira setup --yes` nunca bloqueia.

## 2. `kira connect azure` — a credencial nasce na hora certa

```bash
kira connect azure
```

Só se você quiser ler work items do board. O comando imprime a **aula do PAT**
(o que é, como criar, escopo Work Items → Read) e oferece o menu:

```
1) colar PAT
2) usar AZURE_DEVOPS_PAT do ambiente
3) cancelar
```

A entrada do segredo é **sem eco** e a validação é imediata — 200, 401, 403
ou sem rede, as 4 mensagens da spec. No 200, o PAT vai para o keyring do OS
(service `kira`) e a capacidade `board.azure` fica conectada **para o projeto
atual**. 401/403/cancelar ⇒ **nada gravado**, exit ≠ 0. Fora de TTY (ou com
`--yes`) a única origem é a env `AZURE_DEVOPS_PAT` — sem ela, erro acionável,
nunca um prompt pendurado.

## 3. `kira ingest` — ele lê o projeto inteiro, sem IA

```bash
kira ingest
```

Uma caminhada com **denylist** (nada de allowlist de extensão — stack nova
entra de graça), sniff de binário, teto por arquivo **com nota** (nunca skip
silencioso — binário e arquivo acima do cap aparecem como nota, nunca somem
calados) e detector de stack por marcador (`go.mod`, `pubspec.yaml`, `*.sln`,
`build.gradle.kts`, `app.json`, `Package.swift`, `terraform/`). O scan das
ferramentas da stack roda dentro (pule com `--skip-scan` para offline). No
fim:

- `.kira/facts.md` — os fatos do projeto, **determinístico**: mesma árvore ⇒
  mesmos facts, byte a byte;
- `.kira/ingest/{inventory.json,tree.txt}`;
- `.kira/findings.json` — achados do scan, com fingerprint FNV-1a e ordem
  estável;
- índice FTS5 do repositório no store, incremental por `(mtime, size)`, com
  **redact na escrita** — segredo nunca entra no índice.

Segundos, sem LLM, sem rede. E não pede trust: read-only roda sempre.

## 4. `kira trust` — você aceita o repositório

```bash
kira trust
```

Grava `.kira/trust.json` com o hash do caminho canônico. Este aceite é a
fronteira: **o `digest` — que envia código ao modelo — recusa rodar sem ele**.
Mover o repositório pede aceite de novo (o hash é do caminho). `--revoke`
apaga. Sem trust, nada sai da máquina.

## 5. `kira digest` — o julgamento local, com orçamento

```bash
kira digest
```

Agora sim o Ollama entra: `kira digest` enfileira em `posts` o que falta,
retira com lease e grava rascunhos em `.kira/drafts/<slug>.md` (kinds
`intro` e `c4-container`, prompts versionados). O que o orçamento significa
na prática:

- **1 M tokens por run** e **30 k por post** — estourou, para com nota, nunca
  com erro;
- **cache por `(input_hash, prompt_version, model, lang)`** — rodar de novo
  sem mudança não repete post; trocar `--lang` regenera (idioma é dado, não
  código);
- **`--pause N`** para depois de N posts e **`--resume`** continua de onde
  parou;
- ao final, `.kira/briefing.md` com o estado da fila e o próximo passo.

Sem Ollama no ar? Erro acionável com exit ≠ 0 — sem fallback silencioso para
nuvem. O default do modelo é `qwen3.5:4b` (Q4_K_M); troque por execução
(`--model`) ou env (`KIRA_MODEL`). Quer mais modelos? `kira model list`
mostra os instalados e `kira model pull hf.co/<repo>:<quant>` traz GGUF do
Hugging Face **via Ollama** — consentimento e disco checado antes, nunca
download silencioso (ADR-0040: HF é catálogo, nunca runtime).

## 6. `kira publish` — o que vai para a wiki passa por gate

```bash
kira publish
```

Publica os rascunhos em `.kira/wiki/<slug>.md` com front-matter
(`slug, kind, lang, model, prompt_version, source_hash, published_hash,
created_at`) e `index.md` **derivado** (nunca gerado por modelo — senão mente
quando a lista muda). O gate: cada página carrega `published_hash`; se você
editou a página à mão, o hash do arquivo não bate e o publish **recusa e
preserva** o seu texto. `--force` é a única porta — decisão explícita sua.
Idempotente: publicar duas vezes não muda nada.

## 7. `kira dashboard` — o estado vivo, leitura por padrão

```bash
kira dashboard
```

A TUI (Ratatui) mostra cabeçalho (projeto, backend LLM, idade do último
scan), painel de achados (ordem critical→low), sessões (trilha do store) e
wiki (o markdown real de `.kira/wiki/`, rendido por renderizador próprio, sem
dependência). **Navegar nunca executa**: `Tab`, `j/k`, `Enter`, `/`, `?` —
nenhuma tecla de navegação spawna, escreve ou toca rede. Ação só com tecla
explícita (`s`/`t`/`d`) seguida de **Enter na folha de confirmação**, que
declara os efeitos antes (disco/rede/LLM). `kira dashboard --read-only`
desliga as ações — e o e2e prova o read-only comparando o hash do
`store.db` byte-idêntico. Dado ausente aparece como `—` com a causa, nunca
inventado.

## 8. `kira doctor` — a verdade sobre a máquina

```bash
kira doctor
```

Diagnóstico completo: de onde veio cada valor (env, keyring ou
`config.toml`), teste de conexão e do nome do projeto (PAT sai mascarado),
capacidades conectadas (`capacidades: nenhuma conectada (modo local)` quando
não há), e a linha de ingestão/trust/fila do ciclo — posts desatualizados
quando o repo mudou. Se algo estiver estranho, o doctor aponta o próximo
comando.

## 9. `kira loop --once` — o primeiro tick

```bash
kira loop --once
```

Um tick do ciclo: `ingest → digest (só se trusted) → publish`, tudo local.
Com tudo já feito acima, este tick reprocessa só o que mudou e — se nada
mudou — termina com o estado byte-idêntico (o e2e prova o segundo tick sem
diff em facts, drafts, wiki e fila). É o modo que roda em CI e é a porta do
ciclo contínuo (`kira loop` roda até o ponto fixo "estável: nada a fazer").
Sem trust, o digest do tick é pulado com nota acionável e o ciclo segue com
exit 0.

## A restrição que atravessa tudo

Budget: **50 chamadas de LLM por dia** (default, `max_llm_calls_per_day` no
`config.toml`), contadas de verdade desde o contador `llm_calls` — comando
determinístico não consome budget, baixar modelo também não. Estourou?
O erro diz o número do dia e como retomar. E a regra-mestra de escrita:
**leitura é livre; escrita no mundo só com `--apply` e, no ciclo autônomo,
só depois de `awaiting_approval` + `kira approve` humano** — merge é sempre
humano. O que o KIRA ainda não faz, ele diz ([[contexto]] traz a fila). Nada
é prometido como pronto se não está no binário.
