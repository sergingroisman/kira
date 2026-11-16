# Lote inicial de `triage_gold` (gate M2) — PARA REVISÃO DO DONO

Estes 20 itens foram montados a partir dos **commits de correção reais do KIRA**:
a *causa raiz* proposta vem da justificativa do próprio fix, e os *arquivos* vêm do diff daquele commit.

**Regra de método**: o `item.json` traz só o SINTOMA observável (o que o dev reclamaria);
a causa fica no `gold.json`, fora do prompt. Se o sintoma já entregasse a causa, a medição não valeria nada.

## O que eu preciso de você

Para cada linha: **confirma** a causa raiz como verdadeira, ou **corrige** com a sua (você é o dono do “porquê”).
Só depois do seu ok estes rótulos valem como o lote que fecha o gate M2 (≥70%).

| # | pasta | commit | título | causa raiz proposta | arquivos | conf. | ok? |
|---|---|---|---|---|---|---|---|
| 01 | `bug-01-97b9154` | `97b9154` | bug 01 — pr draft sai oco quando o triage veio de --file | O rascunho recuperava o triage pela string dos argumentos do comando em vez do item gravado; com --file a chave nao casa e o lookup devolve vazio. | src/main.rs, src/prdraft.rs, src/store/mod.rs, src/tui/state.rs | alta | ☐ |
| 02 | `bug-02-8358f54` | `8358f54` | bug 02 — kira scan falha no Windows com erro de linha de comando | O wrapper cmd.exe recebia o chcp como argumento separado; precisa ser uma unica linha montada pelo shellline. | src/lib.rs, src/llm/copilot.rs, src/scan/dotnet.rs, src/scan/mod.rs | alta | ☐ |
| 03 | `bug-03-8347097` | `8347097` | bug 03 — CI intermitente: spawn do copilot falha com ETXTBSY | O shim era escrito direto no caminho final; gravar em temp e renomear atomicamente elimina a janela de arquivo em uso. | src/llm/copilot.rs | alta | ☐ |
| 04 | `bug-04-8de6e69` | `8de6e69` | bug 04 — segredo em comentario escapa da mascara do redact | Caracteres invisiveis e de bidi (zero-width, RLO) antes do segredo quebram o padrao do regex; e preciso removê-los antes de mascarar. | src/redact.rs | media | ☐ |
| 05 | `bug-05-8f1f206` | `8f1f206` | bug 05 — Esc nao volta de tela e Ctrl+C nao sai no dashboard | O tratamento de teclas nao distinguia o modo de busca do modo de tela; Esc sem destino e saida sem rota ficavam sem efeito. | src/tui/app.rs, src/tui/state.rs | media | ☐ |
| 06 | `bug-06-8729987` | `8729987` | bug 06 — dashboard trava na tela de detalhe e Move ignora o painel focado | A saida so existia no estado inicial e o Move nao era escopado pelo painel focado. | src/tui/app.rs, src/tui/keymap.rs, src/tui/state.rs | media | ☐ |
| 07 | `bug-07-f4c2462` | `f4c2462` | bug 07 — cabecalho do dashboard quebra a linha e desalinha os paineis | Faltava disciplina de linha no cabecalho: cada campo precisa ser truncado em uma unica linha. | src/tui/app.rs, src/tui/state.rs, src/tui/widgets/header.rs | baixa | ☐ |
| 08 | `bug-08-d0787cc` | `d0787cc` | bug 08 — build do binario falha fora do Windows | O import de Path so e usado dentro do bloco cfg(windows); fora dele vira erro de import nao utilizado com -D warnings. | src/llm/copilot.rs | alta | ☐ |
| 09 | `bug-09-ee7748f` | `ee7748f` | bug 09 — prompt com aspas internas chega corrompido ao copilot no Windows | O std recitava a linha passada ao cmd /C; o raw_arg evita a recitacao e as aspas internas precisam virar apostrofo. | src/llm/copilot.rs | media | ☐ |
| 10 | `bug-10-a8c508c` | `a8c508c` | bug 10 — comando com quebra de linha quebra o spawn no Windows | A linha do cmd /C precisa ser citada e escapada como um unico argumento antes do spawn. | src/llm/copilot.rs | media | ☐ |
| 11 | `bug-11-6694772` | `6694772` | bug 11 — scan reporta nome de ferramenta inexistente no Windows | A busca no PATH resolvia case-insensitive (como o NTFS) mas devolvia o nome procurado em vez do nome real encontrado no disco. | src/scan/mod.rs | alta | ☐ |
| 12 | `bug-12-3dd8fa3` | `3dd8fa3` | bug 12 — scan classifica ausencia de ferramenta como falha de execucao | A ferramenta era spawnada pelo nome nu antes de qualquer resolucao; falta resolver o caminho which-style ANTES do spawn e tratar ausencia como erro proprio. | src/scan/dotnet.rs, src/scan/mod.rs, src/scan/node.rs | alta | ☐ |
| 13 | `bug-13-7c55301` | `7c55301` | bug 13 — pnpm audit gera um finding inutil em vez de um por advisory | O parser esperava o formato do npm e nao o do pnpm audit real (por advisory), entao colapsava tudo em um item. | src/scan/node.rs | media | ☐ |
| 14 | `bug-14-b06ac98` | `b06ac98` | bug 14 — wiki gravada no lugar errado e pagina vazia entra no indice | O destino era docs/wiki em vez de .kira/wiki, episodio vazio nao era filtrado antes de gravar, e o display nao removia o prefixo \?\ do caminho canonico. | src/context/mod.rs, src/context/repo.rs, src/main.rs, src/store/mod.rs | media | ☐ |
| 15 | `bug-15-c3565dd` | `c3565dd` | bug 15 — triage responde sem os cabecalhos e o comando sai com sucesso | Resposta sem nenhum dos tres cabecalhos obrigatorios precisa virar erro de formato acionavel (exit diferente de zero), nunca sucesso com placeholder. | src/error.rs, src/triage.rs | alta | ☐ |
| 16 | `bug-16-ff5d174` | `ff5d174` | bug 16 — scan de workspace pnpm ignora os pacotes membros | package.json sem lockfile proprio sob workspace pnpm e MEMBRO do workspace: precisa dobrar em members em vez de spawnar auditoria propria. | src/scan/mod.rs | media | ☐ |
| 17 | `bug-17-2734fab` | `2734fab` | bug 17 — listagem de sessoes falha em maquinas com muitas sessoes | O cast de usize para i64 no LIMIT podia enrolar para valor negativo; precisa saturar no maximo. | src/store/mod.rs | media | ☐ |
| 18 | `bug-18-7581854` | `7581854` | bug 18 — projetos .NET nao sao detectados quando a extensao tem caixa diferente | A deteccao comparava a extensao com case-sensitivity, o que quebra no Windows onde o filesystem e case-insensitive. | src/scan/mod.rs | alta | ☐ |
| 19 | `bug-19-3e250b1` | `3e250b1` | bug 19 — mensagem de ferramenta ausente sai ilegivel no Windows | O wrapper cmd nao forcava UTF-8 na saida, entao o texto localizado do sistema chegava em mojibake e nao era classificavel. | src/scan/mod.rs | media | ☐ |
| 20 | `bug-20-eaf6571` | `eaf6571` | bug 20 — scan trata ferramenta ausente como falha generica de execucao | Faltava classificar o texto do wrapper cmd de ferramenta ausente e canonicalizar o caminho comparado no e2e. | src/scan/dotnet.rs, src/scan/mod.rs, src/scan/node.rs | baixa | ☐ |

Para corrigir: edite o `gold.json` da pasta (campos `causa_raiz`, `arquivos`, `confianca`)
e troque `nota` para `rotulado pelo dono em <data>`. Se um item não for um bug de verdade, apague a pasta —
o teste `tests/triage_gold_fixtures.rs` garante que o lote continua válido, mas o **número 20 é o gate**,
então apagar um exige repor outro.
