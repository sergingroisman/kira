---
name: tanstack
description: Use when adding data fetching, caching or tables with TanStack Query/Table. Do NOT use for simple fetch-once pages or server-rendered data without client cache.
license: MIT
author: KIRA
version: 1.0.0
---
# TanStack Query / Table

- **Query key**: sempre um array com o recurso e os filtros que mudam a resposta (`['issues', projectId, state]`); key incompleta serve dado errado para outra tela.
- **staleTime**: declare a política por recurso (segundos). Sem ela o default revalida sempre — custo escondido.
- **Mutação**: `onSuccess` invalida as keys afetadas; nunca atualize o cache "no escuro" sem invalidar a fonte.
- **Erro e loading**: trate os três estados (loading, erro, vazio) na tela — `isError` com mensagem acionável, nunca spinner eterno.
- **Table**: colunas declarativas (`columns`) separadas da renderização; sorting/pagination no servidor quando a fonte é paginada.
- **Sem duplicação**: dois componentes que pedem a mesma key compartilham o cache — não crie uma segunda query "para garantir".
