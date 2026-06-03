# Kira web search — setup

Kira's `web_search` runs a fallback chain: **Brave → SearXNG**. Set up either
(or both — Brave is faster, SearXNG is unlimited and keyless).

DuckDuckGo scraping was dropped: its HTML endpoint returns an anti-bot `202`
challenge to non-browser clients, so it never returns results.

## Option A — Brave API (fast, free 2000/mo)

1. Grab a free key: https://api.search.brave.com (Data for AI, free tier).
2. Add it to `.env` at the repo root:
   ```
   BRAVE_API_KEY=your-key-here
   ```
3. Restart `kira-cored`. Brave is the chain's primary — done.

## Option B — SearXNG (self-host, unlimited, keyless)

Requires Docker.

```bash
cd deploy/searxng
docker compose up -d        # Docker Compose v2
#   or, if you have the standalone binary:
# docker-compose up -d      # Compose v1
# verify JSON output works:
curl 'http://127.0.0.1:8888/search?q=rust&format=json' | head
```

Kira hits `http://127.0.0.1:8888` by default (`WebToolsConfig.searxng_base_url`).
JSON format + disabled bot-limiter are set in `settings.yml`.

Stop: `docker compose down`.

## Both

With Brave key set **and** SearXNG running, the chain uses Brave first and
falls back to SearXNG on any failure (quota, network) — maximum robustness.
