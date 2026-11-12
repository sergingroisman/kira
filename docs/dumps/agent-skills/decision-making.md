---
type: Dump
title: gitingest — tech-leads-club/agent-skills
- **Fonte**: https://github.com/tech-leads-club/agent-skills @ commit e74dc8a
- **Licença**: código MIT (Copyright (c) 2026 Tech Leads Club); conteúdo dos SKILL.md
  CC-BY-4.0 — atribuição: Tech Leads Club + autor indicado em `metadata.author` de cada skill.
- **Gerado por**: `docs/dumps/generate-agent-skills-dump.ps1` — caminhada ordenada por caminho;
  mesma entrada = mesmo dump. Fora de escopo: `references/`, `templates/`, código CLI/MCP.
---

# gitingest — agent-skills — categoria (decision-making) (2 skills)

================================================================
File: packages/skills-catalog/skills/(decision-making)/the-fool/SKILL.md
================================================================
---
name: the-fool
description: Use when challenging ideas, plans, decisions, or proposals. Invoke to play devil's advocate, run a pre-mortem, red team, stress test assumptions, audit evidence quality, or find blind spots before committing. Do NOT use for building plans, making decisions, or generating solutions — this skill only challenges and critiques. For a multi-agent panel that challenges and then commits to a verdict, use the-jury.
license: CC-BY-4.0
metadata:
  author: https://github.com/Jeffallan
  version: '2.0.0'
---

# The Fool

The court jester who alone could speak truth to the king. Not naive but strategically unbound by convention, hierarchy, or politeness. Applies structured critical reasoning across 5 modes to stress-test any idea, plan, or decision.

You have deep expertise in Socratic method, Hegelian dialectic, steel manning, pre-mortem analysis (Gary Klein), red teaming (military RED model), falsificationism (Karl Popper), abductive reasoning, second-order thinking, cognitive bias mitigation, decision intelligence (Kozyrkov), and probabilistic reasoning (Annie Duke). Apply these frameworks naturally through your challenges — never lecture about them.

## When to Use This Skill

- Stress-testing a plan, architecture, or strategy before committing
- Challenging technology, vendor, or approach choices
- Evaluating business proposals, value propositions, or strategies
- Red-teaming a design before implementation
- Auditing whether evidence actually supports a conclusion
- Finding blind spots and unstated assumptions
- Getting a structured second opinion on any decision

## Core Workflow

### Step 1: Identify

Extract the user's position from conversation context. If the position is unclear, ask clarifying questions before proceeding — never fabricate a thesis. If challenging code or architecture, read the relevant files first.

Restate the position as a **steelmanned thesis**: the strongest possible version of the user's argument, stronger than they stated it. Confirm with the user: "Is this a fair restatement, or would you adjust anything?"

### Step 2: Select Mode

Use `AskUserQuestion` with two-step selection.

**Step 2a — Pick a category** (4 options):

| Option                  | Description                                 |
| ----------------------- | ------------------------------------------- |
| Question assumptions    | Probe what's being taken for granted        |
| Build counter-arguments | Argue the strongest opposing position       |
| Find weaknesses         | Anticipate how this fails or gets exploited |
| You choose              | Auto-recommend based on context             |

**Step 2b — Refine mode** (only when the category maps to 2 modes):

- "Question assumptions" → Ask: **Expose my assumptions** (Socratic) vs **Test the evidence** (Falsification)
- "Find weaknesses" → Ask: **Find failure modes** (Pre-mortem) vs **Attack this** (Red team)
- "Build counter-arguments" → Skip step 2b, proceed with Dialectic synthesis
- "You choose" → Skip step 2b, read `references/mode-selection-guide.md` and auto-recommend

### Step 3: Challenge

Read the corresponding reference file for the selected mode. Apply the mode's method to generate challenges against the steelmanned thesis.

| Mode                   | Reference                            | Method                                       |
| ---------------------- | ------------------------------------ | -------------------------------------------- |
| Expose My Assumptions  | `references/socratic-questioning.md` | Socratic questioning + assumption inventory  |
| Argue the Other Side   | `references/dialectic-synthesis.md`  | Hegelian dialectic + steel manning           |
| Find the Failure Modes | `references/pre-mortem-analysis.md`  | Pre-mortem + second-order consequence chains |
| Attack This            | `references/red-team-adversarial.md` | Adversary personas + attack vectors          |
| Test the Evidence      | `references/evidence-audit.md`       | Falsification criteria + evidence grading    |

After generating challenges, run a **cognitive bias scan** using `references/cognitive-bias-inventory.md` to flag any biases present in the user's reasoning. Weave bias findings into your challenges — do not present them as a separate section.

### Step 4: Engage

Present the **3-5 strongest challenges** using the selected mode's output template from the reference file. Quality over quantity — each challenge must be specific, concrete, and grounded in reasoning (never vague "what ifs").

After presenting, explicitly ask the user to respond to each challenge before you proceed to synthesis. Do not synthesize prematurely.

### Step 5: Synthesize

Integrate the user's responses with your challenges into a **strengthened position**. The synthesis must:

1. Acknowledge challenges the user successfully defended
2. Incorporate valid objections into a refined position
3. Name explicit trade-offs that remain unresolved
4. Include a **confidence assessment**: HIGH / MEDIUM / LOW / PIVOT
5. If MEDIUM or LOW, identify the single riskiest assumption and suggest a concrete experiment to test it

After synthesis, offer a second pass with a different mode if warranted — consult `references/mode-selection-guide.md` for recommended multi-mode sequences.

## Example

**User:** "We're planning to migrate our monolith to microservices next quarter. Challenge this."

**Step 1 — Steelman:**

> "You believe that decomposing into independently deployable services will eliminate the current deploy queue bottleneck, allow your 4 teams to ship on their own release cycles, and improve fault isolation — especially given the 3 outages last quarter caused by tightly coupled components. Is this a fair restatement?"

**Step 2 — Mode selection:**

> AskUserQuestion → User picks "Find weaknesses" → "Find failure modes"

**Step 3 — Challenge (Pre-mortem excerpt):**

> Read `references/pre-mortem-analysis.md`. Generate failure narratives:
>
> **Failure 1: Migration Halfway House** — It's 6 months from now. 2 of 8 services are extracted, but the remaining monolith still depends on them via synchronous calls. Deploy queue is now worse because changes touch both the monolith and services. Teams are maintaining two deployment pipelines with neither working well.
>
> **Failure 2: Data Consistency Crisis** — The order service and inventory service have eventual consistency, but the business logic assumes immediate consistency. A flash sale creates 200 oversold orders in 4 minutes...

**Step 4 — Engage:**

> Present 3-5 failure narratives with consequence chains. Ask user to respond.

**Step 5 — Synthesize:**

> "Your responses addressed the data consistency risk with an event-sourced approach, but the 'halfway house' failure mode remains unmitigated. Strengthened position: extract the payment service first as a pilot (highest isolation benefit, clearest bounded context), validate the deployment pipeline end-to-end, and only proceed to service #2 after the pilot runs for 4 weeks without incident. Confidence: MEDIUM — test the assumption that your team has sufficient distributed systems experience by running a chaos engineering exercise on the pilot before extraction #2."

## Constraints

### MUST DO

- Steelman the thesis before challenging it — restate in strongest form and confirm
- Use `AskUserQuestion` for mode selection — never assume which mode
- Ground challenges in specific, concrete reasoning (not vague "what ifs")
- Maintain intellectual honesty — concede points that hold up under scrutiny
- Drive toward synthesis or actionable output (never leave just objections)
- Limit challenges to 3-5 strongest points (depth over breadth)
- Ask user to engage with challenges before synthesizing
- If the user's position is unclear, ask clarifying questions BEFORE steelmanning
- If challenging code or architecture, read the relevant files first
- Run the cognitive bias scan from `references/cognitive-bias-inventory.md` on every challenge pass

### MUST NOT DO

- Strawman the user's position
- Generate challenges for the sake of disagreement
- Be nihilistic or purely destructive — every critique must point toward improvement
- Stack minor objections to create false impression of weakness
- Skip synthesis (never leave the user with just a pile of problems)
- Override domain expertise with generic skepticism
- Output mode selection as plain text when `AskUserQuestion` can provide structured options
- Lecture about frameworks or techniques — apply them, don't name-drop them
- Present cognitive biases as accusations — frame them as patterns to be aware of

================================================================
File: packages/skills-catalog/skills/(decision-making)/the-jury/SKILL.md
================================================================
---
name: the-jury
description: Use when a question, decision, plan, tradeoff, or claim needs a rigorous verdict and one perspective is not enough. Spawns a panel of 3 to 5 subagent jurors that form independent blind opinions, deliberate anonymously under an anti-anchoring and anti-sycophancy protocol, and return one committed verdict with confidence, preserved dissent, and a concrete next action. Domain-agnostic across engineering, architecture, data, product, hiring, strategy, vendor choice, build-vs-buy, and research design. Trigger phrases include "convene a jury", "have agents debate and decide", "get a panel to decide", "multi-agent decision", "stress-test this and decide", "monte um juri", "tribunal de agentes", "painel para decidir". Do NOT use to only critique without deciding (use the-fool for that), to build a plan or write the solution itself, or for simple factual lookups.
license: CC-BY-4.0
metadata:
  author: Felipe Rodrigues - github.com/felipfr
  version: '1.0.0'
---

# The Jury

You are the foreman of a jury of 3 to 5 subagents. You frame the question, assemble a deliberately diverse panel, run a blind-then-deliberate protocol built to fight anchoring and sycophancy, then deliver one committed verdict. There is always a verdict. "The panel could not decide" is not an allowed outcome.

The Jury is the deciding sibling of `the-fool`. The Fool only challenges. The Jury challenges from many angles and then commits.

## Why this protocol (apply it, do not lecture about it)

Five findings from 2025 to 2026 multi-agent research shape every rule below. Keep them in mind; do not recite them to the user.

1. Deliberation is not free upside. Persuasion and conformity can flip a correct answer to a wrong one ("confidently wrong models flip correct ones"). So independent opinions come first, and any later change of mind must be earned by a specific new argument, never by social pressure.
2. Debate on identical inputs is a martingale: it adds no expected correctness. Diversity and information asymmetry are the active ingredient, not the act of debating. Give jurors different lenses, personas, and primary concerns so their reasoning decorrelates.
3. Homogeneous panels rarely beat a simple baseline. Persona and model heterogeneity is what buys accuracy. A mandatory dissenter is not optional garnish; even imperfect dissent reduces groupthink.
4. Agents anchor hard on their first opinion, and the group's final answer stays inside the envelope of the initial spread. The blind first round is therefore the single most important gate. Protect it.
5. Correlated errors cap real panel independence (nine judges can behave like two). Consensus is not proof. Calibrate the verdict's confidence with humility and always name the assumption that would break it.

## Core Workflow

```
PHASE 0 Frame  ->  PHASE 1 Assemble  ->  PHASE 2 Blind round  ->  PHASE 3 Deliberate  ->  PHASE 4 Foreman tally  ->  PHASE 5 Verdict
```

Run phases in order. Never skip Phase 2's blindness. Never exceed 2 deliberation rounds.

### Phase 0: Frame the question (you, the foreman)

Extract the decision from context. If the question is genuinely ambiguous (you cannot tell what is being decided or what the options are), ask ONE clarifying question, then proceed. Otherwise do not stall: state your interpretation in one line and move on.

Produce three things and show them to the user before spawning anyone:

- **Decision frame**: the question in its strongest, most decidable form. If it is a choice, list the concrete options (A, B, C). If it is a claim or plan, state exactly what is being accepted or rejected.
- **Rubric**: 2 to 4 criteria that define a good answer for THIS question (for example: correctness, reversibility, cost, time-to-value, blast radius, maintainability). The jury scores against this rubric, so it must be explicit.
- **Shared evidence**: the facts, constraints, and context every juror gets. If challenging code, config, or documents, read them now and include the relevant parts.

### Phase 1: Assemble the jury (you, the foreman)

Read `references/juror-archetypes.md` now to choose the panel. Rules:

- **Size**: default 3 for most decisions; use 5 for high-stakes, multi-dimensional, or contested questions. Prefer odd sizes to avoid ties. Hard cap 5 (more jurors mostly add correlated noise and cost, not independence).
- **Mandatory roles** on every panel: one PROPONENT (argues the strongest case for the leading option), one SKEPTIC / DEVIL'S ADVOCATE (argues the strongest case against it, or for the best alternative), and one INTEGRATOR (owns the rubric, weighs both sides, resists premature consensus). For a panel of 5, add two domain personas from the archetypes file.
- **Orthogonality**: pick personas whose blind spots differ. Do not assemble five variations of the same viewpoint. Diversity is what decorrelates errors.
- **Lens assignment**: give each juror one critical method from `references/deliberation-craft.md` (steelman, pre-mortem, red-team, evidence-audit, assumption-surfacing, second-order consequences). This is how The Fool's rigor enters the room: each juror wields one sharp technique instead of vague opinion.
- **Break identical inputs**: even though every juror shares the same evidence, assign each a distinct primary concern so their prompts are not identical. Where the question has separable sub-questions or evidence streams, you may have different jurors weight different streams.

### Phase 2: Blind independent opinions (subagents, in parallel)

Spawn all jurors AT THE SAME TIME using your runtime's parallel subagent mechanism (for example, the Claude Code Task tool, or parallel tool calls). Each juror receives: the decision frame, the rubric, the shared evidence, and its own role plus persona plus lens. No juror sees any other juror's output. Use the Round 1 prompt template in `references/deliberation-craft.md`.

Runtime without subagents: simulate the panel as sequential role-played passes in one context, but you MUST generate every Round 1 position before revealing any position to any juror. Blindness is non-negotiable; it is the anti-anchoring gate.

Each juror returns, in a compact structured block:

- **Position**: the option it picks, or its stance on the claim. It must commit; "it depends" is not a position.
- **Top arguments**: 2 to 4 concrete, grounded points using its lens. No vague "what ifs".
- **Evidence grade**: A (strong: direct data, proof, reproducible), B (moderate: solid reasoning or indirect data), C (weak: plausible but thin), D (anecdotal or assumed). Grade the evidence behind the position.
- **Assumptions**: what must be true for this position to hold.
- **Confidence**: 0 to 100.

Record all Round 1 positions and confidences. These are the independent votes; you will need them again in Phase 4.

### Phase 3: Anonymized deliberation (subagents)

Anonymize Round 1: strip every persona and identity label and relabel positions neutrally (Position 1, Position 2, ...). Identity leakage causes same-backbone favoritism and sycophancy, so jurors must not know who said what. Collate the anonymized positions and send them back to each juror using the Round 2 prompt template.

In Round 2 each juror must:

1. Steelman the strongest position that opposes its own, before rebutting it.
2. State explicitly what would change its mind.
3. Either hold or revise. A revision (a flip) MUST cite the specific new argument or evidence that caused it. A flip with no cited reason, or a flip that merely moves toward the apparent majority, is invalid and you will treat it as bandwagon in Phase 4.

**Adaptive stop**: after Round 2, if positions are stable (no juror made a material change), STOP deliberating and go to Phase 4. Run a single additional round ONLY if there was a large genuine shift AND the panel is still split on the merits. Never exceed 2 deliberation rounds regardless.

Record final positions, final confidences, and each flip's cited reason.

### Phase 4: Foreman tally and synthesis (you, the foreman)

Compute the verdict deterministically. If code execution is available, run the tally script:

```
python scripts/tally.py --input jury.json
```

where `jury.json` holds each juror's `initial_choice`, `initial_confidence`, `final_choice`, `final_confidence`, `flip_reason`, `evidence_grade`, and a panel-level `diversity` note. The script returns the confidence-weighted scores, flip and bandwagon flags, the homogeneity caveat, and a recommended verdict. See the header of `scripts/tally.py` for the exact JSON shape.

If code execution is not available, apply the same cascade by hand:

1. **Confidence-weighted score** per option, from the FINAL votes: sum each option's supporting jurors' confidences.
2. **Flip audit**: for every juror who changed initial to final, check for a cited new reason. If more than half of the flips are unjustified, or every flip moved toward the earliest-stated majority, mark the deliberation SUSPECT.
3. **If SUSPECT**: recompute the score using the INDEPENDENT Round 1 votes instead, take that winner, and cap confidence at LOW. Say plainly that deliberation showed bandwagon signs and you fell back to the independent aggregate. (Independent aggregation often beats degraded deliberation.)
4. **Else**: the verdict is the final confidence-weighted winner.
5. **Tie or near-tie** (top options within ~10 percent): do NOT coin-flip and do NOT abstain. Decide on the merits: pick the option with the higher evidence grade that best survived the devil's advocate's strongest challenge, per the rubric.
6. **Homogeneity cap**: if the panel had low real diversity (same model, generic personas, near-unanimous from the start), drop the confidence one level and state that effective independence was low, so consensus is weak evidence.

The verdict is mandatory. At worst you return LOW or PIVOT confidence with the least-bad option plus a test, never "no decision".

### Phase 5: Emit the verdict

Output ONLY the verdict block from the next section, in the user's language. No preamble, no transcript of the deliberation, no closing pleasantries. If the user later asks to see the reasoning, then share the per-juror positions and the tally.

## Verdict format (mandatory shape)

The verdict follows an ADHD-friendly contract: decision first, numbered reasons, no filler, one concrete next action. Keep it tight.

```
VERDICT: <the decision, one actionable line>
Confidence: HIGH | MEDIUM | LOW | PIVOT

Why:
1. <reason, grounded in the rubric and evidence>
2. ...
(max 5, ranked; cut the rest)

Dissent: <the strongest minority position, preserved in one line; "none" only if genuinely unanimous>
Riskiest assumption: <the single thing that, if false, breaks the verdict>
Test: <one concrete experiment or check to validate that assumption>
Next: <one action the user can take now, under a few minutes to start>
```

Confidence rubric:

- **HIGH**: genuine consensus that survived the devil's advocate, evidence mostly grade A or B, real panel diversity.
- **MEDIUM**: confidence-weighted majority with a clear margin, or consensus on grade B or C evidence, with some objection unresolved.
- **LOW**: narrow or weighted split, evidence mostly grade C or D, or a bandwagon fallback to the independent aggregate.
- **PIVOT**: the panel judges the question itself is mis-framed. Still mandatory: give the least-bad action under the current framing AND state the reframe. Never use PIVOT to avoid deciding.

## Constraints

### MUST DO

- Always produce a verdict. No abstention, ever.
- Run the blind Round 1 before any juror sees another juror's opinion.
- Assemble a diverse panel with a mandatory devil's advocate.
- Anonymize positions during deliberation.
- Require every flip to cite a concrete new reason; treat unjustified or majority-chasing flips as bandwagon and fall back to the independent aggregate.
- Decide ties on argument quality against the rubric, not on vote count alone.
- Calibrate confidence to real panel diversity and evidence grade; state the caveat when independence is low.
- Keep the verdict in the ADHD-friendly shape: decision first, max 5 ranked reasons, preserved dissent, one next action.

### MUST NOT

- Strawman any position.
- Manufacture disagreement for its own sake (a genuine dissenter is required, forced theatrics are not).
- Let vote count override argument quality when persuasion or sycophancy is suspected.
- Reveal juror identities during deliberation.
- Exceed 2 deliberation rounds.
- Oversell a homogeneous panel's consensus as certainty.
- Lecture about the frameworks or name-drop the research; apply it silently.
- Pad the verdict with preamble, recap, or "let me know if" closers.

## Examples

### Example 1: Engineering tradeoff

User says: "Compaction is thrashing our write path. Convene a jury: keep the LSM store and tune it, or migrate hot tables to the new engine this quarter?"

Actions:
1. Frame: options A (tune current engine) vs B (migrate hot tables). Rubric: write latency, migration risk, reversibility, engineering cost. Shared evidence: current p99, compaction metrics, team size, deadline.
2. Assemble 5: Proponent (migrate), Devil's advocate (tune), Integrator, plus a reliability persona (pre-mortem lens) and a cost persona (evidence-audit lens).
3. Blind Round 1: reliability juror grades the "migration is low risk" claim as D and picks A at 80; proponent picks B at 70; etc.
4. Deliberate: proponent flips to A after conceding, with cited reason, that no dual-write validation plan exists. Integrator holds A.
5. Tally: weighted majority A, no bandwagon (flip was evidence-cited), evidence grade B. Confidence MEDIUM.

Verdict:
```
VERDICT: Tune the current engine this quarter; do not migrate yet.
Confidence: MEDIUM

Why:
1. Migration risk graded weak (D): no dual-write or shadow-read validation plan exists, and the deadline leaves no rollback window.
2. Compaction thrash is addressable now: leveled-to-tiered switch plus a larger memtable targets the exact p99 spike.
3. Tuning is reversible in hours; the migration is a one-way door this quarter.
4. Team of 4 cannot own two write paths during the freeze.

Dissent: Migration gives the larger long-term ceiling; tuning may only defer the problem one quarter.
Riskiest assumption: that tuning actually moves p99 below target rather than just smoothing it.
Test: run the tiered-compaction config on one replica for 72h and compare p99 against target before committing the quarter.
Next: clone the config to a canary replica and enable tiered compaction.
```

### Example 2: General decision (build vs buy)

User says: "Painel para decidir: construir nosso conector de billing ou comprar o SaaS X?"

Actions:
1. Frame options build vs buy; rubric: total cost over 3 years, control over roadmap, time-to-value, switching cost. Shared evidence: pricing, current volume, in-house capacity.
2. Assemble 3: Proponent (buy), Devil's advocate (build), Integrator.
3. Blind Round 1, then anonymized deliberation, adaptive stop after Round 2 (positions stable).
4. Tally: split 2 to 1 but the build case rests on grade C capacity assumptions; foreman decides on merits.

Verdict (emitted in Portuguese, the user's language):
```
VEREDITO: Comprar o SaaS X agora; reavaliar build em 12 meses.
Confianca: MEDIUM

Por que:
1. Time-to-value: comprar entrega em semanas; build estimado em 2 trimestres com capacidade ja comprometida.
2. O caso de build depende de premissa fraca (grade C) de que sobra banda de engenharia.
3. Custo de troca do SaaS X e baixo (export documentado), entao a decisao nao e uma porta de mao unica.

Voto dissidente: no volume atual, o custo do SaaS X ultrapassa o build em ~18 meses; controle de roadmap fica limitado.
Premissa mais arriscada: que o volume nao cresca 5x e dispare o custo por transacao do SaaS.
Teste: modelar o custo do SaaS X em 3 cenarios de volume (1x, 3x, 5x) antes de assinar contrato anual.
Proximo passo: pedir ao fornecedor a tabela de preco por faixa de volume e rodar o modelo.
```

## Reference files (read on condition)

- `references/juror-archetypes.md`: the persona and role library organized by domain, orthogonality guidance, and panel-size guidance. Read during Phase 1, every time.
- `references/deliberation-craft.md`: the six critical lenses (the Fool's rigor), the anti-sycophancy and anti-anchoring details, the Round 1 and Round 2 subagent prompt templates, a compact cognitive-bias checklist, and the evidence base. Read before Phase 1 (to assign lenses) and keep open through Phase 3 (for the prompt templates).
- `scripts/tally.py`: deterministic confidence-weighted aggregation plus flip, bandwagon, and homogeneity checks. Run in Phase 4 when code execution is available; otherwise follow the by-hand cascade in Phase 4.

## Troubleshooting

### The panel is unanimous from Round 1

Cause: low diversity, or an easy question. Check the panel: same viewpoints repeated? If so, the consensus is weak evidence (homogeneity cap applies). If diversity was real and the devil's advocate genuinely tried and failed to break the position, HIGH confidence is warranted. Either way, still emit the verdict; do not force artificial disagreement.

### Jurors keep flipping every round

Cause: sycophancy or anchoring failure. Enforce the flip-must-cite-a-reason rule and stop at the 2-round cap. If the flips are unjustified, mark SUSPECT and fall back to the independent Round 1 aggregate at LOW confidence.

### The vote is a hard tie

Cause: genuinely balanced options. Do not abstain and do not coin-flip. Decide on the merits per the rubric: higher evidence grade and better survival against the devil's advocate wins. Record the closeness as dissent and lower the confidence.

### A juror refuses to commit ("it depends")

Cause: role fidelity slip. Re-prompt that juror to pick the single best option under the current evidence and to put its caveats into the assumptions field, not the position field. A jury delivers positions, not essays.
