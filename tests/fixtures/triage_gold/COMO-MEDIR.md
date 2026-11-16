# Como medir o gate M2 na sua máquina (o teste do MVP do KIRA)

Isto **não roda no VPS**: o `--eval` chama o seu Ollama. É o teste do MVP — o
número que decide se o KIRA acerta a causa raiz em ≥70% dos 20 bugs reais.

## Pré-requisitos (5 minutos)

1. **Ollama rodando**: `ollama list` deve responder e ter o modelo do triage
   (default `qwen3.5:4b`; se usar outro, passe `--model`).
2. **KIRA compilado na sua máquina**: `cargo build --release` no repo, ou o
   binário que você já usa.
3. **`kira doctor` limpo** — ele mostra o modelo, o teto de chamadas do dia e a
   saúde do provedor (`ollama: ok — último sucesso há X`).
4. **Repositório aceito**: o eval roda o triage em modo offline (`--file`), mas
   o digest/triage seguem a regra de trust. Rode `kira trust` no repo do KIRA se
   ainda não fez.

## O comando

```bash
kira triage --eval tests/fixtures/triage_gold --json > m2.json
echo "exit=$?"
```

- **exit 0** = ≥70% (PASSOU) · **exit 1** = <70% (NÃO PASSOU) ou erro.
- Cada item é uma pasta com `item.json` (o sintoma) + `gold.json` (a causa
  raiz que você confirmou). A comparação é determinística: similaridade de
  tokens ≥ 0.50 na causa, e basename dos arquivos — nenhum LLM julga outro LLM.
- `m2.json` traz, por item: score, se acertou causa e arquivos, o texto previsto
  e o texto do gold. É o que eu preciso ver para dizer onde o triage erra.

Rodada de fumaça antes, se quiser confirmar o formato (usa 3 fixtures
sintéticas, ~1 min):

```bash
kira triage --eval /tmp/eval-seed --seed
```

## Orçamento

São **20 chamadas de LLM** (1 por item) — dentro do teto de 50/dia. Se um item
falhar por 429, o KIRA agora respeita 5 min de silêncio e diz isso; espere e
rode de novo em vez de insistir.

## O que me mandar de volta

Só o `m2.json` (ou o `stderr` se não gerar arquivo). Com ele eu:

1. digo o percentual e **quais** itens erraram;
2. olho se o erro é do triage (prompt/packer) ou do rótulo (sintoma ambíguo);
3. abro o PR de correção do que for do código — sem mexer no seu rótulo.

## Se um item estiver mal rotulado

Edite o `gold.json` da pasta (`causa_raiz`, `arquivos`, `confianca`) e troque a
`nota` para `corrigido pelo dono em <data>`. O teste
`tests/triage_gold_fixtures.rs` garante que o lote continua válido depois da
edição, e **20 é o número do gate**: apagar um exige repor outro.
