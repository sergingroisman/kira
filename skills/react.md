---
name: react
description: Use when editing React/JSX components, hooks or render logic. Do NOT use for Vue/Svelte/Angular components or plain HTML.
license: MIT
author: KIRA
version: 1.0.0
---
# React

- **Componentes**: função por padrão; uma responsabilidade por componente; props explícitas e tipadas.
- **Estado**: estado local só quando pertence ao componente; estado compartilhado sobe para o menor ancestral comum ou para o store do projeto (Redux/Zustand/TanStack) — nunca duplique a mesma fonte.
- **Efeitos**: `useEffect` só para sincronizar com o mundo externo (rede, timers, DOM). Cálculo derivado é valor calculado no render, não efeito.
- **Dependências de efeito**: liste todas as reativas; se o efeito precisa de menos, mova o valor para fora ou use `useRef` conscientemente.
- **Listas**: `key` estável e única (id do dado, nunca índice quando a lista reordena).
- **Memoização**: `useMemo`/`useCallback` só com custo medido ou dependência de referência real (efeito, contexto, filho memoizado) — memoizar tudo é custo, não otimização.
- **Teste**: comportamento observável (texto renderizado, chamada de rede), não detalhe de implementação.
