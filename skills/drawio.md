---
name: drawio
description: Use when producing or editing draw.io diagrams (architecture, flow, C4). Do NOT use for free-form illustration or slide decks.
license: MIT
author: KIRA
version: 1.0.0
---
# draw.io

- **Um diagrama, uma pergunta**: arquitetura mostra componentes e fronteiras; fluxo mostra o caminho de um evento. Misturar os dois esconde os dois.
- **Nomes do domínio**: rótulos com o vocabulário do produto (`digest`, `turn`, `post`), nunca "serviço A"/"módulo X".
- **Direção**: fluxo de dados da esquerda para a direita ou de cima para baixo; seta só quando existe chamada/dependência real.
- **Fronteiras**: agrupe por dono do dado (cliente, KIRA, board) com retângulo tracejado; dependência que cruza fronteira é decisão, não detalhe.
- **Texto mínimo**: cada caixa com 2–4 palavras; o detalhe vive no texto fora da imagem, não dentro.
- **Legenda**: quando houver mais de um tipo de seta (leitura/escrita, sync/async), legenda obrigatória.
