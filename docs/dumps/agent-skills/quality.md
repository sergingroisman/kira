---
type: Dump
title: gitingest — tech-leads-club/agent-skills
- **Fonte**: https://github.com/tech-leads-club/agent-skills @ commit e74dc8a
- **Licença**: código MIT (Copyright (c) 2026 Tech Leads Club); conteúdo dos SKILL.md
  CC-BY-4.0 — atribuição: Tech Leads Club + autor indicado em `metadata.author` de cada skill.
- **Gerado por**: `docs/dumps/generate-agent-skills-dump.ps1` — caminhada ordenada por caminho;
  mesma entrada = mesmo dump. Fora de escopo: `references/`, `templates/`, código CLI/MCP.
---

# gitingest — agent-skills — categoria (quality) (7 skills)

================================================================
File: packages/skills-catalog/skills/(quality)/react-best-practices/SKILL.md
================================================================
---
name: react-best-practices
description: React and Next.js performance optimization guidelines from Vercel Engineering. Use when writing, reviewing, or refactoring React/Next.js code to ensure optimal performance patterns. Triggers on tasks involving React components, Next.js pages, data fetching, bundle optimization, or performance improvements. Do NOT use for component API architecture or composition patterns (use react-composition-patterns instead).
license: MIT
metadata:
  author: vercel
  version: '1.0.0'
---

# Vercel React Best Practices

Comprehensive performance optimization guide for React and Next.js applications, maintained by Vercel. Contains 57 rules across 8 categories, prioritized by impact to guide automated refactoring and code generation.

## When to Apply

Reference these guidelines when:

- Writing new React components or Next.js pages
- Implementing data fetching (client or server-side)
- Reviewing code for performance issues
- Refactoring existing React/Next.js code
- Optimizing bundle size or load times

## Rule Categories by Priority

| Priority | Category                  | Impact      | Prefix       |
| -------- | ------------------------- | ----------- | ------------ |
| 1        | Eliminating Waterfalls    | CRITICAL    | `async-`     |
| 2        | Bundle Size Optimization  | CRITICAL    | `bundle-`    |
| 3        | Server-Side Performance   | HIGH        | `server-`    |
| 4        | Client-Side Data Fetching | MEDIUM-HIGH | `client-`    |
| 5        | Re-render Optimization    | MEDIUM      | `rerender-`  |
| 6        | Rendering Performance     | MEDIUM      | `rendering-` |
| 7        | JavaScript Performance    | LOW-MEDIUM  | `js-`        |
| 8        | Advanced Patterns         | LOW         | `advanced-`  |

## Quick Reference

### 1. Eliminating Waterfalls (CRITICAL)

- `async-defer-await` - Move await into branches where actually used
- `async-parallel` - Use Promise.all() for independent operations
- `async-dependencies` - Use better-all for partial dependencies
- `async-api-routes` - Start promises early, await late in API routes
- `async-suspense-boundaries` - Use Suspense to stream content

### 2. Bundle Size Optimization (CRITICAL)

- `bundle-barrel-imports` - Import directly, avoid barrel files
- `bundle-dynamic-imports` - Use next/dynamic for heavy components
- `bundle-defer-third-party` - Load analytics/logging after hydration
- `bundle-conditional` - Load modules only when feature is activated
- `bundle-preload` - Preload on hover/focus for perceived speed

### 3. Server-Side Performance (HIGH)

- `server-auth-actions` - Authenticate server actions like API routes
- `server-cache-react` - Use React.cache() for per-request deduplication
- `server-cache-lru` - Use LRU cache for cross-request caching
- `server-dedup-props` - Avoid duplicate serialization in RSC props
- `server-serialization` - Minimize data passed to client components
- `server-parallel-fetching` - Restructure components to parallelize fetches
- `server-after-nonblocking` - Use after() for non-blocking operations

### 4. Client-Side Data Fetching (MEDIUM-HIGH)

- `client-swr-dedup` - Use SWR for automatic request deduplication
- `client-event-listeners` - Deduplicate global event listeners
- `client-passive-event-listeners` - Use passive listeners for scroll
- `client-localstorage-schema` - Version and minimize localStorage data

### 5. Re-render Optimization (MEDIUM)

- `rerender-defer-reads` - Don't subscribe to state only used in callbacks
- `rerender-memo` - Extract expensive work into memoized components
- `rerender-memo-with-default-value` - Hoist default non-primitive props
- `rerender-dependencies` - Use primitive dependencies in effects
- `rerender-derived-state` - Subscribe to derived booleans, not raw values
- `rerender-derived-state-no-effect` - Derive state during render, not effects
- `rerender-functional-setstate` - Use functional setState for stable callbacks
- `rerender-lazy-state-init` - Pass function to useState for expensive values
- `rerender-simple-expression-in-memo` - Avoid memo for simple primitives
- `rerender-move-effect-to-event` - Put interaction logic in event handlers
- `rerender-transitions` - Use startTransition for non-urgent updates
- `rerender-use-ref-transient-values` - Use refs for transient frequent values

### 6. Rendering Performance (MEDIUM)

- `rendering-animate-svg-wrapper` - Animate div wrapper, not SVG element
- `rendering-content-visibility` - Use content-visibility for long lists
- `rendering-hoist-jsx` - Extract static JSX outside components
- `rendering-svg-precision` - Reduce SVG coordinate precision
- `rendering-hydration-no-flicker` - Use inline script for client-only data
- `rendering-hydration-suppress-warning` - Suppress expected mismatches
- `rendering-activity` - Use Activity component for show/hide
- `rendering-conditional-render` - Use ternary, not && for conditionals
- `rendering-usetransition-loading` - Prefer useTransition for loading state

### 7. JavaScript Performance (LOW-MEDIUM)

- `js-batch-dom-css` - Group CSS changes via classes or cssText
- `js-index-maps` - Build Map for repeated lookups
- `js-cache-property-access` - Cache object properties in loops
- `js-cache-function-results` - Cache function results in module-level Map
- `js-cache-storage` - Cache localStorage/sessionStorage reads
- `js-combine-iterations` - Combine multiple filter/map into one loop
- `js-length-check-first` - Check array length before expensive comparison
- `js-early-exit` - Return early from functions
- `js-hoist-regexp` - Hoist RegExp creation outside loops
- `js-min-max-loop` - Use loop for min/max instead of sort
- `js-set-map-lookups` - Use Set/Map for O(1) lookups
- `js-tosorted-immutable` - Use toSorted() for immutability

### 8. Advanced Patterns (LOW)

- `advanced-event-handler-refs` - Store event handlers in refs
- `advanced-init-once` - Initialize app once per app load
- `advanced-use-latest` - useLatest for stable callback refs

## How to Use

Read individual rule files for detailed explanations and code examples:

```
rules/async-parallel.md
rules/bundle-barrel-imports.md
```

Each rule file contains:

- Brief explanation of why it matters
- Incorrect code example with explanation
- Correct code example with explanation
- Additional context and references

## Full Compiled Document

For the complete guide with all rules expanded: `AGENTS.md`

================================================================
File: packages/skills-catalog/skills/(quality)/seo/SKILL.md
================================================================
---
name: seo
description: Optimize for search engine visibility and ranking. Use when asked to "improve SEO", "optimize for search", "fix meta tags", "add structured data", "sitemap optimization", or "search engine optimization". Do NOT use for accessibility (use web-accessibility), performance (use core-web-vitals), or comprehensive site audits covering multiple areas (use web-quality-audit).
license: MIT
metadata:
  author: web-quality-skills
  version: '1.0'
---

# SEO optimization

Search engine optimization based on Lighthouse SEO audits and Google Search guidelines. Focus on technical SEO, on-page optimization, and structured data.

## SEO fundamentals

Search ranking factors (approximate influence):

| Factor                            | Influence | This Skill                                         |
| --------------------------------- | --------- | -------------------------------------------------- |
| Content quality & relevance       | ~40%      | Partial (structure)                                |
| Backlinks & authority             | ~25%      | ✗                                                  |
| Technical SEO                     | ~15%      | ✓                                                  |
| Page experience (Core Web Vitals) | ~10%      | See [Core Web Vitals](../core-web-vitals/SKILL.md) |
| On-page SEO                       | ~10%      | ✓                                                  |

---

## Technical SEO

### Crawlability

**robots.txt:**

```text
# /robots.txt
User-agent: *
Allow: /

# Block admin/private areas
Disallow: /admin/
Disallow: /api/
Disallow: /private/

# Don't block resources needed for rendering
# ❌ Disallow: /static/

Sitemap: https://example.com/sitemap.xml
```

**Meta robots:**

```html
<!-- Default: indexable, followable -->
<meta name="robots" content="index, follow" />

<!-- Noindex specific pages -->
<meta name="robots" content="noindex, nofollow" />

<!-- Indexable but don't follow links -->
<meta name="robots" content="index, nofollow" />

<!-- Control snippets -->
<meta name="robots" content="max-snippet:150, max-image-preview:large" />
```

**Canonical URLs:**

```html
<!-- Prevent duplicate content issues -->
<link rel="canonical" href="https://example.com/page" />

<!-- Self-referencing canonical (recommended) -->
<link rel="canonical" href="https://example.com/current-page" />

<!-- For paginated content -->
<link rel="canonical" href="https://example.com/products" />
<!-- Or use rel="prev" / rel="next" for explicit pagination -->
```

### XML sitemap

```xml
<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>https://example.com/</loc>
    <lastmod>2024-01-15</lastmod>
    <changefreq>daily</changefreq>
    <priority>1.0</priority>
  </url>
  <url>
    <loc>https://example.com/products</loc>
    <lastmod>2024-01-14</lastmod>
    <changefreq>weekly</changefreq>
    <priority>0.8</priority>
  </url>
</urlset>
```

**Sitemap best practices:**

- Maximum 50,000 URLs or 50MB per sitemap
- Use sitemap index for larger sites
- Include only canonical, indexable URLs
- Update `lastmod` when content changes
- Submit to Google Search Console

### URL structure

```
✅ Good URLs:
https://example.com/products/blue-widget
https://example.com/blog/how-to-use-widgets

❌ Poor URLs:
https://example.com/p?id=12345
https://example.com/products/item/category/subcategory/blue-widget-2024-sale-discount
```

**URL guidelines:**

- Use hyphens, not underscores
- Lowercase only
- Keep short (< 75 characters)
- Include target keywords naturally
- Avoid parameters when possible
- Use HTTPS always

### HTTPS & security

```html
<!-- Ensure all resources use HTTPS -->
<img src="https://example.com/image.jpg" />

<!-- Not: -->
<img src="http://example.com/image.jpg" />
```

**Security headers for SEO trust signals:**

```
Strict-Transport-Security: max-age=31536000; includeSubDomains
X-Content-Type-Options: nosniff
X-Frame-Options: DENY
```

---

## On-page SEO

### Title tags

```html
<!-- ❌ Missing or generic -->
<title>Page</title>
<title>Home</title>

<!-- ✅ Descriptive with primary keyword -->
<title>Blue Widgets for Sale | Premium Quality | Example Store</title>
```

**Title tag guidelines:**

- 50-60 characters (Google truncates ~60)
- Primary keyword near the beginning
- Unique for every page
- Brand name at end (unless homepage)
- Action-oriented when appropriate

### Meta descriptions

```html
<!-- ❌ Missing or duplicate -->
<meta name="description" content="" />

<!-- ✅ Compelling and unique -->
<meta
  name="description"
  content="Shop premium blue widgets with free shipping. 30-day returns. Rated 4.9/5 by 10,000+ customers. Order today and save 20%."
/>
```

**Meta description guidelines:**

- 150-160 characters
- Include primary keyword naturally
- Compelling call-to-action
- Unique for every page
- Matches page content

### Heading structure

```html
<!-- ❌ Poor structure -->
<h2>Welcome to Our Store</h2>
<h4>Products</h4>
<h1>Contact Us</h1>

<!-- ✅ Proper hierarchy -->
<h1>Blue Widgets - Premium Quality</h1>
<h2>Product Features</h2>
<h3>Durability</h3>
<h3>Design</h3>
<h2>Customer Reviews</h2>
<h2>Pricing</h2>
```

**Heading guidelines:**

- Single `<h1>` per page (the main topic)
- Logical hierarchy (don't skip levels)
- Include keywords naturally
- Descriptive, not generic

### Image SEO

```html
<!-- ❌ Poor image SEO -->
<img src="IMG_12345.jpg" />

<!-- ✅ Optimized image -->
<img
  src="blue-widget-product-photo.webp"
  alt="Blue widget with chrome finish, side view showing control panel"
  width="800"
  height="600"
  loading="lazy"
/>
```

**Image guidelines:**

- Descriptive filenames with keywords
- Alt text describes the image content
- Compressed and properly sized
- WebP/AVIF with fallbacks
- Lazy load below-fold images

### Internal linking

```html
<!-- ❌ Non-descriptive -->
<a href="/products">Click here</a>
<a href="/widgets">Read more</a>

<!-- ✅ Descriptive anchor text -->
<a href="/products/blue-widgets">Browse our blue widget collection</a>
<a href="/guides/widget-maintenance">Learn how to maintain your widgets</a>
```

**Linking guidelines:**

- Descriptive anchor text with keywords
- Link to relevant internal pages
- Reasonable number of links per page
- Fix broken links promptly
- Use breadcrumbs for hierarchy

---

## Structured data (JSON-LD)

### Organization

```html
<script type="application/ld+json">
  {
    "@context": "https://schema.org",
    "@type": "Organization",
    "name": "Example Company",
    "url": "https://example.com",
    "logo": "https://example.com/logo.png",
    "sameAs": ["https://twitter.com/example", "https://linkedin.com/company/example"],
    "contactPoint": {
      "@type": "ContactPoint",
      "telephone": "+1-555-123-4567",
      "contactType": "customer service"
    }
  }
</script>
```

### Article

```html
<script type="application/ld+json">
  {
    "@context": "https://schema.org",
    "@type": "Article",
    "headline": "How to Choose the Right Widget",
    "description": "Complete guide to selecting widgets for your needs.",
    "image": "https://example.com/article-image.jpg",
    "author": {
      "@type": "Person",
      "name": "Jane Smith",
      "url": "https://example.com/authors/jane-smith"
    },
    "publisher": {
      "@type": "Organization",
      "name": "Example Blog",
      "logo": {
        "@type": "ImageObject",
        "url": "https://example.com/logo.png"
      }
    },
    "datePublished": "2024-01-15",
    "dateModified": "2024-01-20"
  }
</script>
```

### Product

```html
<script type="application/ld+json">
  {
    "@context": "https://schema.org",
    "@type": "Product",
    "name": "Blue Widget Pro",
    "image": "https://example.com/blue-widget.jpg",
    "description": "Premium blue widget with advanced features.",
    "brand": {
      "@type": "Brand",
      "name": "WidgetCo"
    },
    "offers": {
      "@type": "Offer",
      "price": "49.99",
      "priceCurrency": "USD",
      "availability": "https://schema.org/InStock",
      "url": "https://example.com/products/blue-widget"
    },
    "aggregateRating": {
      "@type": "AggregateRating",
      "ratingValue": "4.8",
      "reviewCount": "1250"
    }
  }
</script>
```

### FAQ

```html
<script type="application/ld+json">
  {
    "@context": "https://schema.org",
    "@type": "FAQPage",
    "mainEntity": [
      {
        "@type": "Question",
        "name": "What colors are available?",
        "acceptedAnswer": {
          "@type": "Answer",
          "text": "Our widgets come in blue, red, and green."
        }
      },
      {
        "@type": "Question",
        "name": "What is the warranty?",
        "acceptedAnswer": {
          "@type": "Answer",
          "text": "All widgets include a 2-year warranty."
        }
      }
    ]
  }
</script>
```

### Breadcrumbs

```html
<script type="application/ld+json">
  {
    "@context": "https://schema.org",
    "@type": "BreadcrumbList",
    "itemListElement": [
      {
        "@type": "ListItem",
        "position": 1,
        "name": "Home",
        "item": "https://example.com"
      },
      {
        "@type": "ListItem",
        "position": 2,
        "name": "Products",
        "item": "https://example.com/products"
      },
      {
        "@type": "ListItem",
        "position": 3,
        "name": "Blue Widgets",
        "item": "https://example.com/products/blue-widgets"
      }
    ]
  }
</script>
```

### Validation

Test structured data at:

- [Google Rich Results Test](https://search.google.com/test/rich-results)
- [Schema.org Validator](https://validator.schema.org/)

---

## Mobile SEO

### Responsive design

```html
<!-- ❌ Not mobile-friendly -->
<meta name="viewport" content="width=1024" />

<!-- ✅ Responsive viewport -->
<meta name="viewport" content="width=device-width, initial-scale=1" />
```

### Tap targets

```css
/* ❌ Too small for mobile */
.small-link {
  padding: 4px;
  font-size: 12px;
}

/* ✅ Adequate tap target */
.mobile-friendly-link {
  padding: 12px;
  font-size: 16px;
  min-height: 48px;
  min-width: 48px;
}
```

### Font sizes

```css
/* ❌ Too small on mobile */
body {
  font-size: 10px;
}

/* ✅ Readable without zooming */
body {
  font-size: 16px;
  line-height: 1.5;
}
```

---

## International SEO

### Hreflang tags

```html
<!-- For multi-language sites -->
<link rel="alternate" hreflang="en" href="https://example.com/page" />
<link rel="alternate" hreflang="es" href="https://example.com/es/page" />
<link rel="alternate" hreflang="fr" href="https://example.com/fr/page" />
<link rel="alternate" hreflang="x-default" href="https://example.com/page" />
```

### Language declaration

```html
<html lang="en">
  <!-- or -->
  <html lang="es-MX"></html>
</html>
```

---

## SEO audit checklist

### Critical

- [ ] HTTPS enabled
- [ ] robots.txt allows crawling
- [ ] No `noindex` on important pages
- [ ] Title tags present and unique
- [ ] Single `<h1>` per page

### High priority

- [ ] Meta descriptions present
- [ ] Sitemap submitted
- [ ] Canonical URLs set
- [ ] Mobile-responsive
- [ ] Core Web Vitals passing

### Medium priority

- [ ] Structured data implemented
- [ ] Internal linking strategy
- [ ] Image alt text
- [ ] Descriptive URLs
- [ ] Breadcrumb navigation

### Ongoing

- [ ] Fix crawl errors in Search Console
- [ ] Update sitemap when content changes
- [ ] Monitor ranking changes
- [ ] Check for broken links
- [ ] Review Search Console insights

---

## Tools

| Tool                      | Use                           |
| ------------------------- | ----------------------------- |
| Google Search Console     | Monitor indexing, fix issues  |
| Google PageSpeed Insights | Performance + Core Web Vitals |
| Rich Results Test         | Validate structured data      |
| Lighthouse                | Full SEO audit                |
| Screaming Frog            | Crawl analysis                |

## References

- [Google Search Central](https://developers.google.com/search)
- [Schema.org](https://schema.org/)
- [Core Web Vitals](../core-web-vitals/SKILL.md)
- [Web Quality Audit](../web-quality-audit/SKILL.md)

================================================================
File: packages/skills-catalog/skills/(quality)/the-judge/SKILL.md
================================================================
---
name: the-judge
description: Evidence-first pull request judge that reviews a PR and posts one consolidated GitHub review with inline comments via the gh CLI. Runs the repo's own deterministic checks first, researches current official docs before any claim about external libraries or APIs, then reviews correctness, security, structural quality (code judo, spaghetti growth, file-size limits), and AI slop including useless code comments. Every finding must carry evidence, every comment passes a deterministic noise gate before posting, and the verdict (APPROVE, COMMENT, REQUEST_CHANGES) is weighed by findings. Use when asked to review a PR, judge this PR, review this branch or diff before merge, run the-judge, "revise esse PR", "faca o code review", or "julgue esse PR". Do NOT use for reviewing prose or documents, fixing CI failures, resolving merge conflicts, writing the fix itself, or responding to review comments (use gh-address-comments).
license: CC-BY-4.0
metadata:
  author: Felipe Rodrigues - github.com/felipfr
  version: 1.4.0
---

# The Judge

Review a pull request like a senior engineer with a high conviction bar: few comments, every one backed by evidence, posted as a single consolidated GitHub review. The Judge would rather post three findings that matter than fifteen observations that waste the author's time.

## Non-Negotiables

These rules override everything else in this skill. Read them before doing anything.

1. **Evidence or silence.** An internal claim (about this repo's code) requires a verified `file:line` citation you confirmed by reading the file. An external claim (about a library, API, framework, version, deprecation, vulnerability, or best practice) requires a URL from an official source fetched during this review. A finding without evidence is not posted. Period.
2. **Never assert external behavior from memory.** Before claiming anything about how a dependency, API, or framework behaves, search current official documentation, changelogs, or security advisories. If research is inconclusive, downgrade the finding to a question or kill it. Training data is a rumor; the changelog is a source.
3. **Noise budget.** Maximum 5 nit comments inline; overflow becomes a count in the summary. Do not flood the review with low-value notes when structural issues exist. Prefer a small number of high-conviction comments.
4. **Never comment on what the repo's own tooling catches.** Run the repo's linters, type checkers, and focused tests first (Step 1). Anything they flag is out of scope for review comments.
5. **Every comment passes the gate.** All comment bodies and the summary must pass `scripts/review_gate.py` with exit code 0 before posting. No exceptions, no manual overrides.
6. **Language: the user chooses; English is the default.** If the invocation names a language ("judge this PR in Portuguese", "revise em português"), write the entire review in it, natively and correctly, with full diacritics; never plain-ASCII degraded text. Absent an explicit request, write in English. Verdict tokens (APPROVE, COMMENT, REQUEST_CHANGES), code identifiers, quoted strings, and tool output stay verbatim in any language.
7. **Spend tokens where judgment lives.** Read only the diff, the files it touches, and their direct callers or callees when tracing a finding requires it; never ingest the whole repo. Detection a regex can do runs in `scripts/scan_bypasses.py`, not in prose. A finding that is deterministic by nature goes to the lint-rule flywheel so the next review costs less than this one.
8. **The first review is the whole review.** Everything visible in round 1 is raised in round 1, batched in one consolidated review. Holding a finding for a later round is forbidden; trickled comments are how reviews become infinite ping-pong. Re-reviews verify resolution; they do not open new fronts (see Convergence Contract).

## Severity and Verdict

| Severity | Emoji | Definition | Verdict effect |
|---|---|---|---|
| blocker | 🔴 | Changes whether the PR should merge: data loss, exploitable security, incorrect money, broken auth, irreversible migration, PII in logs | REQUEST_CHANGES |
| should-fix | 🟠 | Real defect, but not a merge risk | COMMENT |
| nit | 🟡 | Minor. Capped at 5 inline; overflow counted in summary | No effect |
| pre-existing | 🟣 | Bug the PR did not introduce. Summary only, never inline | No effect |

Verdict mapping: any 🔴 present, REQUEST_CHANGES. Zero 🔴 and zero 🟠, APPROVE. Anything else, COMMENT.

**Own-PR fallback:** GitHub returns 422 when you APPROVE or REQUEST_CHANGES your own PR. `scripts/post_review.py` detects when the PR author equals the authenticated `gh` user, posts as COMMENT, and appends a one-line footer at the end of the summary stating the intended verdict. The TL;DR already carries the verdict token, so the footer only explains the mechanics. Do not fight this; it is API behavior.

## Workflow

### Step 0: Resolve context

```bash
gh pr view --json number,title,body,author,url,baseRefName,headRefName,additions,deletions,changedFiles
gh api user --jq .login
gh pr diff <number>
gh pr view <number> --json files --jq '.files[].path'
```

Classify every changed file as **core** or **mechanical** (generated code, lockfiles, snapshots, vendored deps, build artifacts, migrations output). Mechanical files are skipped and listed in the summary. Detect moved code: 3+ consecutive lines deleted in one place and added identically elsewhere is a move, not new code; do not re-review it as new.

Determine the round. Fetch your own previous reviews on this PR:

```bash
gh api repos/{owner}/{repo}/pulls/{number}/reviews --jq '[.[] | select(.user.login=="<gh-login>") | {id, submitted_at, body}]'
```

No previous review: this is round 1, run the full workflow. Previous review exists: this is round N, load the findings ledger (the ID table) from the latest previous summary and follow the Convergence Contract instead of a full re-run.

If the diff exceeds roughly 400 changed lines in core files and the harness supports subagents, run the Step 3 passes as parallel subagents. Otherwise run them sequentially. Never make subagent support a requirement.

### Step 1: Deterministic ladder

Detect and run the repo's own checks: lint, typecheck, and tests focused on changed files (look at `package.json` scripts, `Makefile`, `pyproject.toml`, CI config). If the repo already runs security scanners (dependency, IaC, or SAST tools wired into its CI), run them or read their current output as deterministic input too. Record results. Findings these tools produce are excluded from your review scope; you judge only what they cannot. If the repo has no such tooling, note that in the summary and move on; do not install anything.

Then run the deterministic bypass scan:

```bash
gh pr diff <number> | python3 scripts/scan_bypasses.py
```

It prints `path:line  category  content` for every bypass marker added by the diff (suppression directives, dodged tests, TLS and type-check bypasses, swallowed errors, sleep-as-synchronization). Scan hits are candidates for Pass F, not findings: a suppression carrying a justification and an issue link is acceptable; a naked one is not. The scan exists so zero LLM tokens are spent detecting what a regex detects.

### Step 2: Mandatory research

Enumerate every external surface the diff touches: dependencies added or version-bumped (read the manifest/lockfile diff), APIs called, framework features used, language features that are version-sensitive. For each surface, search current official documentation, release notes, and security advisories. Log every consulted URL; the summary includes a research log. This step is not optional and not skippable, even when you feel confident. Confidence from memory is exactly the failure mode this step exists to kill. If the diff touches zero external surfaces, state that in the research log.

### Step 3: Review passes

**Completeness contract:** round 1 covers all core files across all passes, in depth, in one shot. Nothing is deferred to "a later look". A finding you could have raised now and raise later is a broken contract with the author. One exception to volume: do not stack comments on code a structural finding will rewrite; if a 🔴 or 🟠 asks for a block to be restructured, withhold nits inside that block and note "nits withheld on lines the structural fix rewrites" in the summary.

**Read `references/review-standards.md` now.** Run six passes over core files:

- **Pass A. Correctness and logic**: broken invariants, unhandled failure paths that lose data, partial state, concrete concurrency hazards.
- **Pass B. Security**: high-confidence exploitability only, newly introduced by this PR, with the hard exclusion and precedent lists applied.
- **Pass C. Structure and maintainability**: the ambitious structural pass. Code judo, file-size limits, spaghetti growth, boundaries, canonical layer.
- **Pass D. AI slop and useless code comments**: comments that restate code, changelog comments, docstring bloat, commented-out code, defensive try/catch on trusted paths, speculative abstractions.
- **Pass E. PR description claims**: every claim of "fixes X" or "improves Y" needs evidence (test, repro, measurement) or becomes a question.
- **Pass F. Bypasses, duplication, and gambiarras**: suppression directives, dodged tests, type and TLS bypasses, copy-paste duplication, magic values, sleep-as-synchronization, unlabeled workarounds. Seeded by the Step 1 bypass scan; every scan hit gets judged here.

Each pass produces candidate findings: claim, tentative severity, evidence pointer. If the harness supports choosing a model per subagent, use light, fast variants for mechanical work (file classification, dedupe, scan triage) and reserve the strongest model for the judgment passes and verification; burning the heavy model on cheap triage is waste, and burning the light model on judgment is false positives.

### Step 4: Verification pass

For each candidate: re-read the actual code at the cited location and confirm the claim holds. Re-apply the exclusion lists. Kill anything you cannot evidence. Deduplicate across passes. Assign final severity conservatively: a blocker you are not certain of is a should-fix phrased as a question. This pass exists because candidate generation is optimized for recall and posting is optimized for precision.

Two grounding rules:

- **Reproduce when feasible.** A 🔴 from Pass A or Pass B that can be demonstrated locally gets the strongest evidence class there is: a failing test or a short script run inside the repo's own test harness (never network attacks, never outside the sandbox of the checkout). Record the command and its output as `repro` evidence. The inverse binds too: when a reproduction was feasible and failed to reproduce the claim, the finding dies, whatever your reading of the code said.
- **Low risk is not false positive.** Severity and validity are orthogonal axes. A real but minor issue is a 🟡, not a discard; killing findings because they are small is how a filter quietly stops detecting real problems. Kill for lack of evidence, downgrade for lack of impact, never conflate the two.

### Step 5: Write comments

**Read `references/comment-voice.md` now.** Write the summary and every comment body under that spec. Produce `findings.json`:

```json
{
  "language": "en",
  "round": 1,
  "carryover": {"blocker": 0, "should-fix": 0},
  "verdict": "REQUEST_CHANGES",
  "summary": "## TL;DR\n...\n## Findings\n...\n## Promote to lint rule\n...\n## Research log\n...\n## Checks run\n...\n## Skipped files\n...",
  "findings": [
    {
      "id": "F1",
      "path": "src/billing/invoice.ts",
      "line": 142,
      "severity": "blocker",
      "body": "🔴 `applyDiscount` divides by `items.length` with no empty-list guard (src/billing/invoice.ts:142)...",
      "evidence": [
        {"type": "internal", "ref": "src/billing/invoice.ts:142"},
        {"type": "external", "ref": "https://official-docs.example/api#behavior"}
      ]
    }
  ]
}
```

`round` defaults to 1. `carryover` counts previous-round findings still unresolved (zeros in round 1); the gate uses it to compute the correct verdict when the new-findings list alone would understate open risk. `id` values are stable across rounds (F1 stays F1 forever) so the resolution ledger maps cleanly. Evidence entries accept three types: `internal` (verified `path:line`), `external` (https URL from an official source fetched this review), and `repro` (the local command that demonstrates the claim, with its observed result in the finding body).

Rules the gate enforces (write to them from the start): body starts with the severity emoji; blocker and should-fix require at least one evidence entry; external evidence must be an https URL; pre-existing findings have no `line`; the summary contains a TL;DR section and a lint-rule section.

### Step 6: Gate

```bash
python3 scripts/review_gate.py findings.json
```

Fix every reported violation and re-run until exit code 0. The gate is deterministic; arguing with it is arguing with a regex.

### Step 7: Post

```bash
python3 scripts/post_review.py findings.json
```

Posts everything as one review (single API call: summary body, verdict event, positioned inline comments), handles the own-PR fallback, prints the review URL. Inline comments only anchor on lines present in the diff; the script tells you which comment failed if GitHub rejects an anchor.

## Summary Body Template

```markdown
## TL;DR
One to three sentences: what the PR does and the verdict with its reason. Must contain the verdict token (APPROVE, COMMENT, or REQUEST_CHANGES) verbatim; the gate checks for it.

## Findings
| ID | Severity | Location | Summary |
|---|---|---|---|

## Resolution (round 2+)
| ID | Status | Note |
|---|---|---|
Every previous finding appears here with status resolved (with the commit), open, or declined-accepted (author's reason held). Omit this section in round 1.

## Promote to lint rule
Findings in this review that are deterministic by nature (banned pattern, naming convention, style rule). For each: the rule, and a one-line sketch of how to lint it. Reviews should get cheaper every cycle; this section is the flywheel. Write "none" if empty.

## Research log
URLs consulted in Step 2, one per line, with the claim each one supports or refutes. Sources that refuted a candidate finding belong here too: a documented non-finding is evidence of diligence and saves a future round. Write "no external surfaces touched" if that is the case.

## Checks run
Deterministic ladder results from Step 1, verbatim numbers with no interpretation: test counts (passed/failed/skipped), linter and typechecker outcome, bypass scan candidate count.

## Skipped files
Mechanical/generated files excluded from review.

## Nit overflow
"N additional nits not posted individually" when the cap was hit. Omit otherwise.
```

## Convergence Contract

The Judge is decisive: it converges reviews instead of stretching them. Round 1 is exhaustive; every later round shrinks. The failure mode this contract kills is the infinite ping-pong where each round discovers a new front.

**Round N (N >= 2) scope is exactly two things:**

1. **Resolution check.** For every finding in the previous ledger, verify against the new commits: resolved (cite the fixing commit), still open, or declined by the author. Report all of them in the Resolution table. This is the focus of the re-review; it is a checklist verification, not a fresh hunt.
2. **New blockers only.** The diff since the last reviewed commit is examined solely for 🔴 introduced by the fix itself. No new 🟠, no new 🟡, no new 🟣, ever. Code that was visible in round 1 and not flagged then is forfeited: the reviewer eats the miss. The single exception is a genuine 🔴 (exploitable security, data loss, money) discovered late, which is always raised, labeled factually as missed in round 1.

**Declined findings.** When the author declines with a reason, first consider that they are closer to the code and may be right; if the reasoning holds from a code-health perspective, record declined-accepted and drop it permanently. If it does not hold, re-argue once with new evidence, at most once. Still unresolved after that: mark it open, state that the disagreement moves out of the review thread, and stop re-raising. Comment threads do not resolve philosophy.

**Round cap.** By round 3, every remaining open finding is either fixed, converted to a follow-up issue by agreement, or escalated out of the thread. The Judge never runs a fourth round on the same finding set.

**Approval bar.** Approve as soon as the PR definitely improves overall code health, even if imperfect; perfect code does not exist, only better code. Unresolved 🟡 never blocks an APPROVE (approve with nits). Withholding approval to extract polish beyond the severity table is scope creep by the reviewer.

The gate enforces the mechanical half of this contract: with `"round": 2` or higher it rejects any non-blocker finding, requires the Resolution section, and requires `carryover` so the verdict reflects still-open previous findings.

## Examples

### Example 1: routine review

User says: "revise esse PR" (on a branch with an open PR, 180 changed lines, TypeScript).
Actions: Steps 0-2 (research: one dep bump, checked its changelog), sequential passes, 1 should-fix + 2 nits survive verification, gate passes on second run (first run caught an exclamation mark), post as COMMENT.
Result: one consolidated review, three inline comments, summary with research log citing the changelog URL.

### Example 2: blocker found

User says: "judge this PR before I merge".
Actions: Pass A finds an unguarded division in a money path with a traced failing input; verification confirms at `src/billing/invoice.ts:142`; verdict REQUEST_CHANGES; PR author equals gh user, so post_review posts as COMMENT with the TL;DR stating REQUEST_CHANGES and a footer explaining the fallback.
Result: review posted, blocker anchored inline with file:line evidence and the lazy fix (guard clause, three lines).

### Example 3: claims without evidence

User says: "review this PR". PR description says "fixes the race condition in the worker pool". No test touches the pool.
Actions: Pass E flags the claim; no repro or test exists; the finding becomes a 🟠 question asking for a failing-then-passing test, not an assertion that the fix is wrong.
Result: comment asks for evidence instead of speculating.

### Example 4: round 2

User says: "julgue de novo" after pushing fixes.
Actions: Step 0 finds the previous review; round 2. Resolution check maps the ledger: F1 fixed in commit `a1b2c3d`, F2 declined with a reason that holds (recorded declined-accepted), F3 still open. Diff since last review scanned for new blockers only; none found. `findings` is empty, `carryover` is `{"blocker": 1, "should-fix": 0}` for F3, verdict REQUEST_CHANGES.
Result: a short review with only the Resolution table and TL;DR. No new comments, no new fronts, convergence preserved.

## Troubleshooting

### Error: HTTP 422 on review submission
Cause: APPROVE/REQUEST_CHANGES on your own PR, or a comment anchored to a line not in the diff.
Solution: `post_review.py` auto-falls back to COMMENT for own PRs. For anchor failures, move the comment to a changed line shown in the patch, or drop `line` and fold it into the summary.

### Error: gh not authenticated
Cause: no `gh auth login` session.
Solution: ask the user to run `gh auth status` and authenticate. Do not attempt tokenless calls.

### Error: no PR for the current branch
Cause: branch never pushed or PR not opened.
Solution: report it and stop. Opening PRs is out of scope for The Judge (that is a different job).

### Gate keeps failing on the same body
Cause: rephrasing around a banned pattern instead of removing it.
Solution: delete the sentence containing the violation. The gate output names the exact pattern and location.

================================================================
File: packages/skills-catalog/skills/(quality)/tlc-generative-engine-optimization/SKILL.md
================================================================
---
name: tlc-generative-engine-optimization
description: "Generative Engine Optimization (GEO) specialist — the technical, on-page publishing work that makes a given page or site discoverable, understandable, trustworthy, quotable, and fresh for AI answer engines (Google AI Overviews, ChatGPT Search, Bing Copilot, Perplexity). Use when asked to 'optimize this page/site for GEO', 'optimize for AI search / answer engines', 'get my page cited by ChatGPT/Perplexity', 'improve AI visibility/citability', 'write an llms.txt', 'add citation-ready structure or schema for AI answers', 'otimizar para busca com IA', or to audit/create/improve a codebase for generative search. Do NOT use for AI-driven SEO content strategy or programmatic pages at scale (use ai-seo), classic keyword/SERP ranking (use seo), accessibility (use web-accessibility), or multi-area site audits (use web-quality-audit)."
metadata:
  version: '1.0.0'
  author: Fernando Paladini - github.com/paladini
license: MIT
---

# GEO Specialist

Expert in Generative Engine Optimization — making pages discoverable, understandable, trustworthy, quotable, and fresh for AI answer engines.

## Philosophy

Treat GEO as **documentation quality, not a trick**. AI engines cite pages they can parse, trust, and quote. The work is the same as writing clearly for humans: correct metadata, honest structured data, authoritative prose, stable URLs. Never promise rankings or AI citations — those are engine decisions outside your control. Do the technical work well; citations follow as a byproduct.

## When to use / not use

**Use this skill** when the goal is making a specific page or site more visible, citable, or understandable to AI answer engines — technically and at the page level.

**Do NOT use** for:

- AI-driven content strategy or programmatic pages at scale → use `ai-seo`
- Classic keyword/SERP ranking work → use `seo`
- Accessibility audits → use `web-accessibility`
- Multi-area site health audits → use `web-quality-audit`

## The Six GEO Pillars

Load `references/pillars-and-workflow.md` for the full deep-dive. Summary:

| #   | Pillar             | Core check                                                                      |
| --- | ------------------ | ------------------------------------------------------------------------------- |
| 1   | **Discoverable**   | robots.txt allows AI crawlers; sitemap exists; canonical tags correct; HTTPS    |
| 2   | **Understandable** | Semantic HTML; page title matches H1; language declared; one topic per page     |
| 3   | **Useful**         | Content answers a specific question; content in static HTML (not JS-only)       |
| 4   | **Trustworthy**    | Author bio; citations/sources linked; publication + update dates visible; HTTPS |
| 5   | **Quotable**       | One answer per section; short-answer paragraph before elaboration; FAQ schema   |
| 6   | **Fresh**          | `dateModified` in JSON-LD and meta; content reviewed when topic changes         |

## Operating Modes

### Mode 1 — Create (new GEO-ready page)

1. Plan page structure: one topic, one H1, question-based H2s/H3s.
2. Apply `templates/page-metadata.html` (canonical, hreflang, meta description).
3. Add `templates/techarticle.jsonld` (or `faqpage.jsonld` for FAQ pages).
4. Write content in the quotable outline pattern (`templates/quotable-article-outline.md`): short direct answer → supporting detail → sources.
5. Update `robots.txt` to allow AI crawlers (`templates/robots-ai-crawlers.txt`).
6. Add or update `llms.txt` if the site wants to guide AI agents (`templates/llms.txt`).
7. Run the GEO page checklist (in `references/pillars-and-workflow.md`).

### Mode 2 — Audit (score an existing page or site)

1. Crawl check: read `robots.txt` — are `OAI-SearchBot` and `BingBot` allowed?
2. Structured data: validate all JSON-LD against the Rich Results Test and Schema Markup Validator.
3. Pillar sweep: for each of the six pillars, mark pass / partial / fail.
4. Produce a **prioritized findings table** (Pillar → Finding → Severity → Fix).
5. Identify quick wins (metadata, schema, robots) vs. content rewrites.

### Mode 3 — Improve (apply fixes)

1. Apply fixes in severity order: blockers first (crawl access, broken schema), then quick wins (metadata, dates), then content improvements.
2. Re-validate structured data after every schema change.
3. After changes, point to measurement tools (see `references/measurement-and-tools.md`) so the user can track AI visibility over time.

## Guardrails

- **Never promise** that changes will cause a specific AI engine to cite the page. Citation is an engine decision.
- **Structured data must match visible page content exactly.** Mismatches violate Google's policies and can suppress the page.
- **`llms.txt` is optional.** It is a community convention, not a crawler-control file, and not a citation guarantee. Recommend it only when the site wants to guide AI agent navigation.
- **`robots.txt` is the only authoritative crawler-control file.** `llms.txt` has no effect on crawling.
- **Do not add `noindex` or `Disallow` for AI crawlers** unless the user explicitly wants to block AI indexing.
- Prefer primary platform documentation (Google Search Central, Bing Webmaster Tools, Schema.org) over third-party summaries.

## Examples

### Example 1 — Audit request

**User:** "Can you audit my blog for AI search visibility?"

**Actions:**

1. Check `robots.txt` → `OAI-SearchBot` is missing a `Disallow` but also missing an explicit `Allow` — confirm default is allow.
2. Validate JSON-LD on the homepage → `datePublished` is missing, `author` has no `url`.
3. Run pillar sweep → Trustworthy: partial (no author bio page); Quotable: fail (no FAQ schema on FAQ page).
4. Return findings table with three priority tiers.

**Result:** Prioritized list: fix `techarticle.jsonld`, add author bio, add `FAQPage` schema. Clear, actionable, no ranking promises.

### Example 2 — Create request

**User:** "Create a new GEO-optimized article page for my Next.js blog."

**Actions:**

1. Draft `<head>` from `templates/page-metadata.html`.
2. Generate `templates/techarticle.jsonld` filled with real title, author, dates.
3. Structure content using `templates/quotable-article-outline.md`: direct-answer intro, H2/H3 sections, sources list.
4. Confirm `robots.txt` allows `OAI-SearchBot`.
5. Run checklist — all eight items pass.

**Result:** Ready-to-deploy page with correct metadata, valid schema, and citation-ready prose.

### Example 3 — llms.txt request

**User:** "Write an llms.txt for my documentation site."

**Actions:**

1. Inventory the three or four most useful pages for an AI agent.
2. Apply `templates/llms.txt` format: H1 site name → blockquote description → `## Key pages` with Markdown links → optional `## Technical files`.
3. Remind the user that `llms.txt` is not a crawler-control file and doesn't guarantee citations.

**Result:** A concise, standards-compliant `llms.txt` with honest caveats.

## Troubleshooting

| Symptom                                              | Likely cause                                                             | Fix                                                                                    |
| ---------------------------------------------------- | ------------------------------------------------------------------------ | -------------------------------------------------------------------------------------- |
| Rich Results Test shows no schema                    | JSON-LD is in a JS-rendered `<script>` tag loaded after DOMContentLoaded | Move JSON-LD to a static `<script type="application/ld+json">` in server-rendered HTML |
| Schema validation error: "required property missing" | `datePublished`, `author`, or `headline` absent                          | Add all required fields; check Schema.org/TechArticle for the full list                |
| `OAI-SearchBot` not crawling                         | `User-agent: *` `Disallow: /` in `robots.txt` blocks all bots            | Add explicit `Allow: /` for `OAI-SearchBot` above the wildcard rule                    |
| `llms.txt` not picked up by agents                   | File not at `https://example.com/llms.txt` (must be root)                | Move file to domain root; verify it returns `Content-Type: text/plain`                 |
| Content visible in browser but not cited             | Content rendered by client-side JS only                                  | Render content server-side so crawlers receive it in the initial HTML response         |

## References and Templates

Load these files on demand — only when the task requires the detail.

| File                                    | Load when                                                                                         |
| --------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `references/pillars-and-workflow.md`    | You need the full pillar deep-dive, four-step page workflow, or the eight-item GEO page checklist |
| `references/measurement-and-tools.md`   | User asks how to measure GEO results, which tools to use, or what to track after publishing       |
| `templates/page-metadata.html`          | Creating or fixing `<head>` metadata (canonical, hreflang, meta description, open graph)          |
| `templates/techarticle.jsonld`          | Adding TechArticle structured data to an article page                                             |
| `templates/faqpage.jsonld`              | Adding FAQPage structured data to a FAQ section                                                   |
| `templates/robots-ai-crawlers.txt`      | Updating `robots.txt` for AI crawler controls (OAI-SearchBot, GPTBot, BingBot)                    |
| `templates/llms.txt`                    | Writing or updating the site's `llms.txt`                                                         |
| `templates/quotable-article-outline.md` | Structuring article content for AI citation                                                       |

================================================================
File: packages/skills-catalog/skills/(quality)/web-accessibility/SKILL.md
================================================================
---
name: accessibility
description: Audit and improve web accessibility following WCAG 2.1 guidelines. Use when asked to "improve accessibility", "a11y audit", "WCAG compliance", "screen reader support", "keyboard navigation", or "make accessible". Do NOT use for SEO (use seo), performance (use core-web-vitals), or comprehensive site audits covering multiple areas (use web-quality-audit).
license: MIT
metadata:
  author: web-quality-skills
  version: '1.0'
---

# Accessibility (a11y)

Comprehensive accessibility guidelines based on WCAG 2.1 and Lighthouse accessibility audits. Goal: make content usable by everyone, including people with disabilities.

## WCAG Principles: POUR

| Principle          | Description                                       |
| ------------------ | ------------------------------------------------- |
| **P**erceivable    | Content can be perceived through different senses |
| **O**perable       | Interface can be operated by all users            |
| **U**nderstandable | Content and interface are understandable          |
| **R**obust         | Content works with assistive technologies         |

## Conformance levels

| Level   | Requirement            | Target                                                |
| ------- | ---------------------- | ----------------------------------------------------- |
| **A**   | Minimum accessibility  | Must pass                                             |
| **AA**  | Standard compliance    | Should pass (legal requirement in many jurisdictions) |
| **AAA** | Enhanced accessibility | Nice to have                                          |

---

## Perceivable

### Text alternatives (1.1)

**Images require alt text:**

```html
<!-- ❌ Missing alt -->
<img src="chart.png" />

<!-- ✅ Descriptive alt -->
<img src="chart.png" alt="Bar chart showing 40% increase in Q3 sales" />

<!-- ✅ Decorative image (empty alt) -->
<img src="decorative-border.png" alt="" role="presentation" />

<!-- ✅ Complex image with longer description -->
<figure>
  <img src="infographic.png" alt="2024 market trends infographic" aria-describedby="infographic-desc" />
  <figcaption id="infographic-desc">
    <!-- Detailed description -->
  </figcaption>
</figure>
```

**Icon buttons need accessible names:**

```html
<!-- ❌ No accessible name -->
<button>
  <svg><!-- menu icon --></svg>
</button>

<!-- ✅ Using aria-label -->
<button aria-label="Open menu">
  <svg aria-hidden="true"><!-- menu icon --></svg>
</button>

<!-- ✅ Using visually hidden text -->
<button>
  <svg aria-hidden="true"><!-- menu icon --></svg>
  <span class="visually-hidden">Open menu</span>
</button>
```

**Visually hidden class:**

```css
.visually-hidden {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}
```

### Color contrast (1.4.3, 1.4.6)

| Text Size                          | AA minimum | AAA enhanced |
| ---------------------------------- | ---------- | ------------ |
| Normal text (< 18px / < 14px bold) | 4.5:1      | 7:1          |
| Large text (≥ 18px / ≥ 14px bold)  | 3:1        | 4.5:1        |
| UI components & graphics           | 3:1        | 3:1          |

```css
/* ❌ Low contrast (2.5:1) */
.low-contrast {
  color: #999;
  background: #fff;
}

/* ✅ Sufficient contrast (7:1) */
.high-contrast {
  color: #333;
  background: #fff;
}

/* ✅ Focus states need contrast too */
:focus-visible {
  outline: 2px solid #005fcc;
  outline-offset: 2px;
}
```

**Don't rely on color alone:**

```html
<!-- ❌ Only color indicates error -->
<input class="error-border" />
<style>
  .error-border {
    border-color: red;
  }
</style>

<!-- ✅ Color + icon + text -->
<div class="field-error">
  <input aria-invalid="true" aria-describedby="email-error" />
  <span id="email-error" class="error-message">
    <svg aria-hidden="true"><!-- error icon --></svg>
    Please enter a valid email address
  </span>
</div>
```

### Media alternatives (1.2)

```html
<!-- Video with captions -->
<video controls>
  <source src="video.mp4" type="video/mp4" />
  <track kind="captions" src="captions.vtt" srclang="en" label="English" default />
  <track kind="descriptions" src="descriptions.vtt" srclang="en" label="Descriptions" />
</video>

<!-- Audio with transcript -->
<audio controls>
  <source src="podcast.mp3" type="audio/mp3" />
</audio>
<details>
  <summary>Transcript</summary>
  <p>Full transcript text...</p>
</details>
```

---

## Operable

### Keyboard accessible (2.1)

**All functionality must be keyboard accessible:**

```javascript
// ❌ Only handles click
element.addEventListener('click', handleAction)

// ✅ Handles both click and keyboard
element.addEventListener('click', handleAction)
element.addEventListener('keydown', (e) => {
  if (e.key === 'Enter' || e.key === ' ') {
    e.preventDefault()
    handleAction()
  }
})
```

**No keyboard traps:**

```javascript
// Modal focus management
function openModal(modal) {
  const focusableElements = modal.querySelectorAll(
    'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
  )
  const firstElement = focusableElements[0]
  const lastElement = focusableElements[focusableElements.length - 1]

  // Trap focus within modal
  modal.addEventListener('keydown', (e) => {
    if (e.key === 'Tab') {
      if (e.shiftKey && document.activeElement === firstElement) {
        e.preventDefault()
        lastElement.focus()
      } else if (!e.shiftKey && document.activeElement === lastElement) {
        e.preventDefault()
        firstElement.focus()
      }
    }
    if (e.key === 'Escape') {
      closeModal()
    }
  })

  firstElement.focus()
}
```

### Focus visible (2.4.7)

```css
/* ❌ Never remove focus outlines */
*:focus {
  outline: none;
}

/* ✅ Use :focus-visible for keyboard-only focus */
:focus {
  outline: none;
}

:focus-visible {
  outline: 2px solid #005fcc;
  outline-offset: 2px;
}

/* ✅ Or custom focus styles */
button:focus-visible {
  box-shadow: 0 0 0 3px rgba(0, 95, 204, 0.5);
}
```

### Skip links (2.4.1)

```html
<body>
  <a href="#main-content" class="skip-link">Skip to main content</a>
  <header><!-- navigation --></header>
  <main id="main-content" tabindex="-1">
    <!-- main content -->
  </main>
</body>
```

```css
.skip-link {
  position: absolute;
  top: -40px;
  left: 0;
  background: #000;
  color: #fff;
  padding: 8px 16px;
  z-index: 100;
}

.skip-link:focus {
  top: 0;
}
```

### Timing (2.2)

```javascript
// Allow users to extend time limits
function showSessionWarning() {
  const modal = createModal({
    title: 'Session Expiring',
    content: 'Your session will expire in 2 minutes.',
    actions: [
      { label: 'Extend session', action: extendSession },
      { label: 'Log out', action: logout },
    ],
    timeout: 120000, // 2 minutes to respond
  })
}
```

### Motion (2.3)

```css
/* Respect reduced motion preference */
@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
    scroll-behavior: auto !important;
  }
}
```

---

## Understandable

### Page language (3.1.1)

```html
<!-- ❌ No language specified -->
<html>
  <!-- ✅ Language specified -->
  <html lang="en">
    <!-- ✅ Language changes within page -->
    <p>The French word for hello is <span lang="fr">bonjour</span>.</p>
  </html>
</html>
```

### Consistent navigation (3.2.3)

```html
<!-- Navigation should be consistent across pages -->
<nav aria-label="Main">
  <ul>
    <li><a href="/" aria-current="page">Home</a></li>
    <li><a href="/products">Products</a></li>
    <li><a href="/about">About</a></li>
  </ul>
</nav>
```

### Form labels (3.3.2)

```html
<!-- ❌ No label association -->
<input type="email" placeholder="Email" />

<!-- ✅ Explicit label -->
<label for="email">Email address</label>
<input type="email" id="email" name="email" autocomplete="email" required />

<!-- ✅ Implicit label -->
<label>
  Email address
  <input type="email" name="email" autocomplete="email" required />
</label>

<!-- ✅ With instructions -->
<label for="password">Password</label>
<input type="password" id="password" aria-describedby="password-requirements" />
<p id="password-requirements">Must be at least 8 characters with one number.</p>
```

### Error handling (3.3.1, 3.3.3)

```html
<!-- Announce errors to screen readers -->
<form novalidate>
  <div class="field" aria-live="polite">
    <label for="email">Email</label>
    <input type="email" id="email" aria-invalid="true" aria-describedby="email-error" />
    <p id="email-error" class="error" role="alert">Please enter a valid email address (e.g., name@example.com)</p>
  </div>
</form>
```

```javascript
// Focus first error on submit
form.addEventListener('submit', (e) => {
  const firstError = form.querySelector('[aria-invalid="true"]')
  if (firstError) {
    e.preventDefault()
    firstError.focus()

    // Announce error summary
    const errorSummary = document.getElementById('error-summary')
    errorSummary.textContent = `${errors.length} errors found. Please fix them and try again.`
    errorSummary.focus()
  }
})
```

---

## Robust

### Valid HTML (4.1.1)

```html
<!-- ❌ Duplicate IDs -->
<div id="content">...</div>
<div id="content">...</div>

<!-- ❌ Invalid nesting -->
<a href="/"><button>Click</button></a>

<!-- ✅ Unique IDs -->
<div id="main-content">...</div>
<div id="sidebar-content">...</div>

<!-- ✅ Proper nesting -->
<a href="/" class="button-link">Click</a>
```

### ARIA usage (4.1.2)

**Prefer native elements:**

```html
<!-- ❌ ARIA role on div -->
<div role="button" tabindex="0">Click me</div>

<!-- ✅ Native button -->
<button>Click me</button>

<!-- ❌ ARIA checkbox -->
<div role="checkbox" aria-checked="false">Option</div>

<!-- ✅ Native checkbox -->
<label><input type="checkbox" /> Option</label>
```

**When ARIA is needed:**

```html
<!-- Custom tabs component -->
<div role="tablist" aria-label="Product information">
  <button role="tab" id="tab-1" aria-selected="true" aria-controls="panel-1">Description</button>
  <button role="tab" id="tab-2" aria-selected="false" aria-controls="panel-2" tabindex="-1">Reviews</button>
</div>
<div role="tabpanel" id="panel-1" aria-labelledby="tab-1">
  <!-- Panel content -->
</div>
<div role="tabpanel" id="panel-2" aria-labelledby="tab-2" hidden>
  <!-- Panel content -->
</div>
```

### Live regions (4.1.3)

```html
<!-- Status updates -->
<div aria-live="polite" aria-atomic="true" class="status">
  <!-- Content updates announced to screen readers -->
</div>

<!-- Urgent alerts -->
<div role="alert" aria-live="assertive">
  <!-- Interrupts current announcement -->
</div>
```

```javascript
// Announce dynamic content changes
function showNotification(message, type = 'polite') {
  const container = document.getElementById(`${type}-announcer`)
  container.textContent = '' // Clear first
  requestAnimationFrame(() => {
    container.textContent = message
  })
}
```

---

## Testing checklist

### Automated testing

```bash
# Lighthouse accessibility audit
npx lighthouse https://example.com --only-categories=accessibility

# axe-core
npm install @axe-core/cli -g
axe https://example.com
```

### Manual testing

- [ ] **Keyboard navigation:** Tab through entire page, use Enter/Space to activate
- [ ] **Screen reader:** Test with VoiceOver (Mac), NVDA (Windows), or TalkBack (Android)
- [ ] **Zoom:** Content usable at 200% zoom
- [ ] **High contrast:** Test with Windows High Contrast Mode
- [ ] **Reduced motion:** Test with `prefers-reduced-motion: reduce`
- [ ] **Focus order:** Logical and follows visual order

### Screen reader commands

| Action        | VoiceOver (Mac)     | NVDA (Windows) |
| ------------- | ------------------- | -------------- |
| Start/Stop    | ⌘ + F5              | Ctrl + Alt + N |
| Next item     | VO + →              | ↓              |
| Previous item | VO + ←              | ↑              |
| Activate      | VO + Space          | Enter          |
| Headings list | VO + U, then arrows | H / Shift + H  |
| Links list    | VO + U              | K / Shift + K  |

---

## Common issues by impact

### Critical (fix immediately)

1. Missing form labels
2. Missing image alt text
3. Insufficient color contrast
4. Keyboard traps
5. No focus indicators

### Serious (fix before launch)

1. Missing page language
2. Missing heading structure
3. Non-descriptive link text
4. Auto-playing media
5. Missing skip links

### Moderate (fix soon)

1. Missing ARIA labels on icons
2. Inconsistent navigation
3. Missing error identification
4. Timing without controls
5. Missing landmark regions

## References

- [WCAG 2.1 Quick Reference](https://www.w3.org/WAI/WCAG21/quickref/)
- [WAI-ARIA Authoring Practices](https://www.w3.org/WAI/ARIA/apg/)
- [Deque axe Rules](https://dequeuniversity.com/rules/axe/)
- [Web Quality Audit](../web-quality-audit/SKILL.md)

================================================================
File: packages/skills-catalog/skills/(quality)/web-best-practices/SKILL.md
================================================================
---
name: best-practices
description: Apply modern web development best practices for security, compatibility, and code quality. Use when asked to "apply best practices", "security audit", "modernize code", "code quality review", or "check for vulnerabilities". Do NOT use for accessibility (use web-accessibility), SEO (use seo), performance (use core-web-vitals), or comprehensive multi-area audits (use web-quality-audit).
license: MIT
metadata:
  author: web-quality-skills
  version: '1.0'
---

# Best practices

Modern web development standards based on Lighthouse best practices audits. Covers security, browser compatibility, and code quality patterns.

## Security

### HTTPS everywhere

**Enforce HTTPS:**

```html
<!-- ❌ Mixed content -->
<img src="http://example.com/image.jpg" />
<script src="http://cdn.example.com/script.js"></script>

<!-- ✅ HTTPS only -->
<img src="https://example.com/image.jpg" />
<script src="https://cdn.example.com/script.js"></script>

<!-- ✅ Protocol-relative (will use page's protocol) -->
<img src="//example.com/image.jpg" />
```

**HSTS Header:**

```
Strict-Transport-Security: max-age=31536000; includeSubDomains; preload
```

### Content Security Policy (CSP)

```html
<!-- Basic CSP via meta tag -->
<meta
  http-equiv="Content-Security-Policy"
  content="default-src 'self'; 
               script-src 'self' https://trusted-cdn.com; 
               style-src 'self' 'unsafe-inline';
               img-src 'self' data: https:;
               connect-src 'self' https://api.example.com;"
/>

<!-- Better: HTTP header -->
```

**CSP Header (recommended):**

```
Content-Security-Policy:
  default-src 'self';
  script-src 'self' 'nonce-abc123' https://trusted.com;
  style-src 'self' 'nonce-abc123';
  img-src 'self' data: https:;
  connect-src 'self' https://api.example.com;
  frame-ancestors 'self';
  base-uri 'self';
  form-action 'self';
```

**Using nonces for inline scripts:**

```html
<script nonce="abc123">
  // This inline script is allowed
</script>
```

### Security headers

```
# Prevent clickjacking
X-Frame-Options: DENY

# Prevent MIME type sniffing
X-Content-Type-Options: nosniff

# Enable XSS filter (legacy browsers)
X-XSS-Protection: 1; mode=block

# Control referrer information
Referrer-Policy: strict-origin-when-cross-origin

# Permissions policy (formerly Feature-Policy)
Permissions-Policy: geolocation=(), microphone=(), camera=()
```

### No vulnerable libraries

```bash
# Check for vulnerabilities
npm audit
yarn audit

# Auto-fix when possible
npm audit fix

# Check specific package
npm ls lodash
```

**Keep dependencies updated:**

```json
// package.json
{
  "scripts": {
    "audit": "npm audit --audit-level=moderate",
    "update": "npm update && npm audit fix"
  }
}
```

**Known vulnerable patterns to avoid:**

```javascript
// ❌ Prototype pollution vulnerable patterns
Object.assign(target, userInput)
_.merge(target, userInput)

// ✅ Safer alternatives
const safeData = JSON.parse(JSON.stringify(userInput))
```

### Input sanitization

```javascript
// ❌ XSS vulnerable
element.innerHTML = userInput
document.write(userInput)

// ✅ Safe text content
element.textContent = userInput

// ✅ If HTML needed, sanitize
import DOMPurify from 'dompurify'
element.innerHTML = DOMPurify.sanitize(userInput)
```

### Secure cookies

```javascript
// ❌ Insecure cookie
document.cookie = "session=abc123";

// ✅ Secure cookie (server-side)
Set-Cookie: session=abc123; Secure; HttpOnly; SameSite=Strict; Path=/
```

---

## Browser compatibility

### Doctype declaration

```html
<!-- ❌ Missing or invalid doctype -->
<html lang="en">
  <head>
    <title>Page</title>
  </head>
  <body></body>
</html>

<!-- ✅ HTML5 doctype -->
<!DOCTYPE html>
<html lang="en">
  <head>
    <title>Page</title>
  </head>
  <body></body>
</html>
```

### Character encoding

```html
<!-- ❌ Missing or late charset -->
<!DOCTYPE html>
<html lang="en">
  <head>
    <title>Page</title>
    <meta charset="UTF-8" />
  </head>
  <body></body>
</html>

<!-- ✅ Charset as first element in head -->
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <title>Page</title>
  </head>
  <body></body>
</html>
```

### Viewport meta tag

```html
<!-- ❌ Missing viewport -->
<head>
  <title>Page</title>
</head>

<!-- ✅ Responsive viewport -->
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Page</title>
</head>
```

### Feature detection

```javascript
// ❌ Browser detection (brittle)
if (navigator.userAgent.includes('Chrome')) {
  // Chrome-specific code
}

// ✅ Feature detection
if ('IntersectionObserver' in window) {
  // Use IntersectionObserver
} else {
  // Fallback
}

// ✅ Using @supports in CSS
@supports (display: grid) {
  .container {
    display: grid;
  }
}

@supports not (display: grid) {
  .container {
    display: flex;
  }
}
```

### Polyfills (when needed)

```html
<!-- Load polyfills conditionally -->
<script>
  if (!('fetch' in window)) {
    document.write('<script src="/polyfills/fetch.js"><\/script>')
  }
</script>

<!-- Or use polyfill.io -->
<script src="https://polyfill.io/v3/polyfill.min.js?features=fetch,IntersectionObserver"></script>
```

---

## Deprecated APIs

### Avoid these

```javascript
// ❌ document.write (blocks parsing)
document.write('<script src="..."></script>');

// ✅ Dynamic script loading
const script = document.createElement('script');
script.src = '...';
document.head.appendChild(script);

// ❌ Synchronous XHR (blocks main thread)
const xhr = new XMLHttpRequest();
xhr.open('GET', url, false); // false = synchronous

// ✅ Async fetch
const response = await fetch(url);

// ❌ Application Cache (deprecated)
<html manifest="cache.manifest">

// ✅ Service Workers
if ('serviceWorker' in navigator) {
  navigator.serviceWorker.register('/sw.js');
}
```

### Event listener passive

```javascript
// ❌ Non-passive touch/wheel (may block scrolling)
element.addEventListener('touchstart', handler)
element.addEventListener('wheel', handler)

// ✅ Passive listeners (allows smooth scrolling)
element.addEventListener('touchstart', handler, { passive: true })
element.addEventListener('wheel', handler, { passive: true })

// ✅ If you need preventDefault, be explicit
element.addEventListener('touchstart', handler, { passive: false })
```

---

## Console & errors

### No console errors

```javascript
// ❌ Errors in production
console.log('Debug info') // Remove in production
throw new Error('Unhandled') // Catch all errors

// ✅ Proper error handling
try {
  riskyOperation()
} catch (error) {
  // Log to error tracking service
  errorTracker.captureException(error)
  // Show user-friendly message
  showErrorMessage('Something went wrong. Please try again.')
}
```

### Error boundaries (React)

```jsx
class ErrorBoundary extends React.Component {
  state = { hasError: false }

  static getDerivedStateFromError(error) {
    return { hasError: true }
  }

  componentDidCatch(error, info) {
    errorTracker.captureException(error, { extra: info })
  }

  render() {
    if (this.state.hasError) {
      return <FallbackUI />
    }
    return this.props.children
  }
}

// Usage
;<ErrorBoundary>
  <App />
</ErrorBoundary>
```

### Global error handler

```javascript
// Catch unhandled errors
window.addEventListener('error', (event) => {
  errorTracker.captureException(event.error)
})

// Catch unhandled promise rejections
window.addEventListener('unhandledrejection', (event) => {
  errorTracker.captureException(event.reason)
})
```

---

## Source maps

### Production configuration

```javascript
// ❌ Source maps exposed in production
// webpack.config.js
module.exports = {
  devtool: 'source-map', // Exposes source code
}

// ✅ Hidden source maps (uploaded to error tracker)
module.exports = {
  devtool: 'hidden-source-map',
}

// ✅ Or no source maps in production
module.exports = {
  devtool: process.env.NODE_ENV === 'production' ? false : 'source-map',
}
```

---

## Performance best practices

### Avoid blocking patterns

```javascript
// ❌ Blocking script
<script src="heavy-library.js"></script>

// ✅ Deferred script
<script defer src="heavy-library.js"></script>

// ❌ Blocking CSS import
@import url('other-styles.css');

// ✅ Link tags (parallel loading)
<link rel="stylesheet" href="styles.css">
<link rel="stylesheet" href="other-styles.css">
```

### Efficient event handlers

```javascript
// ❌ Handler on every element
items.forEach((item) => {
  item.addEventListener('click', handleClick)
})

// ✅ Event delegation
container.addEventListener('click', (e) => {
  if (e.target.matches('.item')) {
    handleClick(e)
  }
})
```

### Memory management

```javascript
// ❌ Memory leak (never removed)
const handler = () => {
  /* ... */
}
window.addEventListener('resize', handler)

// ✅ Cleanup when done
const handler = () => {
  /* ... */
}
window.addEventListener('resize', handler)

// Later, when component unmounts:
window.removeEventListener('resize', handler)

// ✅ Using AbortController
const controller = new AbortController()
window.addEventListener('resize', handler, { signal: controller.signal })

// Cleanup:
controller.abort()
```

---

## Code quality

### Valid HTML

```html
<!-- ❌ Invalid HTML -->
<div id="header">
  <div id="header">
    <!-- Duplicate ID -->
  </div>

  <ul>
    <div>Item</div>
    <!-- Invalid child -->
  </ul>

  <a href="/"><button>Click</button></a>
  <!-- Invalid nesting -->
</div>

<!-- ✅ Valid HTML -->
<header id="site-header"></header>

<ul>
  <li>Item</li>
</ul>

<a href="/" class="button">Click</a>
```

### Semantic HTML

```html
<!-- ❌ Non-semantic -->
<div class="header">
  <div class="nav">
    <div class="nav-item">Home</div>
  </div>
</div>
<div class="main">
  <div class="article">
    <div class="title">Headline</div>
  </div>
</div>

<!-- ✅ Semantic HTML5 -->
<header>
  <nav>
    <a href="/">Home</a>
  </nav>
</header>
<main>
  <article>
    <h1>Headline</h1>
  </article>
</main>
```

### Image aspect ratios

```html
<!-- ❌ Distorted images -->
<img src="photo.jpg" width="300" height="100" />
<!-- If actual ratio is 4:3, this squishes the image -->

<!-- ✅ Preserve aspect ratio -->
<img src="photo.jpg" width="300" height="225" />
<!-- Actual 4:3 dimensions -->

<!-- ✅ CSS object-fit for flexibility -->
<img src="photo.jpg" style="width: 300px; height: 200px; object-fit: cover;" />
```

---

## Permissions & privacy

### Request permissions properly

```javascript
// ❌ Request on page load (bad UX, often denied)
navigator.geolocation.getCurrentPosition(success, error)

// ✅ Request in context, after user action
findNearbyButton.addEventListener('click', async () => {
  // Explain why you need it
  if (await showPermissionExplanation()) {
    navigator.geolocation.getCurrentPosition(success, error)
  }
})
```

### Permissions policy

```html
<!-- Restrict powerful features -->
<meta http-equiv="Permissions-Policy" content="geolocation=(), camera=(), microphone=()" />

<!-- Or allow for specific origins -->
<meta http-equiv="Permissions-Policy" content="geolocation=(self 'https://maps.example.com')" />
```

---

## Audit checklist

### Security (critical)

- [ ] HTTPS enabled, no mixed content
- [ ] No vulnerable dependencies (`npm audit`)
- [ ] CSP headers configured
- [ ] Security headers present
- [ ] No exposed source maps

### Compatibility

- [ ] Valid HTML5 doctype
- [ ] Charset declared first in head
- [ ] Viewport meta tag present
- [ ] No deprecated APIs used
- [ ] Passive event listeners for scroll/touch

### Code quality

- [ ] No console errors
- [ ] Valid HTML (no duplicate IDs)
- [ ] Semantic HTML elements used
- [ ] Proper error handling
- [ ] Memory cleanup in components

### UX

- [ ] No intrusive interstitials
- [ ] Permission requests in context
- [ ] Clear error messages
- [ ] Appropriate image aspect ratios

## Tools

| Tool                                               | Purpose                    |
| -------------------------------------------------- | -------------------------- |
| `npm audit`                                        | Dependency vulnerabilities |
| [SecurityHeaders.com](https://securityheaders.com) | Header analysis            |
| [W3C Validator](https://validator.w3.org)          | HTML validation            |
| Lighthouse                                         | Best practices audit       |
| [Observatory](https://observatory.mozilla.org)     | Security scan              |

## References

- [MDN Web Security](https://developer.mozilla.org/en-US/docs/Web/Security)
- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [Web Quality Audit](../web-quality-audit/SKILL.md)

================================================================
File: packages/skills-catalog/skills/(quality)/web-quality-audit/SKILL.md
================================================================
---
name: web-quality-audit
description: Comprehensive web quality audit covering performance, accessibility, SEO, and best practices in a single review. Use when asked to "audit my site", "review web quality", "run lighthouse audit", "check page quality", or "optimize my website" across multiple areas at once. Orchestrates specialized skills for depth. Do NOT use for single-area audits — prefer core-web-vitals, web-accessibility, seo, or web-best-practices for focused work.
license: MIT
metadata:
  author: web-quality-skills
  version: '1.0'
---

# Web quality audit

Comprehensive quality review based on Google Lighthouse audits. Covers Performance, Accessibility, SEO, and Best Practices across 150+ checks.

## How it works

1. Analyze the provided code/project for quality issues
2. Categorize findings by severity (Critical, High, Medium, Low)
3. Provide specific, actionable recommendations
4. Include code examples for fixes

## Audit categories

### Performance (40% of typical issues)

**Core Web Vitals** — Must pass for good page experience:

- **LCP (Largest Contentful Paint) < 2.5s.** The largest visible element must render quickly. Optimize images, fonts, and server response time.
- **INP (Interaction to Next Paint) < 200ms.** User interactions must feel instant. Reduce JavaScript execution time and break up long tasks.
- **CLS (Cumulative Layout Shift) < 0.1.** Content must not jump around. Set explicit dimensions on images, embeds, and ads.

**Resource Optimization:**

- **Compress images.** Use WebP/AVIF with fallbacks. Serve correctly sized images via `srcset`.
- **Minimize JavaScript.** Remove unused code. Use code splitting. Defer non-critical scripts.
- **Optimize CSS.** Extract critical CSS. Remove unused styles. Avoid `@import`.
- **Efficient fonts.** Use `font-display: swap`. Preload critical fonts. Subset to needed characters.

**Loading Strategy:**

- **Preconnect to origins.** Add `<link rel="preconnect">` for third-party domains.
- **Preload critical assets.** LCP images, fonts, and above-fold CSS.
- **Lazy load below-fold content.** Images, iframes, and heavy components.
- **Cache effectively.** Long cache TTLs for static assets. Immutable caching for hashed files.

### Accessibility (30% of typical issues)

**Perceivable:**

- **Text alternatives.** Every `<img>` has meaningful `alt` text. Decorative images use `alt=""`.
- **Color contrast.** Minimum 4.5:1 for normal text, 3:1 for large text (WCAG AA).
- **Don't rely on color alone.** Use icons, patterns, or text alongside color indicators.
- **Captions and transcripts.** Video has captions. Audio has transcripts.

**Operable:**

- **Keyboard accessible.** All functionality available via keyboard. No keyboard traps.
- **Focus visible.** Clear focus indicators on all interactive elements.
- **Skip links.** Provide "Skip to main content" for keyboard users.
- **Sufficient time.** Users can extend time limits. No auto-advancing content without controls.

**Understandable:**

- **Page language.** Set `lang` attribute on `<html>`.
- **Consistent navigation.** Same navigation structure across pages.
- **Error identification.** Form errors clearly described and associated with fields.
- **Labels and instructions.** All form inputs have associated labels.

**Robust:**

- **Valid HTML.** No duplicate IDs. Properly nested elements.
- **ARIA used correctly.** Prefer native elements. ARIA roles match behavior.
- **Name, role, value.** Interactive elements have accessible names and correct roles.

### SEO (15% of typical issues)

**Crawlability:**

- **Valid robots.txt.** Doesn't block important resources.
- **XML sitemap.** Lists all important pages. Submitted to Search Console.
- **Canonical URLs.** Prevent duplicate content issues.
- **No noindex on important pages.** Check meta robots and headers.

**On-Page SEO:**

- **Unique title tags.** 50-60 characters. Primary keyword included.
- **Meta descriptions.** 150-160 characters. Compelling and unique.
- **Heading hierarchy.** Single `<h1>`. Logical heading structure.
- **Descriptive link text.** Not "click here" or "read more".

**Technical SEO:**

- **Mobile-friendly.** Responsive design. Tap targets ≥ 48px.
- **HTTPS.** Secure connection required.
- **Fast loading.** Performance directly impacts ranking.
- **Structured data.** JSON-LD for rich snippets (Article, Product, FAQ, etc.).

### Best practices (15% of typical issues)

**Security:**

- **HTTPS everywhere.** No mixed content. HSTS enabled.
- **No vulnerable libraries.** Keep dependencies updated.
- **CSP headers.** Content Security Policy to prevent XSS.
- **No exposed source maps.** In production builds.

**Modern Standards:**

- **No deprecated APIs.** Replace `document.write`, synchronous XHR, etc.
- **Valid doctype.** Use `<!DOCTYPE html>`.
- **Charset declared.** `<meta charset="UTF-8">` as first element in `<head>`.
- **No browser errors.** Clean console. No CORS issues.

**UX Patterns:**

- **No intrusive interstitials.** Especially on mobile.
- **Clear permission requests.** Only ask when needed, with context.
- **No misleading buttons.** Buttons do what they say.

## Severity levels

| Level        | Description                                   | Action              |
| ------------ | --------------------------------------------- | ------------------- |
| **Critical** | Security vulnerabilities, complete failures   | Fix immediately     |
| **High**     | Core Web Vitals failures, major a11y barriers | Fix before launch   |
| **Medium**   | Performance opportunities, SEO improvements   | Fix within sprint   |
| **Low**      | Minor optimizations, code quality             | Fix when convenient |

## Audit output format

When performing an audit, structure findings as:

```markdown
## Audit results

### Critical issues (X found)

- **[Category]** Issue description. File: `path/to/file.js:123`
  - **Impact:** Why this matters
  - **Fix:** Specific code change or recommendation

### High priority (X found)

...

### Summary

- Performance: X issues (Y critical)
- Accessibility: X issues (Y critical)
- SEO: X issues
- Best Practices: X issues

### Recommended priority

1. First fix this because...
2. Then address...
3. Finally optimize...
```

## Quick checklist

### Before every deploy

- [ ] Core Web Vitals passing
- [ ] No accessibility errors (axe/Lighthouse)
- [ ] No console errors
- [ ] HTTPS working
- [ ] Meta tags present

### Weekly review

- [ ] Check Search Console for issues
- [ ] Review Core Web Vitals trends
- [ ] Update dependencies
- [ ] Test with screen reader

### Monthly deep dive

- [ ] Full Lighthouse audit
- [ ] Performance profiling
- [ ] Accessibility audit with real users
- [ ] SEO keyword review

## References

For detailed guidelines on specific areas:

- [Performance Optimization](../performance/SKILL.md)
- [Core Web Vitals](../core-web-vitals/SKILL.md)
- [Accessibility](../accessibility/SKILL.md)
- [SEO](../seo/SKILL.md)
- [Best Practices](../best-practices/SKILL.md)
