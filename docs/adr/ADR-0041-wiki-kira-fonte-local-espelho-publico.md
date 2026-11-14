# ADR-0041 — Wiki do KIRA: fonte viva local, espelho público e GitHub Wiki

- **Status**: aceito (14/Nov/2026) — decisão do dono ("terminar o KIRA até criar a
  wiki no GitHub completo"), implementada na noite de 13→14/Nov.
- **Fonte de verdade da nomenclatura**: [`docs/nomenclatura.md`](../nomenclatura.md).

## Contexto

A wiki sobre o KIRA (contexto, engenharia, decisões, padrões, glossário,
getting-started) vive em `.kira/wiki/` — **local por contrato** (ADR-0013/0022;
`.kira/` não é commitado). A §8 do
[`docs/plano-finalizacao-kira.md`](../plano-finalizacao-kira.md) listava três saídas
para "wiki completa no GitHub": (a) GitHub Wiki alimentada por `kira wiki export`,
(b) docs commitados como fonte pública, (c) ambos. Recomendação registrada: (c).

## Decisão

1. **Fonte viva**: `.kira/wiki/*.md` (8 páginas) — o que o KIRA indexa (FTS5) e o
   que o `kira loop`/`publish` lê. Nunca commitada.
2. **Espelho público versionado**: `docs/kira-wiki/*.md` — cópia fiel das 8 páginas
   dentro do repositório (este PR). É o "completo no GitHub" imediato: versionado,
   público, revisável por PR. A fonte continua sendo `.kira/wiki/`; o espelho é
   regenerado por cópia (mesma ferramenta que gerou: escrita local + `kira wiki
   export` como bundle de transporte).
3. **GitHub Wiki nativa**: `has_wiki` foi habilitado no repositório (14/Nov). O repo
   git da wiki (`kira.wiki.git`) só é criado pelo GitHub **após a primeira página
   criada pela UI web** — não existe API para conteúdo de wiki. O kit de push fica
   pronto; fluxo do dono (uma vez):
   - abrir `github.com/sergingroisman/kira/wiki` e criar a página inicial (1 clique);
   - `git clone https://github.com/sergingroisman/kira.wiki.git && cd kira.wiki &&
     cp <espelho>/*.md . && git add . && git commit -m "wiki: conteúdo inicial" &&
     git push`.
   A página inicial (`Home.md`) já vai pronta no espelho depois do primeiro export.
4. **Estaleiro não componente**: o agente de build (Hermes/Delta) prepara e propõe;
   quem publica na GitHub Wiki é o dono (ator humano do fluxo).

## Consequências

- Conteúdo duplicado (local × espelho): a sincronização é manual e **de baixo custo**
  (`cp`); divergência visível é detectável comparando os diretórios. Se a wiki crescer
  como fonte de RAG, automatizar a cópia num comando (`kira wiki mirror`) — não agora.
- `docs/wiki/` continua aposentado pela ADR-0039 (era a localização stale). O
  espelho usa `docs/kira-wiki/` de propósito — nome distinto, sem ressurreição.
- Prova do conteúdo local (14/Nov, store limpo): `kira wiki reindex` = **8 páginas**;
  `wiki search "rendezvous"` ancora `padroes`+`decisoes`; `wiki search "estaleiro"`
  ancora `glossario`+`decisoes`.

## Alternativas rejeitadas

| Alternativa | Por que não |
|---|---|
| Commitar `.kira/` inteiro | fura o contrato local do `.kira/` (ADR-0013/0022) e commitaria store/trust/facts |
| Fazer o agente de build criar a primeira página da GitHub Wiki | não existe API; exigiria automação de UI — frágil e desnecessário |
| Publicar só o bundle (`kira wiki export`) como artefato de release | wiki viva não fica indexável/buscaável no GitHub; fica para depois do 1º clique |
