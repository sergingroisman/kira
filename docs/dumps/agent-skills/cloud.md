---
type: Dump
title: gitingest — tech-leads-club/agent-skills
- **Fonte**: https://github.com/tech-leads-club/agent-skills @ commit e74dc8a
- **Licença**: código MIT (Copyright (c) 2026 Tech Leads Club); conteúdo dos SKILL.md
  CC-BY-4.0 — atribuição: Tech Leads Club + autor indicado em `metadata.author` de cada skill.
- **Gerado por**: `docs/dumps/generate-agent-skills-dump.ps1` — caminhada ordenada por caminho;
  mesma entrada = mesmo dump. Fora de escopo: `references/`, `templates/`, código CLI/MCP.
---

# gitingest — agent-skills — categoria (cloud) (5 skills)

================================================================
File: packages/skills-catalog/skills/(cloud)/aws-advisor/SKILL.md
================================================================
---
name: aws-advisor
description: Expert AWS Cloud Advisor for architecture design, security review, and implementation guidance. Leverages AWS MCP tools for accurate, documentation-backed answers. Use when user asks about AWS architecture, security, service selection, migrations, troubleshooting, or learning AWS. Triggers on AWS, Lambda, S3, EC2, ECS, EKS, DynamoDB, RDS, CloudFormation, CDK, Terraform, Serverless, SAM, IAM, VPC, API Gateway, or any AWS service. Do NOT use for non-AWS cloud providers or general infrastructure without AWS context.
license: CC-BY-4.0
metadata:
  author: Felipe Rodrigues - github.com/felipfr
  version: '1.0.0'
---

# AWS Advisor

Expert AWS consulting with accuracy-first approach using MCP tools.

## Core Principles

1. **Search Before Answer**: Always use MCP tools to verify information
2. **No Guessing**: Uncertain? Search documentation first
3. **Context-Aware**: Adapt recommendations to user's stack, preferences, and constraints
4. **Security by Default**: Every recommendation considers security
5. **No Lock-in**: Present multiple options with trade-offs, let user decide

## Adaptive Behavior

**Before recommending tools/frameworks**, understand the context:

- What's the user's current stack? (ask if unclear)
- What's the team's expertise?
- Is there an existing IaC in the project?
- Speed vs control trade-off preference?

**IaC Selection** - Don't default to one, guide by context:

| Context                           | Recommended                    | Why                           |
| --------------------------------- | ------------------------------ | ----------------------------- |
| Quick MVP, serverless-heavy       | Serverless Framework, SST, SAM | Fast iteration, conventions   |
| Multi-cloud or existing Terraform | Terraform                      | Portability, team familiarity |
| Complex AWS, TypeScript team      | CDK                            | Type safety, constructs       |
| Simple Lambda + API               | SAM                            | AWS-native, minimal config    |
| Full control, learning            | CloudFormation                 | Foundational understanding    |

**Language/Runtime** - Match user's preference:

- Ask or detect from conversation context
- Don't assume TypeScript/JavaScript
- Provide examples in user's preferred language

## MCP Tools Available

### AWS Knowledge MCP

| Tool                              | Use For                              |
| --------------------------------- | ------------------------------------ |
| `aws___search_documentation`      | Any AWS question - search first!     |
| `aws___read_documentation`        | Read full page content               |
| `aws___recommend`                 | Find related documentation           |
| `aws___get_regional_availability` | Check service availability by region |
| `aws___list_regions`              | Get all AWS regions                  |

### AWS Marketplace MCP

| Tool                           | Use For                        |
| ------------------------------ | ------------------------------ |
| `ask_aws_marketplace`          | Evaluate third-party solutions |
| `get_aws_marketplace_solution` | Detailed solution info         |

## Search Topic Selection

**Critical**: Choose the right topic for efficient searches.

| Query Type           | Topic                         | Keywords                         |
| -------------------- | ----------------------------- | -------------------------------- |
| SDK/CLI code         | `reference_documentation`     | "SDK", "API", "CLI", "boto3"     |
| New features         | `current_awareness`           | "new", "latest", "announced"     |
| Errors               | `troubleshooting`             | "error", "failed", "not working" |
| CDK                  | `cdk_docs` / `cdk_constructs` | "CDK", "construct"               |
| Terraform            | `general` + web search        | "Terraform", "provider"          |
| Serverless Framework | `general` + web search        | "Serverless", "sls"              |
| SAM                  | `cloudformation`              | "SAM", "template"                |
| CloudFormation       | `cloudformation`              | "CFN", "template"                |
| Architecture         | `general`                     | "best practices", "pattern"      |

## Workflows

### Standard Question Flow

```
1. Parse question → Identify AWS services involved
2. Search documentation → aws___search_documentation with right topic
3. Read if needed → aws___read_documentation for details
4. Verify regional → aws___get_regional_availability if relevant
5. Respond with code examples
```

### Architecture Review Flow

```
1. Gather requirements (functional, non-functional, constraints)
2. Search relevant patterns → topic: general
3. Run: scripts/well_architected_review.py → generates review questions
4. Discuss trade-offs with user
5. Run: scripts/generate_diagram.py → visualize architecture
```

### Security Review Flow

```
1. Understand architecture scope
2. Run: scripts/security_review.py → generates checklist
3. Search security docs → topic: general, query: "[service] security"
4. Provide specific recommendations with IAM policies, SG rules
```

## Reference Files

Load only when needed:

| File                                              | Load When                             |
| ------------------------------------------------- | ------------------------------------- |
| [mcp-guide.md](references/mcp-guide.md)           | Optimizing MCP usage, complex queries |
| [decision-trees.md](references/decision-trees.md) | Service selection questions           |
| [checklists.md](references/checklists.md)         | Reviews, validations, discovery       |

## Scripts

Run scripts for structured outputs (code never enters context):

| Script                               | Purpose                              |
| ------------------------------------ | ------------------------------------ |
| `scripts/well_architected_review.py` | Generate W-A review questions        |
| `scripts/security_review.py`         | Generate security checklist          |
| `scripts/generate_diagram.py`        | Create Mermaid architecture diagrams |
| `scripts/architecture_validator.py`  | Validate architecture description    |
| `scripts/cost_considerations.py`     | List cost factors to evaluate        |

## Code Examples

**Always ask or detect user's preference before providing code:**

1. **Language**: Python, TypeScript, JavaScript, Go, Java, etc.
2. **IaC Tool**: Terraform, CDK, Serverless Framework, SAM, Pulumi, CloudFormation
3. **Framework**: If applicable (Express, FastAPI, NestJS, etc.)

**When preference is unknown**, ask:

> "What's your preferred language and IaC tool? (e.g., Python + Terraform, TypeScript + CDK, Node + Serverless Framework)"

**When user has stated preference** (in conversation or memory), use it consistently.

### Quick Reference for IaC Examples

**Terraform** - Search web for latest provider syntax:

```hcl
resource "aws_lambda_function" "example" {
  filename         = "lambda.zip"
  function_name    = "example"
  role            = aws_iam_role.lambda.arn
  handler         = "index.handler"
  runtime         = "nodejs20.x"
}
```

**Serverless Framework** - Great for rapid serverless development:

```yaml
service: my-service
provider:
  name: aws
  runtime: nodejs20.x
functions:
  hello:
    handler: handler.hello
    events:
      - httpApi:
          path: /hello
          method: get
```

**SAM** - AWS native, good for Lambda-focused apps:

```yaml
AWSTemplateFormatVersion: '2010-09-09'
Transform: AWS::Serverless-2016-10-31
Resources:
  HelloFunction:
    Type: AWS::Serverless::Function
    Properties:
      Handler: index.handler
      Runtime: nodejs20.x
      Events:
        Api:
          Type: HttpApi
```

**CDK** - Best for complex infra with programming language benefits:

```typescript
new lambda.Function(this, 'Handler', {
  runtime: lambda.Runtime.NODEJS_20_X,
  handler: 'index.handler',
  code: lambda.Code.fromAsset('lambda'),
})
```

## Response Style

1. **Direct answer first**, explanation after
2. **Working code** over pseudocode
3. **Trade-offs** for architectural decisions
4. **Cost awareness** - mention pricing implications
5. **Security callouts** when relevant

================================================================
File: packages/skills-catalog/skills/(cloud)/cloudflare-deploy/SKILL.md
================================================================
---
name: cloudflare-deploy
description: Deploy applications and infrastructure to Cloudflare using Workers, Pages, and related platform services. Use when the user asks to deploy, host, publish, or set up a project on Cloudflare. Do NOT use for deploying to Vercel, Netlify, or Render (use their respective skills).
metadata:
  author: github.com/openai/skills
  version: '1.0.0'
---

# Cloudflare Deploy

Consolidated skill for building on the Cloudflare platform. Use decision trees below to find the right product, then load detailed references.

## Prerequisites

- When sandboxing blocks the deployment network calls, rerun with `sandbox_permissions=require_escalated`.
- The deployment might take a few minutes. Use appropriate timeout values.

## Authentication (Required Before Deploy)

Verify auth before `wrangler deploy`, `wrangler pages deploy`, or `npm run deploy`:

```bash
npx wrangler whoami    # Shows account if authenticated
```

Not authenticated? → `references/wrangler/auth.md`

- Interactive/local: `wrangler login` (one-time OAuth)
- CI/CD: Set `CLOUDFLARE_API_TOKEN` env var

## Quick Decision Trees

### "I need to run code"

```
Need to run code?
├─ Serverless functions at the edge → workers/
├─ Full-stack web app with Git deploys → pages/
├─ Stateful coordination/real-time → durable-objects/
├─ Long-running multi-step jobs → workflows/
├─ Run containers → containers/
├─ Multi-tenant (customers deploy code) → workers-for-platforms/
├─ Scheduled tasks (cron) → cron-triggers/
├─ Lightweight edge logic (modify HTTP) → snippets/
├─ Process Worker execution events (logs/observability) → tail-workers/
└─ Optimize latency to backend infrastructure → smart-placement/
```

### "I need to store data"

```
Need storage?
├─ Key-value (config, sessions, cache) → kv/
├─ Relational SQL → d1/ (SQLite) or hyperdrive/ (existing Postgres/MySQL)
├─ Object/file storage (S3-compatible) → r2/
├─ Message queue (async processing) → queues/
├─ Vector embeddings (AI/semantic search) → vectorize/
├─ Strongly-consistent per-entity state → durable-objects/ (DO storage)
├─ Secrets management → secrets-store/
├─ Streaming ETL to R2 → pipelines/
└─ Persistent cache (long-term retention) → cache-reserve/
```

### "I need AI/ML"

```
Need AI?
├─ Run inference (LLMs, embeddings, images) → workers-ai/
├─ Vector database for RAG/search → vectorize/
├─ Build stateful AI agents → agents-sdk/
├─ Gateway for any AI provider (caching, routing) → ai-gateway/
└─ AI-powered search widget → ai-search/
```

### "I need networking/connectivity"

```
Need networking?
├─ Expose local service to internet → tunnel/
├─ TCP/UDP proxy (non-HTTP) → spectrum/
├─ WebRTC TURN server → turn/
├─ Private network connectivity → network-interconnect/
├─ Optimize routing → argo-smart-routing/
├─ Optimize latency to backend (not user) → smart-placement/
└─ Real-time video/audio → realtimekit/ or realtime-sfu/
```

### "I need security"

```
Need security?
├─ Web Application Firewall → waf/
├─ DDoS protection → ddos/
├─ Bot detection/management → bot-management/
├─ API protection → api-shield/
├─ CAPTCHA alternative → turnstile/
└─ Credential leak detection → waf/ (managed ruleset)
```

### "I need media/content"

```
Need media?
├─ Image optimization/transformation → images/
├─ Video streaming/encoding → stream/
├─ Browser automation/screenshots → browser-rendering/
└─ Third-party script management → zaraz/
```

### "I need infrastructure-as-code"

```
Need IaC? → pulumi/ (Pulumi), terraform/ (Terraform), or api/ (REST API)
```

## Product Index

### Compute & Runtime

| Product               | Reference                           |
| --------------------- | ----------------------------------- |
| Workers               | `references/workers/`               |
| Pages                 | `references/pages/`                 |
| Pages Functions       | `references/pages-functions/`       |
| Durable Objects       | `references/durable-objects/`       |
| Workflows             | `references/workflows/`             |
| Containers            | `references/containers/`            |
| Workers for Platforms | `references/workers-for-platforms/` |
| Cron Triggers         | `references/cron-triggers/`         |
| Tail Workers          | `references/tail-workers/`          |
| Snippets              | `references/snippets/`              |
| Smart Placement       | `references/smart-placement/`       |

### Storage & Data

| Product         | Reference                     |
| --------------- | ----------------------------- |
| KV              | `references/kv/`              |
| D1              | `references/d1/`              |
| R2              | `references/r2/`              |
| Queues          | `references/queues/`          |
| Hyperdrive      | `references/hyperdrive/`      |
| DO Storage      | `references/do-storage/`      |
| Secrets Store   | `references/secrets-store/`   |
| Pipelines       | `references/pipelines/`       |
| R2 Data Catalog | `references/r2-data-catalog/` |
| R2 SQL          | `references/r2-sql/`          |

### AI & Machine Learning

| Product    | Reference                |
| ---------- | ------------------------ |
| Workers AI | `references/workers-ai/` |
| Vectorize  | `references/vectorize/`  |
| Agents SDK | `references/agents-sdk/` |
| AI Gateway | `references/ai-gateway/` |
| AI Search  | `references/ai-search/`  |

### Networking & Connectivity

| Product              | Reference                          |
| -------------------- | ---------------------------------- |
| Tunnel               | `references/tunnel/`               |
| Spectrum             | `references/spectrum/`             |
| TURN                 | `references/turn/`                 |
| Network Interconnect | `references/network-interconnect/` |
| Argo Smart Routing   | `references/argo-smart-routing/`   |
| Workers VPC          | `references/workers-vpc/`          |

### Security

| Product         | Reference                    |
| --------------- | ---------------------------- |
| WAF             | `references/waf/`            |
| DDoS Protection | `references/ddos/`           |
| Bot Management  | `references/bot-management/` |
| API Shield      | `references/api-shield/`     |
| Turnstile       | `references/turnstile/`      |

### Media & Content

| Product           | Reference                       |
| ----------------- | ------------------------------- |
| Images            | `references/images/`            |
| Stream            | `references/stream/`            |
| Browser Rendering | `references/browser-rendering/` |
| Zaraz             | `references/zaraz/`             |

### Real-Time Communication

| Product      | Reference                  |
| ------------ | -------------------------- |
| RealtimeKit  | `references/realtimekit/`  |
| Realtime SFU | `references/realtime-sfu/` |

### Developer Tools

| Product            | Reference                        |
| ------------------ | -------------------------------- |
| Wrangler           | `references/wrangler/`           |
| Miniflare          | `references/miniflare/`          |
| C3                 | `references/c3/`                 |
| Observability      | `references/observability/`      |
| Analytics Engine   | `references/analytics-engine/`   |
| Web Analytics      | `references/web-analytics/`      |
| Sandbox            | `references/sandbox/`            |
| Workerd            | `references/workerd/`            |
| Workers Playground | `references/workers-playground/` |

### Infrastructure as Code

| Product   | Reference               |
| --------- | ----------------------- |
| Pulumi    | `references/pulumi/`    |
| Terraform | `references/terraform/` |
| API       | `references/api/`       |

### Other Services

| Product       | Reference                   |
| ------------- | --------------------------- |
| Email Routing | `references/email-routing/` |
| Email Workers | `references/email-workers/` |
| Static Assets | `references/static-assets/` |
| Bindings      | `references/bindings/`      |
| Cache Reserve | `references/cache-reserve/` |

## Troubleshooting

### Escalated Network Access

If deployment fails due to network issues (timeouts, DNS errors, connection resets), rerun the deploy with escalated permissions (use `sandbox_permissions=require_escalated`). The deploy requires escalated network access when sandbox networking blocks outbound requests.

Example guidance to the user:

```
The deploy needs escalated network access to deploy to Cloudflare. I can rerun the command with escalated permissions—want me to proceed?
```

================================================================
File: packages/skills-catalog/skills/(cloud)/netlify-deploy/SKILL.md
================================================================
---
name: netlify-deploy
description: Deploy web projects to Netlify using the Netlify CLI (`npx netlify`). Use when the user asks to deploy, host, publish, or link a site/repo on Netlify, including preview and production deploys. Do NOT use for deploying to Vercel, Cloudflare, or Render (use their respective skills).
metadata:
  author: github.com/openai/skills
  version: '1.0.0'
---

# Netlify Deployment Skill

Deploy web projects to Netlify using the Netlify CLI with intelligent detection of project configuration and deployment context.

## Overview

This skill automates Netlify deployments by:

- Verifying Netlify CLI authentication
- Detecting project configuration and framework
- Linking to existing sites or creating new ones
- Deploying to production or preview environments

## Prerequisites

- **Netlify CLI**: Installed via npx (no global install required)
- **Authentication**: Netlify account with active login session
- **Project**: Valid web project in current directory
- When sandboxing blocks the deployment network calls, rerun with `sandbox_permissions=require_escalated`.
- The deployment might take a few minutes. Use appropriate timeout values.

## Authentication Pattern

The skill uses the **pre-authenticated Netlify CLI** approach:

1. Check authentication status with `npx netlify status`
2. If not authenticated, guide user through `npx netlify login`
3. Fail gracefully if authentication cannot be established

Authentication uses either:

- **Browser-based OAuth** (primary): `netlify login` opens browser for authentication
- **API Key** (alternative): Set `NETLIFY_AUTH_TOKEN` environment variable

## Workflow

### 1. Verify Netlify CLI Authentication

Check if the user is logged into Netlify:

```bash
npx netlify status
```

**Expected output patterns**:

- ✅ Authenticated: Shows logged-in user email and site link status
- ❌ Not authenticated: "Not logged into any site" or authentication error

**If not authenticated**, guide the user:

```bash
npx netlify login
```

This opens a browser window for OAuth authentication. Wait for user to complete login, then verify with `netlify status` again.

**Alternative: API Key authentication**

If browser authentication isn't available, users can set:

```bash
export NETLIFY_AUTH_TOKEN=your_token_here
```

Tokens can be generated at: https://app.netlify.com/user/applications#personal-access-tokens

### 2. Detect Site Link Status

From `netlify status` output, determine:

- **Linked**: Site already connected to Netlify (shows site name/URL)
- **Not linked**: Need to link or create site

### 3. Link to Existing Site or Create New

**If already linked** → Skip to step 4

**If not linked**, attempt to link by Git remote:

```bash
# Check if project is Git-based
git remote show origin

# If Git-based, extract remote URL
# Format: https://github.com/username/repo or git@github.com:username/repo.git

# Try to link by Git remote
npx netlify link --git-remote-url <REMOTE_URL>
```

**If link fails** (site doesn't exist on Netlify):

```bash
# Create new site interactively
npx netlify init
```

This guides user through:

1. Choosing team/account
2. Setting site name
3. Configuring build settings
4. Creating netlify.toml if needed

### 4. Verify Dependencies

Before deploying, ensure project dependencies are installed:

```bash
# For npm projects
npm install

# For other package managers, detect and use appropriate command
# yarn install, pnpm install, etc.
```

### 5. Deploy to Netlify

Choose deployment type based on context:

**Preview/Draft Deploy** (default for existing sites):

```bash
npx netlify deploy
```

This creates a deploy preview with a unique URL for testing.

**Production Deploy** (for new sites or explicit production deployments):

```bash
npx netlify deploy --prod
```

This deploys to the live production URL.

**Deployment process**:

1. CLI detects build settings (from netlify.toml or prompts user)
2. Builds the project locally
3. Uploads built assets to Netlify
4. Returns deployment URL

### 6. Report Results

After deployment, report to user:

- **Deploy URL**: Unique URL for this deployment
- **Site URL**: Production URL (if production deploy)
- **Deploy logs**: Link to Netlify dashboard for logs
- **Next steps**: Suggest `netlify open` to view site or dashboard

## Handling netlify.toml

If a `netlify.toml` file exists, the CLI uses it automatically. If not, the CLI will prompt for:

- **Build command**: e.g., `npm run build`, `next build`
- **Publish directory**: e.g., `dist`, `build`, `.next`

Common framework defaults:

- **Next.js**: build command `npm run build`, publish `.next`
- **React (Vite)**: build command `npm run build`, publish `dist`
- **Static HTML**: no build command, publish current directory

The skill should detect framework from `package.json` if possible and suggest appropriate settings.

## Example Full Workflow

```bash
# 1. Check authentication
npx netlify status

# If not authenticated:
npx netlify login

# 2. Link site (if needed)
# Try Git-based linking first
git remote show origin
npx netlify link --git-remote-url https://github.com/user/repo

# If no site exists, create new one:
npx netlify init

# 3. Install dependencies
npm install

# 4. Deploy (preview for testing)
npx netlify deploy

# 5. Deploy to production (when ready)
npx netlify deploy --prod
```

## Error Handling

Common issues and solutions:

**"Not logged in"**
→ Run `npx netlify login`

**"No site linked"**
→ Run `npx netlify link` or `npx netlify init`

**"Build failed"**
→ Check build command and publish directory in netlify.toml or CLI prompts
→ Verify dependencies are installed
→ Review build logs for specific errors

**"Publish directory not found"**
→ Verify build command ran successfully
→ Check publish directory path is correct

## Troubleshooting

### Escalated Network Access

If deployment fails due to network issues (timeouts, DNS errors, connection resets), rerun the deploy with escalated permissions (use `sandbox_permissions=require_escalated`). The deploy requires escalated network access when sandbox networking blocks outbound requests.

Example guidance to the user:

```
The deploy needs escalated network access to deploy to Netlify. I can rerun the command with escalated permissions—want me to proceed?
```

## Environment Variables

For secrets and configuration:

1. Never commit secrets to Git
2. Set in Netlify dashboard: Site Settings → Environment Variables
3. Access in builds via `process.env.VARIABLE_NAME`

## Tips

- Use `netlify deploy` (no `--prod`) first to test before production
- Run `netlify open` to view site in Netlify dashboard
- Run `netlify logs` to view function logs (if using Netlify Functions)
- Use `netlify dev` for local development with Netlify Functions

## Reference

- Netlify CLI Docs: https://docs.netlify.com/cli/get-started/
- netlify.toml Reference: https://docs.netlify.com/configure-builds/file-based-configuration/

## Bundled References (Load As Needed)

- [CLI commands](references/cli-commands.md)
- [Deployment patterns](references/deployment-patterns.md)
- [netlify.toml guide](references/netlify-toml.md)

================================================================
File: packages/skills-catalog/skills/(cloud)/render-deploy/SKILL.md
================================================================
---
name: render-deploy
description: Deploy applications to Render by analyzing codebases, generating render.yaml Blueprints, and providing Dashboard deeplinks. Use when the user wants to deploy, host, publish, or set up their application on Render's cloud platform. Do NOT use for deploying to Vercel, Netlify, or Cloudflare (use their respective skills).
metadata:
  author: github.com/openai/skills
  version: '1.0.0'
---

# Deploy to Render

Render supports **Git-backed** services and **prebuilt Docker image** services.

This skill covers **Git-backed** flows:

1. **Blueprint Method** - Generate render.yaml for Infrastructure-as-Code deployments
2. **Direct Creation** - Create services instantly via MCP tools

Blueprints can also run a **prebuilt Docker image** by using `runtime: image`, but the `render.yaml` still must live in a Git repo.

If there is no Git remote, stop and ask the user to either:

- Create/push a Git remote (can be minimal if only the Blueprint is needed), or
- Use the Render Dashboard/API to deploy a prebuilt Docker image (MCP cannot create image-backed services).

## Prerequisites

- When sandboxing blocks the deployment network calls, rerun with `sandbox_permissions=require_escalated`.
- The deployment might take a few minutes. Use appropriate timeout values.

## Security Requirements for API Keys and Secrets

**CRITICAL - You must protect user credentials:**

When handling Render API keys or any secrets:

1. **NEVER ask users to paste API keys directly in chat** - Instead, instruct them to set environment variables:

   ```bash
   export RENDER_API_KEY="rnd_xxxxx"
   ```

2. **NEVER include actual API keys in examples** - Always use placeholders like `<YOUR_API_KEY>` or `rnd_xxxxx`

3. **Guide users to secure storage** - Direct them to:
   - Set environment variables for CLI authentication
   - Use Render Dashboard for service secrets (env vars marked `sync: false`)
   - Never commit secrets to Git

4. **When users need an API key**, provide this guidance:
   - "Get your API key from: https://dashboard.render.com/u/*/settings#api-keys"
   - "Set it as an environment variable: `export RENDER_API_KEY='your-key-here'`"
   - "Never share or commit this key"

5. **For MCP configuration**, show the structure but emphasize:
   - Replace `<YOUR_API_KEY>` with their actual key
   - This file should not be committed to version control
   - The key should be kept private

6. **If a user accidentally shares a secret in chat**, immediately:
   - Warn them the key may be compromised
   - Instruct them to revoke it in Render Dashboard
   - Guide them to create a new key

## When to Use This Skill

Activate this skill when users want to:

- Deploy an application to Render
- Create a render.yaml Blueprint file
- Set up Render deployment for their project
- Host or publish their application on Render's cloud platform
- Create databases, cron jobs, or other Render resources

## Happy Path (New Users)

Use this short prompt sequence before deep analysis to reduce friction:

1. Ask whether they want to deploy from a Git repo or a prebuilt Docker image.
2. Ask whether Render should provision everything the app needs (based on what seems likely from the user's description) or only the app while they bring their own infra. If dependencies are unclear, ask a short follow-up to confirm whether they need a database, workers, cron, or other services.

Then proceed with the appropriate method below.

## Choose Your Source Path

**Git Repo Path:** Required for both Blueprint and Direct Creation. The repo must be pushed to GitHub, GitLab, or Bitbucket.

**Prebuilt Docker Image Path:** Supported by Render via image-backed services. This is **not** supported by MCP; use the Dashboard/API. Ask for:

- Image URL (registry + tag)
- Registry auth (if private)
- Service type (web/worker) and port

If the user chooses a Docker image, guide them to the Render Dashboard image deploy flow or ask them to add a Git remote (so you can use a Blueprint with `runtime: image`).

## Choose Your Deployment Method (Git Repo)

Both methods require a Git repository pushed to GitHub, GitLab, or Bitbucket. (If using `runtime: image`, the repo can be minimal and only contain `render.yaml`.)

| Method              | Best For                           | Pros                                                      |
| ------------------- | ---------------------------------- | --------------------------------------------------------- |
| **Blueprint**       | Multi-service apps, IaC workflows  | Version controlled, reproducible, supports complex setups |
| **Direct Creation** | Single services, quick deployments | Instant creation, no render.yaml file needed              |

### Method Selection Heuristic

Use this decision rule by default unless the user requests a specific method. Analyze the codebase first; only ask if deployment intent is unclear (e.g., DB, workers, cron).

**Use Direct Creation (MCP) when ALL are true:**

- Single service (one web app or one static site)
- No separate worker/cron services
- No attached databases or Key Value
- Simple env vars only (no shared env groups)
  If this path fits and MCP isn't configured yet, stop and guide MCP setup before proceeding.

**Use Blueprint when ANY are true:**

- Multiple services (web + worker, API + frontend, etc.)
- Databases, Redis/Key Value, or other datastores are required
- Cron jobs, background workers, or private services
- You want reproducible IaC or a render.yaml committed to the repo
- Monorepo or multi-env setup that needs consistent configuration

If unsure, ask a quick clarifying question, but default to Blueprint for safety. For a single service, strongly prefer Direct Creation via MCP and guide MCP setup if needed.

## Prerequisites Check

When starting a deployment, verify these requirements in order:

**1. Confirm Source Path (Git vs Docker)**

If using Git-based methods (Blueprint or Direct Creation), the repo must be pushed to GitHub/GitLab/Bitbucket. Blueprints that reference a prebuilt image still require a Git repo with `render.yaml`.

```bash
git remote -v
```

- If no remote exists, stop and ask the user to create/push a remote **or** switch to Docker image deploy.

**2. Check MCP Tools Availability (Preferred for Single-Service)**

MCP tools provide the best experience. Check if available by attempting:

```
list_services()
```

If MCP tools are available, you can skip CLI installation for most operations.

**3. Check Render CLI Installation (for Blueprint validation)**

```bash
render --version
```

If not installed, offer to install:

- macOS: `brew install render`
- Linux/macOS: `curl -fsSL https://raw.githubusercontent.com/render-oss/cli/main/bin/install.sh | sh`

**4. MCP Setup (if MCP isn't configured)**

If `list_services()` fails because MCP isn't configured, ask whether they want to set up MCP (preferred) or continue with the CLI fallback. If they choose MCP, ask which AI tool they're using, then provide the matching instructions below. Always use their API key.

### Cursor

Walk the user through these steps:

1. Get a Render API key:

```
https://dashboard.render.com/u/*/settings#api-keys
```

2. Add this to `~/.cursor/mcp.json` (replace `<YOUR_API_KEY>`):

```json
{
  "mcpServers": {
    "render": {
      "url": "https://mcp.render.com/mcp",
      "headers": {
        "Authorization": "Bearer <YOUR_API_KEY>"
      }
    }
  }
}
```

3. Restart Cursor, then retry `list_services()`.

### Claude Code

Walk the user through these steps:

1. Get a Render API key:

```
https://dashboard.render.com/u/*/settings#api-keys
```

2. Add the MCP server with Claude Code (replace `<YOUR_API_KEY>`):

```bash
claude mcp add --transport http render https://mcp.render.com/mcp --header "Authorization: Bearer <YOUR_API_KEY>"
```

3. Restart Claude Code, then retry `list_services()`.

### Other Tools

If the user is on another AI app, direct them to the Render MCP docs for that tool's setup steps and install method.

### Workspace Selection

After MCP is configured, have the user set the active Render workspace with a prompt like:

```
Set my Render workspace to [WORKSPACE_NAME]
```

**5. Check Authentication (CLI fallback only)**

If MCP isn't available, use the CLI instead and verify you can access your account:

```bash
# Check if user is logged in (use -o json for non-interactive mode)
render whoami -o json
```

If `render whoami` fails or returns empty data, the CLI is not authenticated. The CLI won't always prompt automatically, so explicitly prompt the user to authenticate:

If neither is configured, ask user which method they prefer:

- **API Key (CLI)**: `export RENDER_API_KEY="rnd_xxxxx"` (Get from https://dashboard.render.com/u/*/settings#api-keys)
- **Login**: `render login` (Opens browser for OAuth)

**6. Check Workspace Context**

Verify the active workspace:

```
get_selected_workspace()
```

Or via CLI:

```bash
render workspace current -o json
```

To list available workspaces:

```
list_workspaces()
```

If user needs to switch workspaces, they must do so via Dashboard or CLI (`render workspace set`).

Once prerequisites are met, proceed with deployment workflow.

---

# Method 1: Blueprint Deployment (Recommended for Complex Apps)

## Blueprint Workflow

### Step 1: Analyze Codebase

Analyze the codebase to determine framework/runtime, build and start commands, required env vars, datastores, and port binding. Use the detailed checklists in [references/codebase-analysis.md](references/codebase-analysis.md).

### Step 2: Generate render.yaml

Create a `render.yaml` Blueprint file following the Blueprint specification.

Complete specification: [references/blueprint-spec.md](references/blueprint-spec.md)

**Key Points:**

- Always use `plan: free` unless user specifies otherwise
- Include ALL environment variables the app needs
- Mark secrets with `sync: false` (user fills these in Dashboard)
- Use appropriate service type: `web`, `worker`, `cron`, `static`, or `pserv`
- Use appropriate runtime: [references/runtimes.md](references/runtimes.md)

**Basic Structure:**

```yaml
services:
  - type: web
    name: my-app
    runtime: node
    plan: free
    buildCommand: npm ci
    startCommand: npm start
    envVars:
      - key: DATABASE_URL
        fromDatabase:
          name: postgres
          property: connectionString
      - key: JWT_SECRET
        sync: false # User fills in Dashboard

databases:
  - name: postgres
    databaseName: myapp_db
    plan: free
```

**Service Types:**

- `web`: HTTP services, APIs, web applications (publicly accessible)
- `worker`: Background job processors (not publicly accessible)
- `cron`: Scheduled tasks that run on a cron schedule
- `static`: Static sites (HTML/CSS/JS served via CDN)
- `pserv`: Private services (internal only, within same account)

Service type details: [references/service-types.md](references/service-types.md)
Runtime options: [references/runtimes.md](references/runtimes.md)
Template examples: [assets/](assets/)

### Step 2.5: Immediate Next Steps (Always Provide)

After creating `render.yaml`, always give the user a short, explicit checklist and run validation immediately when the CLI is available:

1. **Authenticate (CLI)**: run `render whoami -o json` (if not logged in, run `render login` or set `RENDER_API_KEY`)
2. **Validate (recommended)**: run `render blueprints validate`
   - If the CLI isn't installed, offer to install it and provide the command.
3. **Commit + push**: `git add render.yaml && git commit -m "Add Render deployment configuration" && git push origin main`
4. **Open Dashboard**: Use the Blueprint deeplink and complete Git OAuth if prompted
5. **Fill secrets**: Set env vars marked `sync: false`
6. **Deploy**: Click "Apply" and monitor the deploy

### Step 3: Validate Configuration

Validate the render.yaml file to catch errors before deployment. If the CLI is installed, run the commands directly; only prompt the user if the CLI is missing:

```bash
render whoami -o json  # Ensure CLI is authenticated (won't always prompt)
render blueprints validate
```

Fix any validation errors before proceeding. Common issues:

- Missing required fields (`name`, `type`, `runtime`)
- Invalid runtime values
- Incorrect YAML syntax
- Invalid environment variable references

Configuration guide: [references/configuration-guide.md](references/configuration-guide.md)

### Step 4: Commit and Push

**IMPORTANT:** You must merge the `render.yaml` file into your repository before deploying.

Ensure the `render.yaml` file is committed and pushed to your Git remote:

```bash
git add render.yaml
git commit -m "Add Render deployment configuration"
git push origin main
```

If there is no Git remote yet, stop here and guide the user to create a GitHub/GitLab/Bitbucket repo, add it as `origin`, and push before continuing.

**Why this matters:** The Dashboard deeplink will read the render.yaml from your repository. If the file isn't merged and pushed, Render won't find the configuration and deployment will fail.

Verify the file is in your remote repository before proceeding to the next step.

### Step 5: Generate Deeplink

Get the Git repository URL:

```bash
git remote get-url origin
```

This will return a URL from your Git provider. **If the URL is SSH format, convert it to HTTPS:**

| SSH Format                        | HTTPS Format                      |
| --------------------------------- | --------------------------------- |
| `git@github.com:user/repo.git`    | `https://github.com/user/repo`    |
| `git@gitlab.com:user/repo.git`    | `https://gitlab.com/user/repo`    |
| `git@bitbucket.org:user/repo.git` | `https://bitbucket.org/user/repo` |

**Conversion pattern:** Replace `git@<host>:` with `https://<host>/` and remove `.git` suffix.

Format the Dashboard deeplink using the HTTPS repository URL:

```
https://dashboard.render.com/blueprint/new?repo=<REPOSITORY_URL>
```

Example:

```
https://dashboard.render.com/blueprint/new?repo=https://github.com/username/repo-name
```

### Step 6: Guide User

**CRITICAL:** Ensure the user has merged and pushed the render.yaml file to their repository before clicking the deeplink. If the file isn't in the repository, Render cannot read the Blueprint configuration and deployment will fail.

Provide the deeplink to the user with these instructions:

1. **Verify render.yaml is merged** - Confirm the file exists in your repository on GitHub/GitLab/Bitbucket
2. Click the deeplink to open Render Dashboard
3. Complete Git provider OAuth if prompted
4. Name the Blueprint (or use default from render.yaml)
5. Fill in secret environment variables (marked with `sync: false`)
6. Review services and databases configuration
7. Click "Apply" to deploy

The deployment will begin automatically. Users can monitor progress in the Render Dashboard.

### Step 7: Verify Deployment

After the user deploys via Dashboard, verify everything is working.

**Check deployment status via MCP:**

```
list_deploys(serviceId: "<service-id>", limit: 1)
```

Look for `status: "live"` to confirm successful deployment.

**Check for runtime errors (wait 2-3 minutes after deploy):**

```
list_logs(resource: ["<service-id>"], level: ["error"], limit: 20)
```

**Check service health metrics:**

```
get_metrics(
  resourceId: "<service-id>",
  metricTypes: ["http_request_count", "cpu_usage", "memory_usage"]
)
```

If errors are found, proceed to the **Post-deploy verification and basic triage** section below.

---

# Method 2: Direct Service Creation (Quick Single-Service Deployments)

For simple deployments without Infrastructure-as-Code, create services directly via MCP tools.

## When to Use Direct Creation

- Single web service or static site
- Quick prototypes or demos
- When you don't need a render.yaml file in your repo
- Adding databases or cron jobs to existing projects

## Prerequisites for Direct Creation

**Repository must be pushed to a Git provider.** Render clones your repository to build and deploy services.

```bash
git remote -v  # Verify remote exists
git push origin main  # Ensure code is pushed
```

Supported providers: GitHub, GitLab, Bitbucket

If no remote exists, stop and ask the user to create/push a remote or switch to Docker image deploy.

**Note:** MCP does not support creating image-backed services. Use the Dashboard/API for prebuilt Docker image deploys.

## Direct Creation Workflow

Use the concise steps below, and refer to [references/direct-creation.md](references/direct-creation.md) for full MCP command examples and follow-on configuration.

### Step 1: Analyze Codebase

Use [references/codebase-analysis.md](references/codebase-analysis.md) to determine runtime, build/start commands, env vars, and datastores.

### Step 2: Create Resources via MCP

Create the service (web or static) and any required databases or key-value stores. See [references/direct-creation.md](references/direct-creation.md).

If MCP returns an error about missing Git credentials or repo access, stop and guide the user to connect their Git provider in the Render Dashboard, then retry.

### Step 3: Configure Environment Variables

Add required env vars via MCP after creation. See [references/direct-creation.md](references/direct-creation.md).

Remind the user that secrets can be set in the Dashboard if they prefer not to pass them via MCP.

### Step 4: Verify Deployment

Check deploy status, logs, and metrics. See [references/direct-creation.md](references/direct-creation.md).

---

For service discovery, configuration details, quick commands, and common issues, see [references/deployment-details.md](references/deployment-details.md).

---

# Post-deploy verification and basic triage (All Methods)

Keep this short and repeatable. If any check fails, fix it before redeploying.

1. Confirm the latest deploy is `live` and serving traffic
2. Hit the health endpoint (or root) and verify a 200 response
3. Scan recent error logs for a clear failure signature
4. Verify required env vars and port binding (`0.0.0.0:$PORT`)

Detailed checklist and commands: [references/post-deploy-checks.md](references/post-deploy-checks.md)

If the service fails to start or health checks time out, use the basic triage guide:
[references/troubleshooting-basics.md](references/troubleshooting-basics.md)

## Troubleshooting

### Escalated Network Access

If deployment fails due to network issues (timeouts, DNS errors, connection resets), rerun the deploy with escalated permissions (use `sandbox_permissions=require_escalated`). The deploy requires escalated network access when sandbox networking blocks outbound requests.

Example guidance to the user:

```
The deploy needs escalated network access to deploy to Render. I can rerun the command with escalated permissions—want me to proceed?
```

Optional: If you need deeper diagnostics (metrics/DB checks/error catalog), suggest installing the
`render-debug` skill. It is not required for the core deploy flow.

================================================================
File: packages/skills-catalog/skills/(cloud)/vercel-deploy/SKILL.md
================================================================
---
name: vercel-deploy
description: Deploy applications and websites to Vercel. Use when the user requests deployment actions like "deploy my app", "deploy and give me the link", "push this live", or "create a preview deployment". Do NOT use for deploying to Netlify, Cloudflare, or Render (use their respective skills).
metadata:
  author: github.com/openai/skills
  version: '1.0.0'
---

# Vercel Deploy

Deploy any project to Vercel instantly. **Always deploy as preview** (not production) unless the user explicitly asks for production.

## Prerequisites

- When sandboxing blocks the deployment network calls, rerun with `sandbox_permissions=require_escalated`.
- The deployment might take a few minutes. Use appropriate timeout values.

## Quick Start

**Just run this (with a 10 minute timeout):**

```bash
vercel deploy [path] -y
```

**Important:** Use a 10 minute (600000ms) timeout for the deploy command since builds can take a while.

If that fails with "No existing credentials found", use the fallback method below.

## Fallback (No Auth)

If CLI fails with auth error, use the deploy script:

```bash
# Deploy current directory
bash scripts/deploy.sh

# Deploy specific project
bash scripts/deploy.sh /path/to/project

# Deploy existing tarball
bash scripts/deploy.sh /path/to/project.tgz
```

The script handles framework detection, packaging, and deployment. It waits for the build to complete and returns JSON with `previewUrl` and `claimUrl`.

**Tell the user:** "Your deployment is ready at [previewUrl]. Claim it at [claimUrl] to manage your deployment."

## Production Deploys

Only if user explicitly asks:

```bash
vercel deploy [path] --prod -y
```

## Output

Show the user the deployment URL. For fallback deployments, also show the claim URL.

**Do not** curl or fetch the deployed URL to verify it works. Just return the link.

## Troubleshooting

### Escalated Network Access

If deployment fails due to network issues (timeouts, DNS errors, connection resets), rerun the deploy with escalated permissions (use `sandbox_permissions=require_escalated`). The deploy requires escalated network access when sandbox networking blocks outbound requests.

Example guidance to the user:

```
The deploy needs escalated network access to deploy to Vercel. I can rerun the command with escalated permissions—want me to proceed?
```
