---
type: Dump
title: gitingest — tech-leads-club/agent-skills
- **Fonte**: https://github.com/tech-leads-club/agent-skills @ commit e74dc8a
- **Licença**: código MIT (Copyright (c) 2026 Tech Leads Club); conteúdo dos SKILL.md
  CC-BY-4.0 — atribuição: Tech Leads Club + autor indicado em `metadata.author` de cada skill.
- **Gerado por**: `docs/dumps/generate-agent-skills-dump.ps1` — caminhada ordenada por caminho;
  mesma entrada = mesmo dump. Fora de escopo: `references/`, `templates/`, código CLI/MCP.
---

# gitingest — agent-skills — categoria (performance) (4 skills)

================================================================
File: packages/skills-catalog/skills/(performance)/core-web-vitals/SKILL.md
================================================================
---
name: core-web-vitals
description: Optimize Core Web Vitals (LCP, INP, CLS) for better page experience and search ranking. Use when asked to "improve Core Web Vitals", "fix LCP", "reduce CLS", "optimize INP", "page experience optimization", or "fix layout shifts". Focuses specifically on the three Core Web Vitals metrics. Do NOT use for general web performance (use perf-web-optimization), Lighthouse audits (use perf-lighthouse), or Astro-specific optimization (use perf-astro).
license: MIT
metadata:
  author: web-quality-skills
  version: '1.0'
---

# Core Web Vitals optimization

Targeted optimization for the three Core Web Vitals metrics that affect Google Search ranking and user experience.

## The three metrics

| Metric  | Measures         | Good    | Needs work    | Poor    |
| ------- | ---------------- | ------- | ------------- | ------- |
| **LCP** | Loading          | ≤ 2.5s  | 2.5s – 4s     | > 4s    |
| **INP** | Interactivity    | ≤ 200ms | 200ms – 500ms | > 500ms |
| **CLS** | Visual Stability | ≤ 0.1   | 0.1 – 0.25    | > 0.25  |

Google measures at the **75th percentile** — 75% of page visits must meet "Good" thresholds.

---

## LCP: Largest Contentful Paint

LCP measures when the largest visible content element renders. Usually this is:

- Hero image or video
- Large text block
- Background image
- `<svg>` element

### Common LCP issues

**1. Slow server response (TTFB > 800ms)**

```
Fix: CDN, caching, optimized backend, edge rendering
```

**2. Render-blocking resources**

```html
<!-- ❌ Blocks rendering -->
<link rel="stylesheet" href="/all-styles.css" />

<!-- ✅ Critical CSS inlined, rest deferred -->
<style>
  /* Critical above-fold CSS */
</style>
<link rel="preload" href="/styles.css" as="style" onload="this.onload=null;this.rel='stylesheet'" />
```

**3. Slow resource load times**

```html
<!-- ❌ No hints, discovered late -->
<img src="/hero.jpg" alt="Hero" />

<!-- ✅ Preloaded with high priority -->
<link rel="preload" href="/hero.webp" as="image" fetchpriority="high" />
<img src="/hero.webp" alt="Hero" fetchpriority="high" />
```

**4. Client-side rendering delays**

```javascript
// ❌ Content loads after JavaScript
useEffect(() => {
  fetch('/api/hero-text')
    .then((r) => r.json())
    .then(setHeroText)
}, [])

// ✅ Server-side or static rendering
// Use SSR, SSG, or streaming to send HTML with content
export async function getServerSideProps() {
  const heroText = await fetchHeroText()
  return { props: { heroText } }
}
```

### LCP optimization checklist

```markdown
- [ ] TTFB < 800ms (use CDN, edge caching)
- [ ] LCP image preloaded with fetchpriority="high"
- [ ] LCP image optimized (WebP/AVIF, correct size)
- [ ] Critical CSS inlined (< 14KB)
- [ ] No render-blocking JavaScript in <head>
- [ ] Fonts don't block text rendering (font-display: swap)
- [ ] LCP element in initial HTML (not JS-rendered)
```

### LCP element identification

```javascript
// Find your LCP element
new PerformanceObserver((list) => {
  const entries = list.getEntries()
  const lastEntry = entries[entries.length - 1]
  console.log('LCP element:', lastEntry.element)
  console.log('LCP time:', lastEntry.startTime)
}).observe({ type: 'largest-contentful-paint', buffered: true })
```

---

## INP: Interaction to Next Paint

INP measures responsiveness across ALL interactions (clicks, taps, key presses) during a page visit. It reports the worst interaction (at 98th percentile for high-traffic pages).

### INP breakdown

Total INP = **Input Delay** + **Processing Time** + **Presentation Delay**

| Phase        | Target  | Optimization                |
| ------------ | ------- | --------------------------- |
| Input Delay  | < 50ms  | Reduce main thread blocking |
| Processing   | < 100ms | Optimize event handlers     |
| Presentation | < 50ms  | Minimize rendering work     |

### Common INP issues

**1. Long tasks blocking main thread**

```javascript
// ❌ Long synchronous task
function processLargeArray(items) {
  items.forEach((item) => expensiveOperation(item))
}

// ✅ Break into chunks with yielding
async function processLargeArray(items) {
  const CHUNK_SIZE = 100
  for (let i = 0; i < items.length; i += CHUNK_SIZE) {
    const chunk = items.slice(i, i + CHUNK_SIZE)
    chunk.forEach((item) => expensiveOperation(item))

    // Yield to main thread
    await new Promise((r) => setTimeout(r, 0))
    // Or use scheduler.yield() when available
  }
}
```

**2. Heavy event handlers**

```javascript
// ❌ All work in handler
button.addEventListener('click', () => {
  // Heavy computation
  const result = calculateComplexThing()
  // DOM updates
  updateUI(result)
  // Analytics
  trackEvent('click')
})

// ✅ Prioritize visual feedback
button.addEventListener('click', () => {
  // Immediate visual feedback
  button.classList.add('loading')

  // Defer non-critical work
  requestAnimationFrame(() => {
    const result = calculateComplexThing()
    updateUI(result)
  })

  // Use requestIdleCallback for analytics
  requestIdleCallback(() => trackEvent('click'))
})
```

**3. Third-party scripts**

```javascript
// ❌ Eagerly loaded, blocks interactions
;<script src="https://heavy-widget.com/widget.js"></script>

// ✅ Lazy loaded on interaction or visibility
const loadWidget = () => {
  import('https://heavy-widget.com/widget.js').then((widget) => widget.init())
}
button.addEventListener('click', loadWidget, { once: true })
```

**4. Excessive re-renders (React/Vue)**

```javascript
// ❌ Re-renders entire tree
function App() {
  const [count, setCount] = useState(0)
  return (
    <div>
      <Counter count={count} />
      <ExpensiveComponent /> {/* Re-renders on every count change */}
    </div>
  )
}

// ✅ Memoized expensive components
const MemoizedExpensive = React.memo(ExpensiveComponent)

function App() {
  const [count, setCount] = useState(0)
  return (
    <div>
      <Counter count={count} />
      <MemoizedExpensive />
    </div>
  )
}
```

### INP optimization checklist

```markdown
- [ ] No tasks > 50ms on main thread
- [ ] Event handlers complete quickly (< 100ms)
- [ ] Visual feedback provided immediately
- [ ] Heavy work deferred with requestIdleCallback
- [ ] Third-party scripts don't block interactions
- [ ] Debounced input handlers where appropriate
- [ ] Web Workers for CPU-intensive operations
```

### INP debugging

```javascript
// Identify slow interactions
new PerformanceObserver((list) => {
  for (const entry of list.getEntries()) {
    if (entry.duration > 200) {
      console.warn('Slow interaction:', {
        type: entry.name,
        duration: entry.duration,
        processingStart: entry.processingStart,
        processingEnd: entry.processingEnd,
        target: entry.target,
      })
    }
  }
}).observe({ type: 'event', buffered: true, durationThreshold: 16 })
```

---

## CLS: Cumulative Layout Shift

CLS measures unexpected layout shifts. A shift occurs when a visible element changes position between frames without user interaction.

**CLS Formula:** `impact fraction × distance fraction`

### Common CLS causes

**1. Images without dimensions**

```html
<!-- ❌ Causes layout shift when loaded -->
<img src="photo.jpg" alt="Photo" />

<!-- ✅ Space reserved -->
<img src="photo.jpg" alt="Photo" width="800" height="600" />

<!-- ✅ Or use aspect-ratio -->
<img src="photo.jpg" alt="Photo" style="aspect-ratio: 4/3; width: 100%;" />
```

**2. Ads, embeds, and iframes**

```html
<!-- ❌ Unknown size until loaded -->
<iframe src="https://ad-network.com/ad"></iframe>

<!-- ✅ Reserve space with min-height -->
<div style="min-height: 250px;">
  <iframe src="https://ad-network.com/ad" height="250"></iframe>
</div>

<!-- ✅ Or use aspect-ratio container -->
<div style="aspect-ratio: 16/9;">
  <iframe src="https://youtube.com/embed/..." style="width: 100%; height: 100%;"></iframe>
</div>
```

**3. Dynamically injected content**

```javascript
// ❌ Inserts content above viewport
notifications.prepend(newNotification)

// ✅ Insert below viewport or use transform
const insertBelow = viewport.bottom < newNotification.top
if (insertBelow) {
  notifications.prepend(newNotification)
} else {
  // Animate in without shifting
  newNotification.style.transform = 'translateY(-100%)'
  notifications.prepend(newNotification)
  requestAnimationFrame(() => {
    newNotification.style.transform = ''
  })
}
```

**4. Web fonts causing FOUT**

```css
/* ❌ Font swap shifts text */
@font-face {
  font-family: 'Custom';
  src: url('custom.woff2') format('woff2');
}

/* ✅ Optional font (no shift if slow) */
@font-face {
  font-family: 'Custom';
  src: url('custom.woff2') format('woff2');
  font-display: optional;
}

/* ✅ Or match fallback metrics */
@font-face {
  font-family: 'Custom';
  src: url('custom.woff2') format('woff2');
  font-display: swap;
  size-adjust: 105%; /* Match fallback size */
  ascent-override: 95%;
  descent-override: 20%;
}
```

**5. Animations triggering layout**

```css
/* ❌ Animates layout properties */
.animate {
  transition:
    height 0.3s,
    width 0.3s;
}

/* ✅ Use transform instead */
.animate {
  transition: transform 0.3s;
}
.animate.expanded {
  transform: scale(1.2);
}
```

### CLS optimization checklist

```markdown
- [ ] All images have width/height or aspect-ratio
- [ ] All videos/embeds have reserved space
- [ ] Ads have min-height containers
- [ ] Fonts use font-display: optional or matched metrics
- [ ] Dynamic content inserted below viewport
- [ ] Animations use transform/opacity only
- [ ] No content injected above existing content
```

### CLS debugging

```javascript
// Track layout shifts
new PerformanceObserver((list) => {
  for (const entry of list.getEntries()) {
    if (!entry.hadRecentInput) {
      console.log('Layout shift:', entry.value)
      entry.sources?.forEach((source) => {
        console.log('  Shifted element:', source.node)
        console.log('  Previous rect:', source.previousRect)
        console.log('  Current rect:', source.currentRect)
      })
    }
  }
}).observe({ type: 'layout-shift', buffered: true })
```

---

## Measurement tools

### Lab testing

- **Chrome DevTools** → Performance panel, Lighthouse
- **WebPageTest** → Detailed waterfall, filmstrip
- **Lighthouse CLI** → `npx lighthouse <url>`

### Field data (real users)

- **Chrome User Experience Report (CrUX)** → BigQuery or API
- **Search Console** → Core Web Vitals report
- **web-vitals library** → Send to your analytics

```javascript
import { onLCP, onINP, onCLS } from 'web-vitals'

function sendToAnalytics({ name, value, rating }) {
  gtag('event', name, {
    event_category: 'Web Vitals',
    value: Math.round(name === 'CLS' ? value * 1000 : value),
    event_label: rating,
  })
}

onLCP(sendToAnalytics)
onINP(sendToAnalytics)
onCLS(sendToAnalytics)
```

---

## Framework quick fixes

### Next.js

```jsx
// LCP: Use next/image with priority
import Image from 'next/image'
;<Image src="/hero.jpg" priority fill alt="Hero" />

// INP: Use dynamic imports
const HeavyComponent = dynamic(() => import('./Heavy'), { ssr: false })

// CLS: Image component handles dimensions automatically
```

### React

```jsx
// LCP: Preload in head
;<link rel="preload" href="/hero.jpg" as="image" fetchpriority="high" />

// INP: Memoize and useTransition
const [isPending, startTransition] = useTransition()
startTransition(() => setExpensiveState(newValue))

// CLS: Always specify dimensions in img tags
```

### Vue/Nuxt

```vue
<!-- LCP: Use nuxt/image with preload -->
<NuxtImg src="/hero.jpg" preload loading="eager" />

<!-- INP: Use async components -->
<component :is="() => import('./Heavy.vue')" />

<!-- CLS: Use aspect-ratio CSS -->
<img :style="{ aspectRatio: '16/9' }" />
```

## References

- [web.dev LCP](https://web.dev/articles/lcp)
- [web.dev INP](https://web.dev/articles/inp)
- [web.dev CLS](https://web.dev/articles/cls)
- [Performance skill](../performance/SKILL.md)

================================================================
File: packages/skills-catalog/skills/(performance)/perf-astro/SKILL.md
================================================================
---
name: perf-astro
description: 'Astro-specific performance optimizations for 95+ Lighthouse scores. Covers critical CSS inlining, compression, font loading, and LCP optimization. Use when optimizing Astro site performance, improving Astro Lighthouse scores, or configuring astro-critters. Do NOT use for non-Astro sites (use perf-web-optimization or core-web-vitals) or running Lighthouse audits (use perf-lighthouse).'
---

# Astro Performance Playbook

Astro-specific optimizations for 95+ Lighthouse scores.

## Quick Setup

```bash
npm install astro-critters @playform/compress
```

```js
// astro.config.mjs
import { defineConfig } from 'astro/config'
import critters from 'astro-critters'
import compress from '@playform/compress'

export default defineConfig({
  integrations: [
    critters(),
    compress({
      CSS: true,
      HTML: true,
      JavaScript: true,
      Image: false,
      SVG: false,
    }),
  ],
})
```

## Integrations

### astro-critters

Automatically extracts and inlines critical CSS. No configuration needed.

What it does:

- Scans rendered HTML for above-the-fold elements
- Inlines only the CSS those elements need
- Lazy-loads the rest

Build output shows what it inlined:

```
Inlined 40.70 kB (80% of original 50.50 kB) of _astro/index.xxx.css.
```

### @playform/compress

Minifies HTML, CSS, and JavaScript in the final build.

Options:

```js
compress({
  CSS: true, // Minify CSS
  HTML: true, // Minify HTML
  JavaScript: true, // Minify JS
  Image: false, // Skip if using external image optimization
  SVG: false, // Skip if SVGs are already optimized
})
```

## Layout Pattern

Structure your `Layout.astro` for performance:

```astro
---
import '../styles/global.css'
---

<!doctype html>
<html lang="pt-BR">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />

    <!-- Font fallback (prevents FOIT) -->
    <style>
      @font-face {
        font-family: 'Inter';
        font-display: swap;
        src: local('Inter');
      }
    </style>

    <!-- Non-blocking Google Fonts -->
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link
      rel="stylesheet"
      href="https://fonts.googleapis.com/css2?family=Inter:wght@400;600;700&display=swap"
      media="print"
      onload="this.media='all'"
    />
    <noscript>
      <link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@400;600;700&display=swap">
    </noscript>

    <!-- Preload LCP images -->
    <link rel="preload" as="image" href="/hero.png" fetchpriority="high">

    <title>{title}</title>

    <!-- Defer third-party scripts -->
    <script>
      let loaded = false;
      function loadAnalytics() {
        if (loaded) return;
        loaded = true;
        // Load GTM, analytics, etc.
      }
      ['scroll', 'click', 'touchstart'].forEach(e => {
        document.addEventListener(e, loadAnalytics, { once: true, passive: true });
      });
      setTimeout(loadAnalytics, 5000);
    </script>
  </head>
  <body>
    <slot />
  </body>
</html>
```

## Measuring

```bash
npx lighthouse https://your-site.com --preset=perf --form-factor=mobile
```

See also:

- **perf-lighthouse** - Running audits, reading reports, setting budgets
- **perf-web-optimization** - Core Web Vitals, bundle size, caching strategies

## Checklist

- [ ] `astro-critters` installed and configured
- [ ] `@playform/compress` installed and configured
- [ ] Google Fonts use `media="print" onload` pattern
- [ ] Third-party scripts deferred to user interaction
- [ ] LCP images preloaded in `<head>`

================================================================
File: packages/skills-catalog/skills/(performance)/perf-lighthouse/SKILL.md
================================================================
---
name: perf-lighthouse
description: 'Run Lighthouse audits locally via CLI or Node API, parse and interpret reports, and set performance budgets. Use when measuring site performance, understanding Lighthouse scores, setting up budgets, or integrating audits into CI. Triggers on: lighthouse, run lighthouse, lighthouse score, performance audit, performance budget. Do NOT use for fixing specific performance issues (use perf-web-optimization or core-web-vitals) or Astro-specific optimization (use perf-astro).'
---

# Lighthouse Audits

## CLI Quick Start

```bash
# Install
npm install -g lighthouse

# Basic audit
lighthouse https://example.com

# Mobile performance only (faster)
lighthouse https://example.com --preset=perf --form-factor=mobile

# Output JSON for parsing
lighthouse https://example.com --output=json --output-path=./report.json

# Output HTML report
lighthouse https://example.com --output=html --output-path=./report.html
```

## Common Flags

```bash
--preset=perf           # Performance only (skip accessibility, SEO, etc.)
--form-factor=mobile    # Mobile device emulation (default)
--form-factor=desktop   # Desktop
--throttling-method=devtools  # More accurate throttling
--only-categories=performance,accessibility  # Specific categories
--chrome-flags="--headless"   # Headless Chrome
```

## Performance Budgets

Create `budget.json`:

```json
[
  {
    "resourceSizes": [
      { "resourceType": "script", "budget": 200 },
      { "resourceType": "image", "budget": 300 },
      { "resourceType": "stylesheet", "budget": 50 },
      { "resourceType": "total", "budget": 500 }
    ],
    "resourceCounts": [{ "resourceType": "third-party", "budget": 5 }],
    "timings": [
      { "metric": "interactive", "budget": 3000 },
      { "metric": "first-contentful-paint", "budget": 1500 },
      { "metric": "largest-contentful-paint", "budget": 2500 }
    ]
  }
]
```

Run with budget:

```bash
lighthouse https://example.com --budget-path=./budget.json
```

## Node API

```javascript
import lighthouse from 'lighthouse'
import * as chromeLauncher from 'chrome-launcher'

async function runAudit(url) {
  const chrome = await chromeLauncher.launch({ chromeFlags: ['--headless'] })

  const result = await lighthouse(url, {
    port: chrome.port,
    onlyCategories: ['performance'],
    formFactor: 'mobile',
    throttling: {
      cpuSlowdownMultiplier: 4,
    },
  })

  await chrome.kill()

  const { performance } = result.lhr.categories
  const { 'largest-contentful-paint': lcp } = result.lhr.audits

  return {
    score: Math.round(performance.score * 100),
    lcp: lcp.numericValue,
  }
}
```

## GitHub Actions

```yaml
# .github/workflows/lighthouse.yml
name: Lighthouse

on:
  pull_request:
  push:
    branches: [main]

jobs:
  lighthouse:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Build site
        run: npm ci && npm run build

      - name: Run Lighthouse
        uses: treosh/lighthouse-ci-action@v11
        with:
          urls: |
            http://localhost:3000
            http://localhost:3000/about
          budgetPath: ./budget.json
          uploadArtifacts: true
          temporaryPublicStorage: true
        env:
          LHCI_GITHUB_APP_TOKEN: ${{ secrets.LHCI_GITHUB_APP_TOKEN }}
```

## Lighthouse CI (LHCI)

For full CI integration with historical tracking:

```bash
# Install
npm install -g @lhci/cli

# Initialize config
lhci wizard
```

Creates `lighthouserc.js`:

```javascript
module.exports = {
  ci: {
    collect: {
      url: ['http://localhost:3000/', 'http://localhost:3000/about'],
      startServerCommand: 'npm run start',
      numberOfRuns: 3,
    },
    assert: {
      assertions: {
        'categories:performance': ['error', { minScore: 0.9 }],
        'categories:accessibility': ['warn', { minScore: 0.9 }],
        'first-contentful-paint': ['error', { maxNumericValue: 1500 }],
        'largest-contentful-paint': ['error', { maxNumericValue: 2500 }],
        'cumulative-layout-shift': ['error', { maxNumericValue: 0.1 }],
      },
    },
    upload: {
      target: 'temporary-public-storage', // or 'lhci' for self-hosted
    },
  },
}
```

Run:

```bash
lhci autorun
```

## Parse JSON Report

```javascript
import fs from 'fs'

const report = JSON.parse(fs.readFileSync('./report.json'))

// Overall scores (0-1, multiply by 100 for percentage)
const scores = {
  performance: report.categories.performance.score,
  accessibility: report.categories.accessibility.score,
  seo: report.categories.seo.score,
}

// Core Web Vitals
const vitals = {
  lcp: report.audits['largest-contentful-paint'].numericValue,
  cls: report.audits['cumulative-layout-shift'].numericValue,
  fcp: report.audits['first-contentful-paint'].numericValue,
  tbt: report.audits['total-blocking-time'].numericValue,
}

// Failed audits
const failed = Object.values(report.audits)
  .filter((a) => a.score !== null && a.score < 0.9)
  .map((a) => ({ id: a.id, score: a.score, title: a.title }))
```

## Compare Builds

```bash
# Save baseline
lighthouse https://prod.example.com --output=json --output-path=baseline.json

# Run on PR
lighthouse https://preview.example.com --output=json --output-path=pr.json

# Compare (custom script)
node compare-reports.js baseline.json pr.json
```

Simple comparison script:

```javascript
const baseline = JSON.parse(fs.readFileSync(process.argv[2]))
const pr = JSON.parse(fs.readFileSync(process.argv[3]))

const metrics = ['largest-contentful-paint', 'cumulative-layout-shift', 'total-blocking-time']

metrics.forEach((metric) => {
  const base = baseline.audits[metric].numericValue
  const current = pr.audits[metric].numericValue
  const diff = (((current - base) / base) * 100).toFixed(1)
  const emoji = current <= base ? '✅' : '❌'
  console.log(`${emoji} ${metric}: ${diff}% (${base.toFixed(0)} → ${current.toFixed(0)})`)
})
```

## Troubleshooting

| Issue               | Solution                                              |
| ------------------- | ----------------------------------------------------- |
| Inconsistent scores | Run multiple times (`--number-of-runs=3`), use median |
| Chrome not found    | Set `CHROME_PATH` env var                             |
| Timeouts            | Increase with `--max-wait-for-load=60000`             |
| Auth required       | Use `--extra-headers` or puppeteer script             |

================================================================
File: packages/skills-catalog/skills/(performance)/perf-web-optimization/SKILL.md
================================================================
---
name: perf-web-optimization
description: 'Optimize web performance: bundle size, images, caching, lazy loading, and overall page speed. Use when site is slow, reducing bundle size, fixing layout shifts, improving Time to Interactive, or optimizing for Lighthouse scores. Triggers on: web performance, bundle size, page speed, slow site, lazy loading. Do NOT use for Core Web Vitals-specific fixes (use core-web-vitals), running Lighthouse audits (use perf-lighthouse), or Astro-specific optimization (use perf-astro).'
---

# Web Performance Optimization

Systematic approach: Measure → Identify → Prioritize → Implement → Verify.

## Target Metrics

| Metric | Good    | Needs Work | Poor    |
| ------ | ------- | ---------- | ------- |
| LCP    | < 2.5s  | 2.5-4s     | > 4s    |
| INP    | < 200ms | 200-500ms  | > 500ms |
| CLS    | < 0.1   | 0.1-0.25   | > 0.25  |
| TTFB   | < 800ms | 800ms-1.8s | > 1.8s  |

## Quick Wins

### 1. Images (usually biggest impact on LCP)

```html
<!-- Hero/LCP image: eager + high priority -->
<img src="/hero.webp" alt="Hero" width="1200" height="600" loading="eager" fetchpriority="high" decoding="async" />

<!-- Below fold: lazy load -->
<img src="/product.webp" alt="Product" width="400" height="300" loading="lazy" decoding="async" />
```

Always set `width` and `height` to prevent CLS.

### 2. Fonts (common LCP/CLS culprit)

```html
<!-- Preconnect to font origin -->
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />

<!-- Non-blocking font load -->
<link
  rel="stylesheet"
  href="https://fonts.googleapis.com/css2?family=Inter&display=swap"
  media="print"
  onload="this.media='all'"
/>
```

### 3. Third-party Scripts (common INP killer)

```html
<!-- Defer to user interaction -->
<script>
  function loadThirdParty() {
    // Load analytics, chat widgets, etc.
  }
  ;['scroll', 'click', 'touchstart'].forEach((e) => addEventListener(e, loadThirdParty, { once: true, passive: true }))
  setTimeout(loadThirdParty, 5000)
</script>
```

### 4. Critical CSS

Inline critical CSS in `<head>`, defer the rest:

```html
<style>
  /* critical styles */
</style>
<link rel="preload" href="/styles.css" as="style" onload="this.rel='stylesheet'" />
```

## Bundle Analysis

```bash
# Webpack
npx webpack-bundle-analyzer dist/stats.json

# Vite
npx vite-bundle-visualizer

# Check package size before installing
npx bundlephobia <package-name>
```

Common heavy packages to replace:

- `moment` (67KB) → `date-fns` (12KB) or `dayjs` (2KB)
- `lodash` (72KB) → cherry-pick imports or native methods

## Code Splitting Patterns

```javascript
// React lazy
const Chart = lazy(() => import('./Chart'))

// Next.js dynamic
const Admin = dynamic(() => import('./Admin'), { ssr: false })

// Vite/Rollup manual chunks
build: {
  rollupOptions: {
    output: {
      manualChunks: {
        vendor: ['react', 'react-dom']
      }
    }
  }
}
```

## Caching Headers

```
# Static assets (immutable hash in filename)
Cache-Control: public, max-age=31536000, immutable

# HTML (revalidate)
Cache-Control: no-cache

# API responses
Cache-Control: private, max-age=0, must-revalidate
```

## Measurement

For running audits, reading reports, and setting budgets, use the **perf-lighthouse** skill.

## Checklist

### Images

- [ ] Modern formats (WebP/AVIF)
- [ ] Responsive `srcset`
- [ ] `width`/`height` attributes
- [ ] `loading="lazy"` below fold
- [ ] `fetchpriority="high"` on LCP image

### JavaScript

- [ ] Bundle < 200KB gzipped
- [ ] Code splitting by route
- [ ] Third-party scripts deferred
- [ ] No unused dependencies

### CSS

- [ ] Critical CSS inlined
- [ ] Non-critical CSS deferred
- [ ] No unused CSS

### Fonts

- [ ] `font-display: swap`
- [ ] Preconnect to font origin
- [ ] Subset if possible

## Detailed Examples

For in-depth optimization patterns, see:

- [references/core-web-vitals.md](references/core-web-vitals.md) - Fixing LCP, CLS, INP issues
- [references/bundle-optimization.md](references/bundle-optimization.md) - Reducing JS bundle size
- [references/image-optimization.md](references/image-optimization.md) - Image formats, responsive images, sharp scripts
