# KIRA em Docker — o container é o AGENDADOR, não um daemon.
#
# Por que assim: o mandato rejeita daemon Axum/SSE/serviço separado (o loop é
# modo do binário), mas não rejeita container. Aqui o cron do container dá o
# batimento (tick) que chama o metabolismo já existente:
#   ingest → (trust?) → digest → publish
# Sem trust no repo, o tick ingere e PARA (nada de código saindo da máquina).
# Sem Ollama vivo, o tick loga nota e segue. Nenhum --apply: escrita no mundo
# continua exigindo `kira approve` humano (ADR-0017).

# ---------- build: compila o binário único ----------
FROM rust:bookworm AS build
WORKDIR /src
# Só o que o build precisa (o .dockerignore da raiz cuida do resto).
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

# ---------- runtime: debian slim + cron ----------
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends cron curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/kira /usr/local/bin/kira
COPY docker/kira-tick.sh /usr/local/bin/kira-tick.sh
COPY docker/entrypoint.sh /usr/local/bin/kira-entrypoint.sh
RUN chmod +x /usr/local/bin/kira-tick.sh /usr/local/bin/kira-entrypoint.sh

# Defaults sensatos; o compose sobrescreve.
ENV KIRA_PROJECT=/repo \
    KIRA_TICK_MINUTES=30 \
    KIRA_POSTS_PER_TICK=1 \
    KIRA_OLLAMA_URL=http://localhost:11434

# O repo do dono entra montado (KIRA só escreve em /repo/.kira por construção);
# a config/store vivem num volume para sobreviver ao container.
VOLUME ["/repo", "/root/.config/kira"]

ENTRYPOINT ["/usr/local/bin/kira-entrypoint.sh"]
