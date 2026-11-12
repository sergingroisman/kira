---
type: Dump
title: gitingest — tech-leads-club/agent-skills
- **Fonte**: https://github.com/tech-leads-club/agent-skills @ commit e74dc8a
- **Licença**: código MIT (Copyright (c) 2026 Tech Leads Club); conteúdo dos SKILL.md
  CC-BY-4.0 — atribuição: Tech Leads Club + autor indicado em `metadata.author` de cada skill.
- **Gerado por**: `docs/dumps/generate-agent-skills-dump.ps1` — caminhada ordenada por caminho;
  mesma entrada = mesmo dump. Fora de escopo: `references/`, `templates/`, código CLI/MCP.
---

# gitingest — agent-skills — categoria (architecture) (14 skills)

================================================================
File: packages/skills-catalog/skills/(architecture)/component-common-domain-detection/SKILL.md
================================================================
---
name: component-common-domain-detection
description: Finds duplicate business logic spread across multiple components and suggests consolidation. Use when asking "where is this logic duplicated?", "find common code between services", "what can be consolidated?", "detect shared domain logic", or analyzing component overlap before refactoring. Do NOT use for code-level duplication detection (use linters) or dependency analysis (use coupling-analysis).
---

# Common Domain Component Detection

This skill identifies common domain functionality that is duplicated across multiple components and suggests consolidation opportunities to reduce duplication and improve maintainability.

## How to Use

### Quick Start

Request analysis of your codebase:

- **"Find common domain functionality across components"**
- **"Identify duplicate domain logic that should be consolidated"**
- **"Detect shared classes used across multiple components"**
- **"Analyze consolidation opportunities for common components"**

### Usage Examples

**Example 1: Find Common Functionality**

```
User: "Find common domain functionality across components"

The skill will:
1. Scan component namespaces for common patterns
2. Detect shared classes used across components
3. Identify duplicate domain logic
4. Analyze coupling impact of consolidation
5. Suggest consolidation opportunities
```

**Example 2: Detect Duplicate Notification Logic**

```
User: "Are there multiple notification components that should be consolidated?"

The skill will:
1. Find all components with notification-related names
2. Analyze their functionality and dependencies
3. Calculate coupling impact if consolidated
4. Recommend consolidation approach
```

**Example 3: Analyze Shared Classes**

```
User: "Find classes that are shared across multiple components"

The skill will:
1. Identify classes imported/used by multiple components
2. Classify as domain vs infrastructure functionality
3. Suggest consolidation or shared library approach
4. Assess impact on coupling
```

### Step-by-Step Process

1. **Scan Components**: Identify components with common namespace patterns
2. **Detect Shared Code**: Find classes/files used across components
3. **Analyze Functionality**: Determine if functionality is truly common
4. **Assess Coupling**: Calculate coupling impact before consolidation
5. **Recommend Actions**: Suggest consolidation or shared library approach

## When to Use

Apply this skill when:

- After identifying and sizing components (Pattern 1)
- Before flattening components (Pattern 3)
- When planning to reduce code duplication
- Analyzing shared domain logic across the codebase
- Preparing for component consolidation
- Identifying candidates for shared services or libraries

## Core Concepts

### Domain vs Infrastructure Functionality

**Domain Functionality** (candidates for consolidation):

- Business processing logic (notification, validation, auditing, formatting)
- Common to **some** processes, not all
- Examples: Customer notification, ticket auditing, data validation

**Infrastructure Functionality** (usually not consolidated here):

- Operational concerns (logging, metrics, security)
- Common to **all** processes
- Examples: Logging, authentication, database connections

### Common Domain Patterns

Common domain functionality often appears as:

1. **Namespace Patterns**: Components ending in same leaf node
   - `*.notification`, `*.audit`, `*.validation`, `*.formatting`
   - Example: `TicketNotification`, `BillingNotification`, `SurveyNotification`

2. **Shared Classes**: Same class used across multiple components
   - Example: `SMTPConnection` used by 5 different components
   - Example: `AuditLogger` used by multiple domain components

3. **Similar Functionality**: Different components doing similar things
   - Example: Multiple components sending emails with slight variations
   - Example: Multiple components writing audit logs

### Consolidation Approaches

**Shared Service**:

- Common functionality becomes a separate service
- Other components call this service
- Good for: Frequently changing logic, complex operations

**Shared Library**:

- Common code packaged as library (JAR, DLL, npm package)
- Components import and use the library
- Good for: Stable functionality, simple utilities

**Component Consolidation**:

- Merge multiple components into one
- Good for: Highly related functionality, low coupling impact

## Analysis Process

### Phase 1: Identify Common Namespace Patterns

Scan component namespaces for common leaf node names:

1. **Extract leaf nodes** from all component namespaces
   - Example: `services/billing/notification` → `notification`
   - Example: `services/ticket/notification` → `notification`

2. **Group by common leaf nodes**
   - Find components with same leaf node name
   - Example: All components ending in `.notification`

3. **Filter out infrastructure patterns**
   - Exclude: `.util`, `.helper`, `.common` (usually infrastructure)
   - Focus on: `.notification`, `.audit`, `.validation`, `.formatting`

**Example Output**:

```markdown
## Common Namespace Patterns Found

**Notification Components**:

- services/customer/notification
- services/ticket/notification
- services/survey/notification

**Audit Components**:

- services/billing/audit
- services/ticket/audit
- services/survey/audit
```

### Phase 2: Detect Shared Classes

Find classes/files used across multiple components:

1. **Scan imports/dependencies** in each component
   - Track which classes are imported from where
   - Note classes used by multiple components

2. **Identify shared classes**
   - Classes imported by 2+ components
   - Exclude infrastructure classes (Logger, Config, etc.)

3. **Classify as domain vs infrastructure**
   - Domain: Business logic classes (SMTPConnection, AuditLogger)
   - Infrastructure: Technical utilities (Logger, DatabaseConnection)

**Example Output**:

```markdown
## Shared Classes Found

**Domain Classes**:

- `SMTPConnection` - Used by 5 components (notification-related)
- `AuditLogger` - Used by 8 components (audit-related)
- `DataFormatter` - Used by 3 components (formatting-related)

**Infrastructure Classes** (exclude from consolidation):

- `Logger` - Used by all components (infrastructure)
- `Config` - Used by all components (infrastructure)
```

### Phase 3: Analyze Functionality Similarity

For each group of common components:

1. **Examine functionality**
   - Read source code of each component
   - Identify what each component does
   - Note similarities and differences

2. **Assess consolidation feasibility**
   - Are differences minor (configurable)?
   - Can differences be abstracted?
   - Is functionality truly the same?

3. **Calculate coupling impact**
   - Count incoming dependencies (afferent coupling) before consolidation
   - Estimate incoming dependencies after consolidation
   - Compare total coupling levels

**Example Analysis**:

```markdown
## Functionality Analysis

**Notification Components**:

- CustomerNotification: Sends billing notifications
- TicketNotification: Sends ticket assignment notifications
- SurveyNotification: Sends survey emails

**Similarities**: All send emails to customers
**Differences**: Email content/templates, triggers

**Consolidation Feasibility**: ✅ High

- Differences are in content, not mechanism
- Can be abstracted with templates/context
```

### Phase 4: Assess Coupling Impact

Before recommending consolidation, analyze coupling:

1. **Calculate current coupling**
   - Count components using each notification component
   - Sum total incoming dependencies

2. **Estimate consolidated coupling**
   - Count components that would use consolidated component
   - Compare to current total

3. **Evaluate coupling increase**
   - Is consolidated component too coupled?
   - Does it create a bottleneck?
   - Is coupling increase acceptable?

**Example Coupling Analysis**:

```markdown
## Coupling Impact Analysis

**Before Consolidation**:

- CustomerNotification: Used by 2 components (CA = 2)
- TicketNotification: Used by 2 components (CA = 2)
- SurveyNotification: Used by 1 component (CA = 1)
- **Total CA**: 5

**After Consolidation**:

- Notification: Used by 5 components (CA = 5)
- **Total CA**: 5 (same!)

**Verdict**: ✅ No coupling increase, safe to consolidate
```

### Phase 5: Recommend Consolidation Approach

Based on analysis, recommend approach:

**Shared Service** (if):

- Functionality changes frequently
- Complex operations
- Needs independent scaling
- Multiple deployment units will use it

**Shared Library** (if):

- Stable functionality
- Simple utilities
- Compile-time dependency acceptable
- No need for independent deployment

**Component Consolidation** (if):

- Highly related functionality
- Low coupling impact
- Same deployment unit acceptable

## Output Format

### Common Domain Components Report

```markdown
## Common Domain Components Found

### Notification Functionality

**Components**:

- services/customer/notification (2% - 1,433 statements)
- services/ticket/notification (2% - 1,765 statements)
- services/survey/notification (2% - 1,299 statements)

**Shared Classes**: SMTPConnection (used by all 3)

**Functionality Analysis**:

- All send emails to customers
- Differences: Content/templates, triggers
- Consolidation Feasibility: ✅ High

**Coupling Analysis**:

- Before: CA = 2 + 2 + 1 = 5
- After: CA = 5 (no increase)
- Verdict: ✅ Safe to consolidate

**Recommendation**: Consolidate into `services/notification`

- Approach: Shared Service
- Expected Size: ~4,500 statements (5% of codebase)
- Benefits: Reduced duplication, easier maintenance
```

### Consolidation Opportunities Table

```markdown
## Consolidation Opportunities

| Common Functionality | Components   | Current CA | After CA | Feasibility | Recommendation                |
| -------------------- | ------------ | ---------- | -------- | ----------- | ----------------------------- |
| Notification         | 3 components | 5          | 5        | ✅ High     | Consolidate to shared service |
| Audit                | 3 components | 8          | 12       | ⚠️ Medium   | Consolidate, monitor coupling |
| Validation           | 2 components | 3          | 3        | ✅ High     | Consolidate to shared library |
```

### Detailed Consolidation Plan

```markdown
## Consolidation Plan

### Priority: High

**Notification Components** → `services/notification`

**Steps**:

1. Create new `services/notification` component
2. Move common functionality from 3 components
3. Create abstraction for content/templates
4. Update dependent components to use new service
5. Remove old notification components

**Expected Impact**:

- Reduced code: ~4,500 statements consolidated
- Reduced duplication: 3 components → 1
- Coupling: No increase (CA stays at 5)
- Maintenance: Easier to maintain single component

### Priority: Medium

**Audit Components** → `services/audit`

**Steps**:
[Similar format]

**Expected Impact**:

- Coupling increase: CA 8 → 12 (monitor)
- Benefits: Reduced duplication
```

## Analysis Checklist

**Common Pattern Detection**:

- [ ] Scanned all component namespaces for common leaf nodes
- [ ] Identified components with same ending names
- [ ] Filtered out infrastructure patterns
- [ ] Grouped similar components

**Shared Class Detection**:

- [ ] Scanned imports/dependencies in each component
- [ ] Identified classes used by multiple components
- [ ] Classified as domain vs infrastructure
- [ ] Documented shared class usage

**Functionality Analysis**:

- [ ] Examined source code of common components
- [ ] Identified similarities and differences
- [ ] Assessed consolidation feasibility
- [ ] Determined if differences can be abstracted

**Coupling Assessment**:

- [ ] Calculated current coupling (CA) for each component
- [ ] Estimated consolidated coupling
- [ ] Compared total coupling levels
- [ ] Evaluated if coupling increase is acceptable

**Recommendations**:

- [ ] Suggested consolidation approach (service/library/merge)
- [ ] Prioritized recommendations by impact
- [ ] Created consolidation plan with steps
- [ ] Estimated expected benefits and risks

## Implementation Notes

### For Node.js/Express Applications

Common patterns to look for:

```
services/
├── CustomerService/
│   └── notification.js      ← Common pattern
├── TicketService/
│   └── notification.js     ← Common pattern
└── SurveyService/
    └── notification.js      ← Common pattern
```

**Shared Classes**:

- Check `require()` statements
- Look for classes imported from other components
- Example: `const SMTPConnection = require('../shared/SMTPConnection')`

### For Java Applications

Common patterns:

```
com.company.billing.audit     ← Common pattern
com.company.ticket.audit      ← Common pattern
com.company.survey.audit      ← Common pattern
```

**Shared Classes**:

- Check `import` statements
- Look for classes in common packages
- Example: `import com.company.shared.AuditLogger`

### Detection Strategies

**Namespace Pattern Detection**:

```javascript
// Extract leaf nodes from namespaces
function extractLeafNode(namespace) {
  const parts = namespace.split('/')
  return parts[parts.length - 1]
}

// Group by common leaf nodes
function groupByLeafNode(components) {
  const groups = {}
  components.forEach((comp) => {
    const leaf = extractLeafNode(comp.namespace)
    if (!groups[leaf]) groups[leaf] = []
    groups[leaf].push(comp)
  })
  return groups
}
```

**Shared Class Detection**:

```javascript
// Find classes used by multiple components
function findSharedClasses(components) {
  const classUsage = {}
  components.forEach((comp) => {
    comp.imports.forEach((imp) => {
      if (!classUsage[imp]) classUsage[imp] = []
      classUsage[imp].push(comp.name)
    })
  })

  return Object.entries(classUsage)
    .filter(([cls, users]) => users.length > 1)
    .map(([cls, users]) => ({ class: cls, usedBy: users }))
}
```

## Fitness Functions

After identifying common components, create automated checks:

### Common Namespace Pattern Detection

```javascript
// Alert if new components with common patterns are created
function checkCommonPatterns(components, exclusionList = []) {
  const leafNodes = {}
  components.forEach((comp) => {
    const leaf = extractLeafNode(comp.namespace)
    if (!exclusionList.includes(leaf)) {
      if (!leafNodes[leaf]) leafNodes[leaf] = []
      leafNodes[leaf].push(comp.name)
    }
  })

  return Object.entries(leafNodes)
    .filter(([leaf, comps]) => comps.length > 1)
    .map(([leaf, comps]) => ({
      pattern: leaf,
      components: comps,
      suggestion: 'Consider consolidating these components',
    }))
}
```

### Shared Class Usage Alert

```javascript
// Alert if class is used by multiple components
function checkSharedClasses(components, exclusionList = []) {
  const classUsage = {}
  components.forEach((comp) => {
    comp.imports.forEach((imp) => {
      if (!exclusionList.includes(imp)) {
        if (!classUsage[imp]) classUsage[imp] = []
        classUsage[imp].push(comp.name)
      }
    })
  })

  return Object.entries(classUsage)
    .filter(([cls, users]) => users.length > 1)
    .map(([cls, users]) => ({
      class: cls,
      usedBy: users,
      suggestion: 'Consider extracting to shared component or library',
    }))
}
```

## Best Practices

### Do's ✅

- Distinguish domain from infrastructure functionality
- Analyze coupling impact before consolidating
- Consider both shared service and shared library approaches
- Look for namespace patterns AND shared classes
- Verify functionality is truly similar before consolidating
- Calculate coupling metrics (CA) before and after

### Don'ts ❌

- Don't consolidate infrastructure functionality (handled separately)
- Don't consolidate without analyzing coupling impact
- Don't assume all common patterns should be consolidated
- Don't ignore differences in functionality
- Don't consolidate if coupling increase is too high
- Don't mix domain and infrastructure in same analysis

## Common Patterns to Look For

### High Consolidation Candidates

- **Notification**: `*.notification`, `*.notify`, `*.email`
- **Audit**: `*.audit`, `*.auditing`, `*.log`
- **Validation**: `*.validation`, `*.validate`, `*.validator`
- **Formatting**: `*.format`, `*.formatter`, `*.formatting`
- **Reporting**: `*.report`, `*.reporting` (if similar functionality)

### Low Consolidation Candidates

- **Infrastructure**: `*.util`, `*.helper`, `*.common` (usually infrastructure)
- **Different contexts**: Same name, different business meaning
- **High coupling risk**: Consolidation would create bottleneck

## Next Steps

After identifying common domain components:

1. **Apply Flatten Components Pattern** - Remove orphaned classes
2. **Apply Determine Component Dependencies Pattern** - Analyze coupling
3. **Create Component Domains** - Group components into domains
4. **Plan Consolidation** - Execute consolidation recommendations

## Notes

- Common domain functionality is different from infrastructure functionality
- Consolidation reduces duplication but may increase coupling
- Always analyze coupling impact before consolidating
- Shared services vs shared libraries have different trade-offs
- Some duplication is acceptable if it reduces coupling
- Not all common patterns should be consolidated

================================================================
File: packages/skills-catalog/skills/(architecture)/component-flattening-analysis/SKILL.md
================================================================
---
name: component-flattening-analysis
description: Detects misplaced classes and fixes component hierarchy problems — finds code that should belong inside a component but sits at the root level. Use when asking "clean up component structure", "find orphaned classes", "fix module hierarchy", "flatten nested components", or analyzing why namespaces have misplaced code. Do NOT use for dependency analysis (use coupling-analysis) or domain grouping (use domain-identification-grouping).
---

# Component Flattening Analysis

This skill identifies component hierarchy issues and ensures components exist only as leaf nodes in directory/namespace structures, removing orphaned classes from root namespaces.

## How to Use

### Quick Start

Request analysis of your codebase:

- **"Find orphaned classes in root namespaces"**
- **"Flatten component hierarchies"**
- **"Identify components that need flattening"**
- **"Analyze component structure for hierarchy issues"**

### Usage Examples

**Example 1: Find Orphaned Classes**

```
User: "Find orphaned classes in root namespaces"

The skill will:
1. Scan component namespaces for hierarchy issues
2. Identify orphaned classes in root namespaces
3. Detect components built on top of other components
4. Suggest flattening strategies
5. Create refactoring plan
```

**Example 2: Flatten Components**

```
User: "Flatten component hierarchies in this codebase"

The skill will:
1. Identify components with hierarchy issues
2. Analyze orphaned classes
3. Suggest consolidation or splitting strategies
4. Create refactoring plan
5. Estimate effort
```

**Example 3: Component Structure Analysis**

```
User: "Analyze component structure for hierarchy issues"

The skill will:
1. Map component namespace structure
2. Identify root namespaces with code
3. Find components built on components
4. Flag hierarchy violations
5. Provide recommendations
```

### Step-by-Step Process

1. **Scan Structure**: Map component namespace hierarchies
2. **Identify Issues**: Find orphaned classes and component nesting
3. **Analyze Options**: Determine flattening strategy (consolidate vs split)
4. **Create Plan**: Generate refactoring plan with steps
5. **Execute**: Refactor components to remove hierarchy

## When to Use

Apply this skill when:

- After gathering common domain components (Pattern 2)
- Before determining component dependencies (Pattern 4)
- When components have nested structures
- Finding orphaned classes in root namespaces
- Preparing for domain grouping
- Cleaning up component structure
- Ensuring components are leaf nodes only

## Core Concepts

### Component Definition

A **component** is identified by a **leaf node** in directory/namespace structure:

- **Leaf Node**: The deepest directory containing source files
- **Component**: Source code files in leaf node namespace
- **Subdomain**: Parent namespace that has been extended

**Key Rule**: Components exist only as leaf nodes. If a namespace is extended, the parent becomes a subdomain, not a component.

### Root Namespace

A **root namespace** is a namespace node that has been extended:

- **Extended**: Another namespace node added below it
- **Example**: `ss.survey` extended to `ss.survey.templates`
- **Result**: `ss.survey` becomes a root namespace (subdomain)

### Orphaned Classes

**Orphaned classes** are source files in root namespaces:

- **Location**: Root namespace (non-leaf node)
- **Problem**: No definable component associated with them
- **Solution**: Move to leaf node namespace (component)

**Example**:

```
ss.survey/              ← Root namespace (extended by .templates)
├── Survey.js           ← Orphaned class (in root namespace)
└── templates/          ← Component (leaf node)
    └── Template.js
```

### Flattening Strategies

**Strategy 1: Consolidate Down**

- Move code from leaf nodes into root namespace
- Makes root namespace the component
- Example: Move `ss.survey.templates` → `ss.survey`

**Strategy 2: Split Up**

- Move code from root namespace into new leaf nodes
- Creates new components from root namespace
- Example: Split `ss.survey` → `ss.survey.create` + `ss.survey.process`

**Strategy 3: Move Shared Code**

- Move shared code to dedicated component
- Creates `.shared` component
- Example: `ss.survey` shared code → `ss.survey.shared`

## Analysis Process

### Phase 1: Map Component Structure

Scan directory/namespace structure to identify hierarchy:

1. **Map Namespace Tree**
   - Build tree of all namespaces
   - Identify parent-child relationships
   - Mark leaf nodes (components)

2. **Identify Root Namespaces**
   - Find namespaces that have been extended
   - Mark as root namespaces (subdomains)
   - Note which namespaces extend them

3. **Locate Source Files**
   - Find all source files in each namespace
   - Map files to their namespace location
   - Identify files in root namespaces

**Example Structure Mapping**:

```markdown
## Component Structure Map
```

ss.survey/ ← Root namespace (extended)
├── Survey.js ← Orphaned class
├── SurveyProcessor.js ← Orphaned class
└── templates/ ← Component (leaf node)
├── EmailTemplate.js
└── SMSTemplate.js

ss.ticket/ ← Root namespace (extended)
├── Ticket.js ← Orphaned class
├── assign/ ← Component (leaf node)
│ └── TicketAssign.js
└── route/ ← Component (leaf node)
└── TicketRoute.js

```

```

### Phase 2: Identify Orphaned Classes

Find source files in root namespaces:

1. **Scan Root Namespaces**
   - Check each root namespace for source files
   - Identify files that are orphaned
   - Count orphaned files per root namespace

2. **Classify Orphaned Classes**
   - **Shared Code**: Common utilities, interfaces, abstract classes
   - **Domain Code**: Business logic that should be in component
   - **Mixed**: Combination of shared and domain code

3. **Assess Impact**
   - How many files are orphaned?
   - What functionality do they contain?
   - What components depend on them?

**Example Orphaned Class Detection**:

```markdown
## Orphaned Classes Found

### Root Namespace: ss.survey

**Orphaned Files** (5 files):

- Survey.js (domain code - survey creation)
- SurveyProcessor.js (domain code - survey processing)
- SurveyValidator.js (shared code - validation)
- SurveyFormatter.js (shared code - formatting)
- SurveyConstants.js (shared code - constants)

**Classification**:

- Domain Code: 2 files (should be in components)
- Shared Code: 3 files (should be in .shared component)

**Dependencies**: Used by ss.survey.templates component
```

### Phase 3: Analyze Flattening Options

Determine best flattening strategy for each root namespace:

1. **Option 1: Consolidate Down**
   - Move leaf node code into root namespace
   - Makes root namespace the component
   - **Use when**: Leaf nodes are small, related functionality

2. **Option 2: Split Up**
   - Move root namespace code into new leaf nodes
   - Creates multiple components from root
   - **Use when**: Root namespace has distinct functional areas

3. **Option 3: Move Shared Code**
   - Extract shared code to `.shared` component
   - Keep domain code in root or split
   - **Use when**: Root namespace has shared utilities

**Example Flattening Analysis**:

```markdown
## Flattening Options Analysis

### Root Namespace: ss.survey

**Current State**:

- Root namespace: 5 orphaned files
- Leaf component: ss.survey.templates (7 files)

**Option 1: Consolidate Down** ✅ Recommended

- Move templates code into ss.survey
- Result: Single component ss.survey
- Effort: Low (7 files to move)
- Rationale: Templates are small, related to survey functionality

**Option 2: Split Up**

- Create ss.survey.create (2 files)
- Create ss.survey.process (1 file)
- Create ss.survey.shared (3 files)
- Keep ss.survey.templates (7 files)
- Effort: High (multiple components to create)
- Rationale: More granular, but may be over-engineering

**Option 3: Move Shared Code**

- Create ss.survey.shared (3 shared files)
- Keep domain code in root (2 files)
- Keep ss.survey.templates (7 files)
- Effort: Medium
- Rationale: Separates shared from domain, but still has hierarchy
```

### Phase 4: Create Flattening Plan

Generate refactoring plan for each root namespace:

1. **Select Strategy**
   - Choose best flattening option
   - Consider effort, complexity, maintainability

2. **Plan Refactoring Steps**
   - List files to move
   - Identify target namespaces
   - Note dependencies to update

3. **Estimate Effort**
   - Time to refactor
   - Risk assessment
   - Testing requirements

**Example Flattening Plan**:

```markdown
## Flattening Plan

### Priority: High

**Root Namespace: ss.survey**

**Strategy**: Consolidate Down

**Steps**:

1. Move files from ss.survey.templates/ to ss.survey/
   - EmailTemplate.js
   - SMSTemplate.js
   - [5 more files]

2. Update imports in dependent components
   - Update references from ss.survey.templates._ to ss.survey._

3. Remove ss.survey.templates/ directory

4. Update namespace declarations
   - Change namespace from ss.survey.templates to ss.survey

5. Run tests to verify changes

**Effort**: 2-3 days
**Risk**: Low (templates are self-contained)
**Dependencies**: None
```

### Phase 5: Execute Flattening

Perform the refactoring:

1. **Move Files**
   - Move source files to target namespace
   - Update file paths and imports

2. **Update References**
   - Update imports in dependent components
   - Update namespace declarations
   - Update directory structure

3. **Verify Changes**
   - Run tests
   - Check for broken references
   - Validate component structure

## Output Format

### Orphaned Classes Report

```markdown
## Orphaned Classes Analysis

### Root Namespace: ss.survey

**Status**: ⚠️ Has Orphaned Classes

**Orphaned Files** (5 files):

- Survey.js (domain code)
- SurveyProcessor.js (domain code)
- SurveyValidator.js (shared code)
- SurveyFormatter.js (shared code)
- SurveyConstants.js (shared code)

**Leaf Components**:

- ss.survey.templates (7 files)

**Issue**: Root namespace contains code but is extended by leaf component

**Recommendation**: Consolidate templates into root namespace
```

### Component Hierarchy Issues

```markdown
## Component Hierarchy Issues

| Root Namespace | Orphaned Files | Leaf Components                 | Issue                | Recommendation   |
| -------------- | -------------- | ------------------------------- | -------------------- | ---------------- |
| ss.survey      | 5              | 1 (templates)                   | Has orphaned classes | Consolidate down |
| ss.ticket      | 45             | 2 (assign, route)               | Large orphaned code  | Split up         |
| ss.reporting   | 0              | 3 (tickets, experts, financial) | No issue             | ✅ OK            |
```

### Flattening Plan

```markdown
## Flattening Plan

### Priority: High

**ss.survey** → Consolidate Down

- Move 7 files from templates to root
- Effort: 2-3 days
- Risk: Low

### Priority: Medium

**ss.ticket** → Split Up

- Create ss.ticket.maintenance (30 files)
- Create ss.ticket.completion (10 files)
- Create ss.ticket.shared (5 files)
- Effort: 1 week
- Risk: Medium
```

## Analysis Checklist

**Structure Mapping**:

- [ ] Mapped all namespace hierarchies
- [ ] Identified root namespaces
- [ ] Located all source files
- [ ] Marked leaf nodes (components)

**Orphaned Class Detection**:

- [ ] Scanned root namespaces for source files
- [ ] Identified orphaned classes
- [ ] Classified orphaned classes (shared/domain/mixed)
- [ ] Assessed impact and dependencies

**Flattening Analysis**:

- [ ] Analyzed consolidation option
- [ ] Analyzed splitting option
- [ ] Analyzed shared code extraction option
- [ ] Selected best strategy for each root namespace

**Plan Creation**:

- [ ] Selected flattening strategy
- [ ] Created refactoring steps
- [ ] Estimated effort and risk
- [ ] Prioritized work

**Execution**:

- [ ] Moved files to target namespaces
- [ ] Updated imports and references
- [ ] Updated namespace declarations
- [ ] Verified changes with tests

## Implementation Notes

### For Node.js/Express Applications

Components typically in `services/` directory:

```
services/
├── survey/              ← Root namespace (extended)
│   ├── Survey.js       ← Orphaned class
│   └── templates/      ← Component (leaf node)
│       └── Template.js
```

**Flattening**:

- Consolidate: Move `templates/` files to `survey/`
- Split: Create `survey/create/` and `survey/process/`
- Shared: Create `survey/shared/` for utilities

### For Java Applications

Components identified by package structure:

```
com.company.survey       ← Root package (extended)
├── Survey.java         ← Orphaned class
└── templates/          ← Component (leaf package)
    └── Template.java
```

**Flattening**:

- Consolidate: Move `templates` classes to `survey` package
- Split: Create `survey.create` and `survey.process` packages
- Shared: Create `survey.shared` package

### Detection Strategies

**Find Root Namespaces with Code**:

```javascript
// Find root namespaces containing source files
function findRootNamespacesWithCode(namespaces, sourceFiles) {
  const rootNamespaces = namespaces.filter((ns) => {
    // Check if namespace has been extended
    const hasChildren = namespaces.some((n) => n.startsWith(ns + '.') || n.startsWith(ns + '/'))

    // Check if namespace contains source files
    const hasFiles = sourceFiles.some((f) => f.namespace === ns)

    return hasChildren && hasFiles
  })

  return rootNamespaces
}
```

**Find Orphaned Classes**:

```javascript
// Find orphaned classes in root namespaces
function findOrphanedClasses(rootNamespaces, sourceFiles) {
  const orphaned = []

  rootNamespaces.forEach((rootNs) => {
    const files = sourceFiles.filter((f) => f.namespace === rootNs)
    orphaned.push({
      rootNamespace: rootNs,
      files: files,
      count: files.length,
    })
  })

  return orphaned
}
```

## Fitness Functions

After flattening components, create automated checks:

### No Source Code in Root Namespaces

```javascript
// Alert if source code exists in root namespace
function checkRootNamespaceCode(namespaces, sourceFiles) {
  const violations = []

  namespaces.forEach((ns) => {
    // Check if namespace has been extended
    const hasChildren = namespaces.some((n) => n.startsWith(ns + '.') || n.startsWith(ns + '/'))

    if (hasChildren) {
      // Check if namespace contains source files
      const files = sourceFiles.filter((f) => f.namespace === ns)

      if (files.length > 0) {
        violations.push({
          namespace: ns,
          files: files.map((f) => f.name),
          issue: 'Root namespace contains source files (orphaned classes)',
        })
      }
    }
  })

  return violations
}
```

### Components Only as Leaf Nodes

```javascript
// Ensure components exist only as leaf nodes
function validateComponentStructure(namespaces, sourceFiles) {
  const violations = []

  // Find all leaf nodes (components)
  const leafNodes = namespaces.filter((ns) => {
    return !namespaces.some((n) => n.startsWith(ns + '.') || n.startsWith(ns + '/'))
  })

  // Check that all source files are in leaf nodes
  sourceFiles.forEach((file) => {
    if (!leafNodes.includes(file.namespace)) {
      violations.push({
        file: file.name,
        namespace: file.namespace,
        issue: 'Source file not in leaf node (component)',
      })
    }
  })

  return violations
}
```

## Best Practices

### Do's ✅

- Ensure components exist only as leaf nodes
- Remove orphaned classes from root namespaces
- Choose flattening strategy based on functionality
- Consolidate when functionality is related
- Split when functionality is distinct
- Extract shared code to `.shared` components
- Update all references after flattening
- Verify changes with tests

### Don'ts ❌

- Don't leave orphaned classes in root namespaces
- Don't create components on top of other components
- Don't skip updating imports after moving files
- Don't flatten without analyzing impact
- Don't mix flattening strategies inconsistently
- Don't ignore shared code when flattening
- Don't skip testing after refactoring

## Common Patterns

### Pattern 1: Simple Consolidation

**Before**:

```
ss.survey/
├── Survey.js           ← Orphaned
└── templates/          ← Component
    └── Template.js
```

**After**:

```
ss.survey/              ← Component (leaf node)
├── Survey.js
└── Template.js
```

### Pattern 2: Functional Split

**Before**:

```
ss.ticket/              ← Root namespace
├── Ticket.js           ← Orphaned (45 files)
├── assign/             ← Component
└── route/              ← Component
```

**After**:

```
ss.ticket/              ← Subdomain
├── maintenance/        ← Component
│   └── Ticket.js
├── completion/        ← Component
│   └── TicketCompletion.js
├── assign/             ← Component
└── route/              ← Component
```

### Pattern 3: Shared Code Extraction

**Before**:

```
ss.survey/              ← Root namespace
├── Survey.js           ← Domain code
├── SurveyValidator.js  ← Shared code
└── templates/          ← Component
```

**After**:

```
ss.survey/              ← Component
├── Survey.js
└── shared/             ← Component
    └── SurveyValidator.js
```

## Next Steps

After flattening components:

1. **Apply Determine Component Dependencies Pattern** - Analyze coupling
2. **Create Component Domains** - Group components into domains
3. **Create Domain Services** - Extract domains to services

## Notes

- Components must exist only as leaf nodes
- Root namespaces with code are problematic
- Flattening improves component clarity
- Choose flattening strategy based on functionality
- Shared code should be in dedicated components
- Always update references after moving files
- Test thoroughly after flattening

================================================================
File: packages/skills-catalog/skills/(architecture)/component-identification-sizing/SKILL.md
================================================================
---
name: component-identification-sizing
description: Maps architectural components in a codebase and measures their size to identify what should be extracted first. Use when asking "how big is each module?", "what components do I have?", "which service is too large?", "analyze codebase structure", "size my monolith", or planning where to start decomposing. Do NOT use for runtime performance sizing or infrastructure capacity planning.
---

# Component Identification and Sizing

This skill identifies architectural components (logical building blocks) in a codebase and calculates size metrics to assess decomposition feasibility and identify oversized components.

## How to Use

### Quick Start

Request analysis of your codebase:

- **"Identify and size all components in this codebase"**
- **"Find oversized components that need splitting"**
- **"Create a component inventory for decomposition planning"**
- **"Analyze component size distribution"**

### Usage Examples

**Example 1: Complete Analysis**

```
User: "Identify and size all components in this codebase"

The skill will:
1. Map directory/namespace structures
2. Identify all components (leaf nodes)
3. Calculate size metrics (statements, files, percentages)
4. Generate component inventory table
5. Flag oversized/undersized components
6. Provide recommendations
```

**Example 2: Find Oversized Components**

```
User: "Which components are too large?"

The skill will:
1. Calculate mean and standard deviation
2. Identify components >2 std dev or >10% threshold
3. Analyze functional areas within large components
4. Suggest specific splits with estimated sizes
```

**Example 3: Component Size Analysis**

```
User: "Analyze component sizes and distribution"

The skill will:
1. Calculate all size metrics
2. Generate size distribution summary
3. Identify outliers
4. Provide statistics and recommendations
```

### Step-by-Step Process

1. **Initial Analysis**: Start with complete component inventory
2. **Identify Issues**: Find components that need attention
3. **Get Recommendations**: Request actionable split/consolidation suggestions
4. **Monitor Progress**: Track component growth over time

## When to Use

Apply this skill when:

- Starting a monolithic decomposition effort
- Assessing codebase structure and organization
- Identifying components that are too large or too small
- Creating component inventory for migration planning
- Analyzing code distribution across components
- Preparing for component-based decomposition patterns

## Core Concepts

### Component Definition

A **component** is an architectural building block that:

- Has a well-defined role and responsibility
- Is identified by a namespace, package structure, or directory path
- Contains source code files (classes, functions, modules) grouped together
- Performs specific business or infrastructure functionality

**Key Rule**: Components are identified by **leaf nodes** in directory/namespace structures. If a namespace is extended (e.g., `services/billing` extended to `services/billing/payment`), the parent becomes a **subdomain**, not a component.

### Size Metrics

**Statements** (not lines of code):

- Count executable statements terminated by semicolons or newlines
- More accurate than lines of code for size comparison
- Accounts for code complexity, not formatting

**Component Size Indicators**:

- **Percent of codebase**: Component statements / Total statements
- **File count**: Number of source files in component
- **Standard deviation**: Distance from mean component size

## Analysis Process

### Phase 1: Identify Components

Scan the codebase directory structure:

1. **Map directory/namespace structure**
   - For Node.js: `services/`, `routes/`, `models/`, `utils/`
   - For Java: Package structure (e.g., `com.company.domain.service`)
   - For Python: Module paths (e.g., `app/billing/payment`)

2. **Identify leaf nodes**
   - Components are the deepest directories containing source files
   - Example: `services/BillingService/` is a component
   - Example: `services/BillingService/payment/` extends it, making `BillingService` a subdomain

3. **Create component inventory**
   - List each component with its namespace/path
   - Note any parent namespaces (subdomains)

### Phase 2: Calculate Size Metrics

For each component:

1. **Count statements**
   - Parse source files in component directory
   - Count executable statements (not comments, blank lines, or declarations alone)
   - Sum across all files in component

2. **Count files**
   - Total source files (`.js`, `.ts`, `.java`, `.py`, etc.)
   - Exclude test files, config files, documentation

3. **Calculate percentage**

   ```
   component_percent = (component_statements / total_statements) * 100
   ```

4. **Calculate statistics**
   - Mean component size: `total_statements / number_of_components`
   - Standard deviation: `sqrt(sum((size - mean)^2) / (n - 1))`
   - Component's deviation: `(component_size - mean) / std_dev`

### Phase 3: Identify Size Issues

**Oversized Components** (candidates for splitting):

- Exceeds 30% of total codebase (for small apps with <10 components)
- Exceeds 10% of total codebase (for large apps with >20 components)
- More than 2 standard deviations above mean
- Contains multiple distinct functional areas

**Undersized Components** (candidates for consolidation):

- Less than 1% of codebase (may be too granular)
- Less than 1 standard deviation below mean
- Contains only a few files with minimal functionality

**Well-Sized Components**:

- Between 1-2 standard deviations from mean
- Represents a single, cohesive functional area
- Appropriate percentage for application size

## Output Format

### Component Inventory Table

```markdown
## Component Inventory

| Component Name  | Namespace/Path               | Statements | Files | Percent | Status       |
| --------------- | ---------------------------- | ---------- | ----- | ------- | ------------ |
| Billing Payment | services/BillingService      | 4,312      | 23    | 5%      | ✅ OK        |
| Reporting       | services/ReportingService    | 27,765     | 162   | 33%     | ⚠️ Too Large |
| Notification    | services/NotificationService | 1,433      | 7     | 2%      | ✅ OK        |
```

**Status Legend**:

- ✅ OK: Well-sized (within 1-2 std dev from mean)
- ⚠️ Too Large: Exceeds size threshold or >2 std dev above mean
- 🔍 Too Small: <1% of codebase or <1 std dev below mean

### Size Analysis Summary

```markdown
## Size Analysis Summary

**Total Components**: 18
**Total Statements**: 82,931
**Mean Component Size**: 4,607 statements
**Standard Deviation**: 5,234 statements

**Oversized Components** (>2 std dev or >10%):

- Reporting (33% - 27,765 statements) - Consider splitting into:
  - Ticket Reports
  - Expert Reports
  - Financial Reports

**Well-Sized Components** (within 1-2 std dev):

- Billing Payment (5%)
- Customer Profile (5%)
- Ticket Assignment (9%)

**Undersized Components** (<1 std dev):

- Login (2% - 1,865 statements) - Consider consolidating with Authentication
```

### Component Size Distribution

```markdown
## Component Size Distribution
```

Component Size Distribution (by percent of codebase)

[Visual representation or histogram if possible]

Largest: ████████████████████████████████████ 33% (Reporting)
████████ 9% (Ticket Assign)
██████ 8% (Ticket)
██████ 6% (Expert Profile)
█████ 5% (Billing Payment)
████ 4% (Billing History)
...

````

### Recommendations

```markdown
## Recommendations

### High Priority: Split Large Components

**Reporting Component** (33% of codebase):
- **Current**: Single component with 27,765 statements
- **Issue**: Too large, contains multiple functional areas
- **Recommendation**: Split into:
  1. Reporting Shared (common utilities)
  2. Ticket Reports (ticket-related reports)
  3. Expert Reports (expert-related reports)
  4. Financial Reports (financial reports)
- **Expected Result**: Each component ~7-9% of codebase

### Medium Priority: Review Small Components

**Login Component** (2% of codebase):
- **Current**: 1,865 statements, 3 files
- **Consideration**: May be too granular if related to broader authentication
- **Recommendation**: Evaluate if should be consolidated with Authentication/User components

### Low Priority: Monitor Well-Sized Components

Most components are appropriately sized. Continue monitoring during decomposition.
````

## Analysis Checklist

**Component Identification**:

- [ ] Mapped all directory/namespace structures
- [ ] Identified leaf nodes (components) vs parent nodes (subdomains)
- [ ] Created complete component inventory
- [ ] Documented namespace/path for each component

**Size Calculation**:

- [ ] Counted statements (not lines) for each component
- [ ] Counted source files (excluding tests/configs)
- [ ] Calculated percentage of total codebase
- [ ] Calculated mean and standard deviation

**Size Assessment**:

- [ ] Identified oversized components (>threshold or >2 std dev)
- [ ] Identified undersized components (<1% or <1 std dev)
- [ ] Flagged components for splitting or consolidation
- [ ] Documented size distribution

**Recommendations**:

- [ ] Suggested splits for oversized components
- [ ] Suggested consolidations for undersized components
- [ ] Prioritized recommendations by impact
- [ ] Created architecture stories for refactoring

## Implementation Notes

### For Node.js/Express Applications

Components typically found in:

- `services/` - Business logic components
- `routes/` - API endpoint components
- `models/` - Data model components
- `utils/` - Utility components
- `middleware/` - Middleware components

**Example Component Identification**:

```
services/
├── BillingService/          ← Component (leaf node)
│   ├── index.js
│   └── BillingService.js
├── CustomerService/          ← Component (leaf node)
│   └── CustomerService.js
└── NotificationService/      ← Component (leaf node)
    └── NotificationService.js
```

### For Java Applications

Components identified by package structure:

- `com.company.domain.service` - Service components
- `com.company.domain.model` - Model components
- `com.company.domain.repository` - Repository components

**Example Component Identification**:

```
com.company.billing.payment   ← Component (leaf package)
com.company.billing.history   ← Component (leaf package)
com.company.billing           ← Subdomain (parent of payment/history)
```

### Statement Counting

**JavaScript/TypeScript**:

- Count statements terminated by `;` or newline
- Include: assignments, function calls, returns, conditionals, loops
- Exclude: comments, blank lines, declarations without assignment

**Java**:

- Count statements terminated by `;`
- Include: method calls, assignments, returns, conditionals
- Exclude: class/interface declarations, comments, blank lines

**Python**:

- Count executable statements (not comments or blank lines)
- Include: assignments, function calls, returns, conditionals
- Exclude: docstrings, comments, blank lines

## Fitness Functions

After identifying and sizing components, create automated checks:

### Component Size Threshold

```javascript
// Alert if any component exceeds 10% of codebase
function checkComponentSize(components, threshold = 0.1) {
  const totalStatements = components.reduce((sum, c) => sum + c.statements, 0)
  return components
    .filter((c) => c.statements / totalStatements > threshold)
    .map((c) => ({
      component: c.name,
      percent: ((c.statements / totalStatements) * 100).toFixed(1),
      issue: 'Exceeds size threshold',
    }))
}
```

### Standard Deviation Check

```javascript
// Alert if component is >2 standard deviations from mean
function checkStandardDeviation(components) {
  const sizes = components.map((c) => c.statements)
  const mean = sizes.reduce((a, b) => a + b, 0) / sizes.length
  const stdDev = Math.sqrt(sizes.reduce((sum, size) => sum + Math.pow(size - mean, 2), 0) / (sizes.length - 1))

  return components
    .filter((c) => Math.abs(c.statements - mean) > 2 * stdDev)
    .map((c) => ({
      component: c.name,
      deviation: ((c.statements - mean) / stdDev).toFixed(2),
      issue: 'More than 2 standard deviations from mean',
    }))
}
```

## Best Practices

### Do's ✅

- Use statements, not lines of code
- Identify components as leaf nodes only
- Calculate both percentage and standard deviation
- Consider application size when setting thresholds
- Document namespace/path for each component
- Create visual size distribution if possible

### Don'ts ❌

- Don't count test files in component size
- Don't treat parent directories as components
- Don't use fixed thresholds without considering app size
- Don't ignore small components (may need consolidation)
- Don't skip standard deviation calculation
- Don't mix infrastructure and domain components in same analysis

## Next Steps

After completing component identification and sizing:

1. **Apply Gather Common Domain Components Pattern** - Identify duplicate functionality
2. **Apply Flatten Components Pattern** - Remove orphaned classes from root namespaces
3. **Apply Determine Component Dependencies Pattern** - Analyze coupling between components
4. **Create Component Domains** - Group components into logical domains

## Notes

- Component size thresholds vary by application size
- Small apps (<10 components): 30% threshold may be appropriate
- Large apps (>20 components): 10% threshold is more appropriate
- Standard deviation is more reliable than fixed percentages
- Well-sized components are 1-2 standard deviations from mean
- Oversized components often contain multiple functional areas that can be split

================================================================
File: packages/skills-catalog/skills/(architecture)/coupling-analysis/SKILL.md
================================================================
---
name: coupling-analysis
description: Analyzes coupling between modules using the three-dimensional model (strength, distance, volatility) from "Balancing Coupling in Software Design". Use when asking "are these modules too coupled?", "show me dependencies", "analyze integration quality", "which modules should I decouple?", "coupling report", or evaluating architectural health. Do NOT use for domain boundary analysis (use domain-analysis) or component sizing (use component-identification-sizing).
---

# Coupling Analysis Skill

You are an expert software architect specializing in coupling analysis. You analyze codebases following the **three-dimensional model** from _Balancing Coupling in Software Design_ (Vlad Khononov):

1. **Integration Strength** — _what_ is shared between components
2. **Distance** — _where_ the coupling physically lives
3. **Volatility** — _how often_ components change

The guiding balance formula:

```
BALANCE = (STRENGTH XOR DISTANCE) OR NOT VOLATILITY
```

A design is **balanced** when:

- Tightly coupled components are close together (high strength + low distance = cohesion)
- Distant components are loosely coupled (low strength + high distance = loose coupling)
- Stable components (low volatility) can tolerate stronger coupling

## When to Use

Apply this skill when the user:

- Asks to "analyze coupling", "evaluate architecture", or "check dependencies"
- Wants to understand integration strength between modules or services
- Needs to identify problematic coupling or architectural smell
- Wants to know if a module should be extracted or merged
- References concepts like connascence, cohesion, or coupling from Khononov's book
- Asks why changes in one module cascade to others unexpectedly

## Process

### PHASE 1 — Context Gathering

Before analyzing code, collect:

**1.1 Scope**

- Full codebase or a specific area?
- Primary level of abstraction: methods, classes, modules/packages, services?
- Is git history available? (useful to estimate volatility)

**1.2 Business context** — ask the user or infer from code:

- Which parts are the business "core" (competitive differentiator)?
- Which are infrastructure/generic support (auth, billing, logging)?
- What changes most frequently according to the team?

This allows classifying **subdomains** (critical for volatility):
| Type | Volatility | Indicators |
|------|-----------|------------|
| **Core subdomain** | High | Proprietary logic, competitive advantage, area the business most wants to evolve |
| **Supporting subdomain** | Low | Simple CRUD, core support, no algorithmic complexity |
| **Generic subdomain** | Minimal | Auth, billing, email, logging, storage |

---

### PHASE 2 — Structural Mapping

**2.1 Module inventory**

For each module, record:

- Name and location (namespace/package/path)
- Primary responsibility
- Declared dependencies (imports, DI, HTTP calls)

**2.2 Dependency graph**

Build a directed graph where:

- Nodes = modules
- Edges = dependencies (A → B means "A depends on B")
- Note: the flow of _knowledge_ is OPPOSITE to the dependency arrow
  - If A → B, then B is _upstream_ and exposes knowledge to A (downstream)

**2.3 Distance calculation**

Use the encapsulation hierarchy to measure distance. The nearest common ancestor determines distance:

| Common ancestor level  | Distance | Example                        |
| ---------------------- | -------- | ------------------------------ |
| Same method/function   | Minimal  | Two lines in same method       |
| Same object/class      | Very low | Methods on same object         |
| Same namespace/package | Low      | Classes in same package        |
| Same library/module    | Medium   | Libs in same project           |
| Different services     | High     | Distinct microservices         |
| Different systems/orgs | Maximum  | External APIs, different teams |

**Social factor**: If modules are maintained by different teams, increase the estimated distance by one level (Conway's Law).

---

### PHASE 3 — Integration Strength Analysis

For each dependency in the graph, classify the **Integration Strength** level (strongest to weakest):

#### INTRUSIVE COUPLING (Strongest — Avoid)

Downstream accesses implementation details of upstream that were _not designed for integration_.

**Code signals**:

- Reflection to access private members
- Service directly reading another service's database
- Dependency on internal file/config structure of another module
- Monkey-patching of internals (Python/Ruby)
- Direct access to internal fields without getter

**Effect**: Any internal change to upstream (even without changing public interface) breaks downstream. Upstream doesn't know it's being observed.

---

#### FUNCTIONAL COUPLING (Second strongest)

Modules implement interrelated functionalities — shared business logic, interdependent rules, or coupled workflows.

**Three degrees (weakest to strongest)**:

**a) Sequential (Temporal)** — modules must execute in specific order

```python
connection.open()   # must come first
connection.query()  # depends on open
connection.close()  # must come last
```

**b) Transactional** — operations must succeed or fail together

```python
with transaction:
    service_a.update(data)
    service_b.update(data)  # both must succeed
```

**c) Symmetric (strongest)** — same business logic duplicated in multiple modules

```python
# Module A
def is_premium_customer(c): return c.purchases > 1000

# Module B — duplicated rule! Must stay in sync
def qualifies_for_discount(c): return c.purchases > 1000
```

Note: symmetric coupling does NOT require modules to reference each other — they can be fully independent in code yet still have this coupling.

**General signals of Functional Coupling**:

- Comments like "remember to update X when changing Y"
- Cascading test failures when a business rule changes
- Duplicated validation logic in multiple places
- Need to deploy multiple services simultaneously for a feature

---

#### MODEL COUPLING (Third level)

Upstream exposes its internal domain model as part of the public interface. Downstream knows and uses objects representing the upstream's internal model.

**Code signals**:

```python
# Analysis module uses Customer from CRM directly
from crm.models import Customer  # CRM's internal model

class Analysis:
    def process(self, customer_id):
        customer = crm_repo.get(customer_id)  # returns full Customer
        status = customer.status  # only needs status, but knows everything
```

```typescript
// Service B consuming Service A's internal model via API
interface CustomerFromServiceA {
  internalAccountCode: string; // internal detail exposed
  legacyId: number; // unnecessary internal field
  // ... many fields Service B doesn't need
}
```

**Degrees** (via static connascence):

- _connascence of name_: knows field names of the model
- _connascence of type_: knows specific types of the model
- _connascence of meaning_: interprets specific values (magic numbers, internal enums)
- _connascence of algorithm_: must use same algorithm to interpret data
- _connascence of position_: depends on element order (tuples, unnamed arrays)

---

#### CONTRACT COUPLING (Weakest — Ideal)

Upstream exposes an _integration-specific model_ (contract), separate from its internal model. The contract abstracts implementation details.

**Code signals**:

```python
class CustomerSnapshot:  # integration DTO, not the internal model
    """Public integration contract — stable and intentional."""
    id: str
    status: str  # enum converted to string
    tier: str    # only what consumers need

    @staticmethod
    def from_customer(customer: Customer) -> 'CustomerSnapshot':
        return CustomerSnapshot(
            id=str(customer.id),
            status=customer.status.value,
            tier=customer.loyalty_tier.display_name
        )
```

**Characteristics of good Contract Coupling**:

- Dedicated DTOs/ViewModels per use case (not the domain model)
- Versionable contracts (V1, V2)
- Primitive types or simple value types
- Explicit contract documentation (OpenAPI, Protobuf, etc.)
- Patterns: Facade, Adapter, Anti-Corruption Layer, Published Language (DDD)

---

### PHASE 4 — Volatility Assessment

For each module, estimate volatility based on:

**4.1 Subdomain type** (preferred) — see table in Phase 1

**4.2 Git analysis** (when available):

```bash
# Commits per file in the last 6 months
git log --since="6 months ago" --format="" --name-only | sort | uniq -c | sort -rn | head -20

# Files that change together frequently (temporal coupling)
# High co-change = possible undeclared functional coupling
```

**4.3 Code signals**:

- Many TODO/FIXME → area under evolution (higher volatility)
- Many API versions (V1, V2, V3) → frequently changing area
- Fragile tests that break constantly → volatile area
- Comments "business rule: ..." → business logic = probably core

**4.4 Inferred volatility**

Even a supporting subdomain module may have high volatility if:

- It has Intrusive or Functional coupling with core subdomain modules
- Changes in core propagate to it frequently

---

### PHASE 5 — Balance Score Calculation

For each coupled pair (A → B):

**Simplified scale (0 = low, 1 = high)**:

| Dimension  | 0 (Low)                      | 1 (High)           |
| ---------- | ---------------------------- | ------------------ |
| Strength   | Contract coupling            | Intrusive coupling |
| Distance   | Same object/namespace        | Different services |
| Volatility | Generic/Supporting subdomain | Core subdomain     |

**Maintenance effort formula**:

```
MAINTENANCE_EFFORT = STRENGTH × DISTANCE × VOLATILITY
```

(0 in any dimension = low effort)

**Classification table**:

| Strength | Distance | Volatility | Diagnosis                                                        |
| -------- | -------- | ---------- | ---------------------------------------------------------------- |
| High     | High     | High       | 🔴 **CRITICAL** — Global complexity + high change cost           |
| High     | High     | Low        | 🟡 **ACCEPTABLE** — Strong but stable (e.g. legacy integration)  |
| High     | Low      | High       | 🟢 **GOOD** — High cohesion (change together, live together)     |
| High     | Low      | Low        | 🟢 **GOOD** — Strong but static                                  |
| Low      | High     | High       | 🟢 **GOOD** — Loose coupling (separate and independent)          |
| Low      | High     | Low        | 🟢 **GOOD** — Loose coupling and stable                          |
| Low      | Low      | High       | 🟠 **ATTENTION** — Local complexity (mixes unrelated components) |
| Low      | Low      | Low        | 🟡 **ACCEPTABLE** — May generate noise, but low cost             |

---

### PHASE 6 — Analysis Report

Structure the report in sections:

#### 6.1 Executive Summary

```
CODEBASE: [name]
MODULES ANALYZED: N
DEPENDENCIES MAPPED: N
CRITICAL ISSUES: N
MODERATE ISSUES: N

OVERALL HEALTH SCORE: [Healthy / Attention / Critical]
```

#### 6.2 Dependency Map

Present the annotated graph:

```
[ModuleA] --[INTRUSIVE]-----------> [ModuleB]
[ModuleC] --[CONTRACT]------------> [ModuleD]
[ModuleE] --[FUNCTIONAL:symmetric]-> [ModuleF]
```

#### 6.3 Identified Issues (by severity)

For each critical or moderate issue:

```
ISSUE: [descriptive name]
────────────────────────────────────────
Modules involved: A → B
Coupling type: Functional Coupling (symmetric)
Connascence level: Connascence of Value

Evidence in code:
  [snippet or description of found pattern]

Dimensions:
  • Strength:   HIGH  (Functional - symmetric)
  • Distance:   HIGH  (separate services)
  • Volatility: HIGH  (core subdomain)

Balance Score: CRITICAL 🔴
Maintenance: High — frequent changes propagate over long distance

Impact: Any change to business rule [X] requires simultaneous
        update in [A] and [B], which belong to different teams.

Recommendation:
  → Extract shared logic to a dedicated module that both can
    reference (DRY + contract coupling)
  → Or: Accept duplication and explicitly document the coupling
    (if volatility is lower than it appears)
```

#### 6.4 Positive Patterns Found

```
✅ [ModuleX] uses dedicated integration DTOs — contract coupling well implemented
✅ [ServiceY] exposes only necessary data via API — minimizes model coupling
✅ [PackageZ] encapsulates its internal model well — low implementation leakage
```

#### 6.5 Prioritized Recommendations

**High priority** (high impact, blocking evolution):

1. ...

**Medium priority** (improve architectural health): 2. ...

**Low priority** (incremental improvements): 3. ...

---

## Quick Reference: Pattern → Integration Strength

| Pattern found                        | Integration Strength       | Action                               |
| ------------------------------------ | -------------------------- | ------------------------------------ |
| Reflection to access private members | Intrusive                  | Refactor urgently                    |
| Reading another service's DB         | Intrusive                  | Refactor urgently                    |
| Duplicated business logic            | Functional (symmetric)     | Extract to shared module             |
| Distributed transaction / Saga       | Functional (transactional) | Evaluate if cohesion would be better |
| Mandatory execution order            | Functional (sequential)    | Document protocol or encapsulate     |
| Rich domain object returned          | Model coupling             | Create integration DTO               |
| Internal enum shared externally      | Model coupling             | Create public contract enum          |
| Use-case-specific DTO                | Contract coupling          | ✅ Correct pattern                   |
| Versioned public interface/protocol  | Contract coupling          | ✅ Correct pattern                   |
| Anti-Corruption Layer                | Contract coupling          | ✅ Correct pattern                   |

## Quick Heuristics

**For Integration Strength**:

- "If I change an internal detail of module X, how many other modules need to change?"
- "Was the integration contract designed to be public, or is it accidental?"
- "Is there duplicated business logic that must be manually synchronized?"

**For Distance**:

- "What's the cost of making a change that affects both modules?"
- "Do teams maintaining these modules need to coordinate deployments?"
- "If one module fails, does the other stop working?"

**For Volatility**:

- "Does this module encapsulate competitive business advantage?"
- "Does the business team frequently request changes in this area?"
- "Is there a history of many refactors in this area?"

**For Balance**:

- "Do components that need to change together live together in the code?"
- "Are independent components well separated?"
- "Where is there strong coupling with volatile and distant components?" (→ this is the main problem)

## Known Limitations

- **Volatility** is best estimated with real git data rather than static analysis alone
- **Symmetric functional coupling** requires semantic code reading — static analysis tools generally don't detect it
- **Organizational distance** (different teams) requires user input
- **Dynamic connascence** (timing, value, identity) is hard to detect without runtime observation
- Analysis is a starting point — business context always refines the conclusions

## Book References

These concepts are based on _Balancing Coupling in Software Design_ by Vlad Khononov (Addison-Wesley).

================================================================
File: packages/skills-catalog/skills/(architecture)/decomposition-planning-roadmap/SKILL.md
================================================================
---
name: decomposition-planning-roadmap
description: Creates step-by-step decomposition plans and migration roadmaps for breaking apart monolithic applications. Use when asking "what order should I extract services?", "plan my migration", "create a decomposition roadmap", "prioritize what to split", "monolith to microservices strategy", or tracking decomposition progress. Do NOT use for domain analysis (use domain-analysis) or component sizing (use component-identification-sizing).
---

# Decomposition Planning and Roadmap

This skill creates structured decomposition plans and roadmaps to guide the migration from monolithic to distributed architectures, prioritizing work and tracking progress through decomposition patterns.

## How to Use

### Quick Start

Request creation of a decomposition plan:

- **"Create a decomposition roadmap for this codebase"**
- **"Plan the decomposition migration strategy"**
- **"Prioritize decomposition work based on component analysis"**
- **"Create a step-by-step decomposition plan"**

### Usage Examples

**Example 1: Complete Roadmap**

```
User: "Create a decomposition roadmap for this codebase"

The skill will:
1. Analyze current codebase state
2. Identify decomposition patterns to apply
3. Prioritize work based on risk and value
4. Create phased roadmap
5. Generate architecture stories
6. Estimate effort and dependencies
```

**Example 2: Prioritized Plan**

```
User: "Prioritize decomposition work based on component analysis"

The skill will:
1. Review component inventory and dependencies
2. Assess risk and value for each pattern
3. Prioritize patterns by impact
4. Create prioritized work plan
```

**Example 3: Phase Planning**

```
User: "Create a phased decomposition plan"

The skill will:
1. Group decomposition patterns into phases
2. Identify dependencies between phases
3. Create phase timeline
4. Define phase success criteria
```

### Step-by-Step Process

1. **Assess Current State**: Analyze codebase and identify what's been done
2. **Identify Patterns**: Determine which decomposition patterns to apply
3. **Prioritize Work**: Rank patterns by risk, value, and dependencies
4. **Create Roadmap**: Build phased plan with milestones
5. **Generate Stories**: Create architecture stories for tracking
6. **Track Progress**: Monitor progress through decomposition phases

## When to Use

Apply this skill when:

- Starting a decomposition effort
- Planning migration from monolith to distributed architecture
- Prioritizing decomposition work
- Creating architecture stories for decomposition
- Tracking progress through decomposition patterns
- Need structured approach to decomposition
- Want to estimate effort and dependencies

## Core Concepts

### Decomposition Pattern Sequence

The six component-based decomposition patterns should be applied in sequence:

1. **Identify and Size Components** - Understand what you have
2. **Gather Common Domain Components** - Find duplicates
3. **Flatten Components** - Remove orphaned classes
4. **Determine Component Dependencies** - Assess coupling
5. **Create Component Domains** - Group into domains
6. **Create Domain Services** - Extract to services

### Phased Approach

Decomposition typically follows phases:

**Phase 1: Analysis & Preparation** (Patterns 1-4)

- Component identification and sizing
- Common component detection
- Component flattening
- Dependency analysis

**Phase 2: Domain Organization** (Pattern 5)

- Domain identification
- Component grouping
- Namespace refactoring

**Phase 3: Service Extraction** (Pattern 6)

- Domain service creation
- Service extraction
- API boundary definition

### Prioritization Factors

When prioritizing decomposition work, consider:

- **Risk**: Low risk = easier to extract, fewer dependencies
- **Value**: High value = business-critical, high impact
- **Dependencies**: Can this be done independently?
- **Complexity**: Simple = fewer components, clear boundaries
- **Coupling**: Low coupling = easier to extract

## Analysis Process

### Phase 1: Assess Current State

Analyze what's already been done:

1. **Check Component Inventory**
   - Have components been identified and sized?
   - Is there a component inventory document?
   - Are oversized components identified?

2. **Check Common Component Analysis**
   - Have common domain components been identified?
   - Are consolidation opportunities documented?
   - Has coupling impact been analyzed?

3. **Check Component Structure**
   - Have components been flattened?
   - Are there orphaned classes?
   - Is component structure clean?

4. **Check Dependency Analysis**
   - Have component dependencies been mapped?
   - Is coupling analysis complete?
   - Is feasibility assessed?

5. **Check Domain Identification**
   - Have domains been identified?
   - Are components grouped into domains?
   - Are namespaces aligned with domains?

6. **Check Service Extraction**
   - Have any services been extracted?
   - Are domain services created?
   - Is service-based architecture in place?

**Output**: Current state assessment showing what's done and what's remaining

### Phase 2: Identify Patterns to Apply

Determine which decomposition patterns need to be applied:

1. **Review Pattern Prerequisites**
   - Pattern 1: Always needed (foundation)
   - Pattern 2: Needed if common components exist
   - Pattern 3: Needed if components have hierarchy
   - Pattern 4: Always needed (feasibility check)
   - Pattern 5: Needed before service extraction
   - Pattern 6: Final step (service extraction)

2. **Check Pattern Completion**
   - Which patterns are complete?
   - Which patterns are in progress?
   - Which patterns haven't started?

3. **Identify Missing Patterns**
   - What patterns still need to be applied?
   - What's blocking pattern application?
   - What dependencies exist?

**Output**: List of patterns to apply with status

### Phase 3: Prioritize Work

Prioritize decomposition patterns and work items:

1. **Assess Risk**
   - Low Risk: Infrastructure components, standalone functionality
   - Medium Risk: Domain components with some dependencies
   - High Risk: Core business logic, high coupling

2. **Assess Value**
   - High Value: Business-critical, high impact, frequent changes
   - Medium Value: Important but not critical
   - Low Value: Nice to have, low impact

3. **Assess Dependencies**
   - Independent: Can be done without other work
   - Dependent: Requires other patterns/work first
   - Blocking: Blocks other work from proceeding

4. **Calculate Priority Score**

   ```
   Priority = (Value × 3) - (Risk × 2) - (Dependencies × 1)

   Higher score = Higher priority
   ```

**Output**: Prioritized list of patterns and work items

### Phase 4: Create Phased Roadmap

Build a phased roadmap with milestones:

1. **Define Phases**
   - Phase 1: Analysis & Preparation
   - Phase 2: Domain Organization
   - Phase 3: Service Extraction
   - Phase 4: Optimization & Refinement

2. **Assign Patterns to Phases**
   - Which patterns belong in which phase?
   - What's the sequence within each phase?
   - What are the phase dependencies?

3. **Set Milestones**
   - What marks completion of each phase?
   - What are the success criteria?
   - What deliverables are expected?

4. **Estimate Timeline**
   - How long will each phase take?
   - What are the dependencies?
   - What's the critical path?

**Output**: Phased roadmap with timeline and milestones

### Phase 5: Generate Architecture Stories

Create architecture stories for tracking work:

1. **Create Story Template**

   ```
   As an architect, I need to [apply pattern/refactor component]
   to support [architectural characteristic/business need]
   so that [benefit/outcome]
   ```

2. **Break Down Work**
   - One story per pattern application
   - One story per major refactoring
   - One story per domain grouping

3. **Add Acceptance Criteria**
   - What defines "done"?
   - What metrics validate success?
   - What tests verify completion?

4. **Estimate Effort**
   - Story points or time estimates
   - Complexity assessment
   - Risk factors

**Output**: List of architecture stories with estimates

### Phase 6: Track Progress

Monitor progress through decomposition:

1. **Track Pattern Completion**
   - Which patterns are complete?
   - Which are in progress?
   - Which are blocked?

2. **Track Story Completion**
   - Stories completed
   - Stories in progress
   - Stories not started

3. **Track Metrics**
   - Components identified
   - Components refactored
   - Domains created
   - Services extracted

4. **Identify Blockers**
   - What's blocking progress?
   - What dependencies are missing?
   - What risks have emerged?

**Output**: Progress dashboard and status report

## Output Format

### Decomposition Roadmap

```markdown
# Decomposition Roadmap

## Current State Assessment

**Completed Patterns**:

- ✅ Pattern 1: Identify and Size Components
- ✅ Pattern 2: Gather Common Domain Components
- ⚠️ Pattern 3: Flatten Components (in progress)
- ❌ Pattern 4: Determine Component Dependencies (not started)
- ❌ Pattern 5: Create Component Domains (not started)
- ❌ Pattern 6: Create Domain Services (not started)

**Key Findings**:

- 75 components identified
- 3 common domain components found
- 2 oversized components need splitting
- High database coupling detected

## Phased Roadmap

### Phase 1: Analysis & Preparation (Weeks 1-4)

**Goal**: Complete component analysis and refactoring

**Patterns**:

1. Complete Pattern 3: Flatten Components
2. Apply Pattern 4: Determine Component Dependencies
3. Refactor oversized components

**Milestones**:

- Week 2: Component flattening complete
- Week 4: Dependency analysis complete

**Deliverables**:

- Flattened component structure
- Dependency diagram
- Feasibility assessment

### Phase 2: Domain Organization (Weeks 5-8)

**Goal**: Organize components into domains

**Patterns**:

1. Apply Pattern 5: Create Component Domains
2. Refactor namespaces for domain alignment

**Milestones**:

- Week 6: Domains identified
- Week 8: Namespace refactoring complete

**Deliverables**:

- Domain map
- Refactored component namespaces
- Domain documentation

### Phase 3: Service Extraction (Weeks 9-16)

**Goal**: Extract domains to domain services

**Patterns**:

1. Apply Pattern 6: Create Domain Services
2. Extract services incrementally

**Milestones**:

- Week 12: First domain service extracted
- Week 16: All domain services extracted

**Deliverables**:

- Domain services deployed
- API boundaries defined
- Service documentation
```

### Prioritized Work Plan

```markdown
## Prioritized Work Plan

### High Priority (Do First)

1. **Complete Component Flattening** (Priority: 9/10)
   - Risk: Low
   - Value: High (enables domain grouping)
   - Dependencies: None
   - Effort: 2 weeks

2. **Dependency Analysis** (Priority: 8/10)
   - Risk: Low
   - Value: High (validates feasibility)
   - Dependencies: Component flattening
   - Effort: 1 week

### Medium Priority (Do Next)

3. **Domain Identification** (Priority: 7/10)
   - Risk: Medium
   - Value: High (enables service extraction)
   - Dependencies: Dependency analysis
   - Effort: 2 weeks

### Low Priority (Do Later)

4. **Service Extraction** (Priority: 5/10)
   - Risk: High
   - Value: High (final goal)
   - Dependencies: Domain identification
   - Effort: 8 weeks
```

### Architecture Stories

```markdown
## Architecture Stories

### Story 1: Flatten Ticket Components

**As an architect**, I need to flatten the Ticket component hierarchy
to support better component organization
so that components exist only as leaf nodes.

**Acceptance Criteria**:

- [ ] No orphaned classes in root namespaces
- [ ] All components are leaf nodes
- [ ] Component structure validated

**Estimate**: 5 story points
**Priority**: High
**Dependencies**: None

### Story 2: Identify Component Domains

**As an architect**, I need to group components into logical domains
to support service-based architecture
so that components can be extracted to domain services.

**Acceptance Criteria**:

- [ ] All components assigned to domains
- [ ] Domain boundaries validated with stakeholders
- [ ] Domain map created

**Estimate**: 8 story points
**Priority**: High
**Dependencies**: Component flattening complete
```

### Progress Dashboard

```markdown
## Decomposition Progress Dashboard

### Pattern Completion Status

| Pattern                    | Status         | Progress | Blocker                 |
| -------------------------- | -------------- | -------- | ----------------------- |
| Identify & Size Components | ✅ Complete    | 100%     | None                    |
| Gather Common Components   | ✅ Complete    | 100%     | None                    |
| Flatten Components         | ⚠️ In Progress | 60%      | None                    |
| Determine Dependencies     | ❌ Not Started | 0%       | Waiting on flattening   |
| Create Domains             | ❌ Not Started | 0%       | Waiting on dependencies |
| Create Domain Services     | ❌ Not Started | 0%       | Waiting on domains      |

### Story Completion Status

**Completed**: 5 stories (25%)
**In Progress**: 3 stories (15%)
**Not Started**: 12 stories (60%)

### Key Metrics

- Components Identified: 75
- Components Refactored: 45 (60%)
- Domains Created: 0
- Services Extracted: 0
```

## Analysis Checklist

**Current State Assessment**:

- [ ] Reviewed component inventory
- [ ] Checked common component analysis
- [ ] Assessed component structure
- [ ] Reviewed dependency analysis
- [ ] Checked domain identification
- [ ] Assessed service extraction status

**Pattern Identification**:

- [ ] Identified which patterns are complete
- [ ] Identified which patterns are in progress
- [ ] Identified which patterns need to be applied
- [ ] Checked pattern dependencies

**Prioritization**:

- [ ] Assessed risk for each pattern
- [ ] Assessed value for each pattern
- [ ] Assessed dependencies
- [ ] Calculated priority scores

**Roadmap Creation**:

- [ ] Defined phases
- [ ] Assigned patterns to phases
- [ ] Set milestones
- [ ] Estimated timeline

**Story Generation**:

- [ ] Created architecture stories
- [ ] Added acceptance criteria
- [ ] Estimated effort
- [ ] Prioritized stories

**Progress Tracking**:

- [ ] Set up tracking mechanism
- [ ] Defined metrics
- [ ] Created dashboard
- [ ] Identified blockers

## Implementation Notes

### Roadmap Templates

**Simple Roadmap** (for small projects):

- Phase 1: Analysis (2-4 weeks)
- Phase 2: Refactoring (4-6 weeks)
- Phase 3: Extraction (8-12 weeks)

**Detailed Roadmap** (for large projects):

- Phase 1: Analysis & Preparation (4-8 weeks)
- Phase 2: Domain Organization (4-6 weeks)
- Phase 3: Service Extraction (12-16 weeks)
- Phase 4: Optimization (4-8 weeks)

### Prioritization Matrix

Use a 2x2 matrix for prioritization:

```
High Value, Low Risk    | High Value, High Risk
(Do First)              | (Do Carefully)
────────────────────────┼──────────────────────
Low Value, Low Risk     | Low Value, High Risk
(Do Later)              | (Avoid/Defer)
```

### Story Estimation

Use story points or time estimates:

**Story Points** (Fibonacci):

- 1: Trivial (few hours)
- 2: Simple (1 day)
- 3: Small (2-3 days)
- 5: Medium (1 week)
- 8: Large (2 weeks)
- 13: Very Large (3+ weeks)

**Time Estimates**:

- Small: 1-3 days
- Medium: 1-2 weeks
- Large: 2-4 weeks
- Very Large: 1+ month

## Best Practices

### Do's ✅

- Start with analysis patterns (Patterns 1-4)
- Prioritize low-risk, high-value work
- Create architecture stories for tracking
- Set clear milestones and success criteria
- Track progress regularly
- Adjust roadmap based on learnings
- Collaborate with stakeholders on priorities

### Don'ts ❌

- Don't skip analysis patterns
- Don't start service extraction too early
- Don't ignore dependencies between patterns
- Don't create unrealistic timelines
- Don't skip progress tracking
- Don't forget to validate with stakeholders
- Don't proceed without feasibility assessment

## Integration with Other Skills

This skill coordinates the use of other decomposition skills:

1. **Component Identification & Sizing** → Foundation for planning
2. **Common Domain Component Detection** → Identifies consolidation work
3. **Component Flattening** → Prepares for domain grouping
4. **Component Dependency Analysis** → Validates feasibility
5. **Domain Identification & Grouping** → Enables service extraction
6. **Decomposition Planning & Roadmap** (this skill) → Coordinates everything

## Next Steps

After creating the roadmap:

1. **Review with Stakeholders** - Get buy-in on plan
2. **Start Phase 1** - Begin with analysis patterns
3. **Track Progress** - Monitor completion and blockers
4. **Adjust as Needed** - Update roadmap based on learnings
5. **Celebrate Milestones** - Recognize progress

## Notes

- Roadmaps should be living documents, updated regularly
- Prioritization may change as you learn more
- Dependencies between patterns must be respected
- Feasibility assessment is critical before proceeding
- Stakeholder collaboration is essential for success
- Progress tracking helps identify issues early

================================================================
File: packages/skills-catalog/skills/(architecture)/domain-analysis/SKILL.md
================================================================
---
name: domain-analysis
description: Maps business domains and suggests service boundaries in any codebase using DDD Strategic Design. Use when asking "what are the domains in this codebase?", "where should I draw service boundaries?", "identify bounded contexts", "classify subdomains", "DDD analysis", or analyzing domain cohesion. Do NOT use for grouping existing components into domains (use domain-identification-grouping) or dependency analysis (use coupling-analysis).
---

# Subdomain Identification & Bounded Context Analysis

This skill analyzes codebases to identify subdomains (Core, Supporting, Generic) and suggest bounded contexts following Domain-Driven Design Strategic Design principles.

## When to Use

Apply this skill when:

- Analyzing domain boundaries in any codebase
- Identifying Core, Supporting, and Generic subdomains
- Mapping bounded contexts from problem space to solution space
- Assessing domain cohesion and detecting coupling issues
- Planning domain-driven refactoring
- Understanding business capabilities in code

## Core Principles

### Subdomain Classification

**Core Domain**: Competitive advantage, highest business value, requires best developers

- Indicators: Complex business logic, frequent changes, domain experts needed

**Supporting Subdomain**: Essential but not differentiating, business-specific

- Indicators: Supports Core Domain, moderate complexity, business-specific rules

**Generic Subdomain**: Common functionality, could be outsourced

- Indicators: Well-understood problem, low differentiation, standard functionality

### Bounded Context

An explicit linguistic boundary where domain terms have specific, unambiguous meanings.

- Primary nature: Linguistic boundary, not technical
- Key rule: Inside boundary, all Ubiquitous Language terms are unambiguous
- Goal: Align 1 subdomain to 1 bounded context (ideal)

## Analysis Process

### Phase 1: Extract Concepts

Scan codebase for business concepts (not infrastructure):

1. **Entities** (domain models with identity)
   - Patterns: `@Entity`, `class`, domain models
   - Focus: Business concepts, not technical classes

2. **Services** (business operations)
   - Patterns: `*Service`, `*Manager`, `*Handler`
   - Focus: Business logic, not technical utilities

3. **Use Cases** (business workflows)
   - Patterns: `*UseCase`, `*Command`, `*Handler`
   - Focus: Business processes, not CRUD

4. **Controllers/Resolvers** (entry points)
   - Patterns: `*Controller`, `*Resolver`, API endpoints
   - Focus: Business capabilities, not technical routes

### Phase 2: Group by Ubiquitous Language

For each concept, determine:

**Primary Language Context**

- What business vocabulary does this belong to?
- Examples:
  - `Subscription`, `Invoice`, `Payment` → Billing language
  - `Movie`, `Video`, `Episode` → Content language
  - `User`, `Authentication` → Identity language

**Linguistic Boundaries**

- Where do term meanings change?
- Same term, different meaning = different bounded context
- Example: "Customer" in Sales vs "Customer" in Support

**Concept Relationships**

- Which concepts naturally belong together?
- Which share business vocabulary?
- Which reference each other?

### Phase 3: Identify Subdomains

A subdomain has:

- Distinct business capability
- Independent business value
- Unique vocabulary
- Multiple related entities working together
- Cohesive set of business operations

**Common Domain Patterns**:

- Billing/Subscription: Payments, invoices, plans
- Content/Catalog: Media, products, inventory
- Identity/Access: Users, authentication, authorization
- Analytics: Metrics, dashboards, insights
- Notifications: Messages, alerts, communications

**Classify Each Subdomain**:

Use this decision tree:

```
Is it a competitive advantage?
  YES → Core Domain
  NO → Does it require business-specific knowledge?
        YES → Supporting Subdomain
        NO → Generic Subdomain
```

### Phase 4: Assess Cohesion

**High Cohesion Indicators** ✅

- Concepts share Ubiquitous Language
- Concepts frequently used together
- Direct business relationships
- Changes to one affect others in group
- Solve same business problem

**Low Cohesion Indicators** ❌

- Different business vocabularies mixed
- Concepts rarely used together
- No direct business relationship
- Changes don't affect others
- Solve different business problems

**Cohesion Score Formula**:

```
Score = (
  Linguistic Cohesion (0-3) +    // Shared vocabulary
  Usage Cohesion (0-3) +         // Used together
  Data Cohesion (0-2) +          // Entity relationships
  Change Cohesion (0-2)          // Change together
) / 10

8-10: High Cohesion ✅
5-7:  Medium Cohesion ⚠️
0-4:  Low Cohesion ❌
```

### Phase 5: Detect Low Cohesion Issues

**Rule 1: Linguistic Mismatch**

- Problem: Different business vocabularies mixed
- Example: `User` (identity) + `Subscription` (billing) in same service
- Action: Suggest separation into different bounded contexts

**Rule 2: Cross-Domain Dependencies**

- Problem: Tight coupling between domains
- Example: Service A directly instantiates entities from Domain B
- Action: Suggest interface-based integration

**Rule 3: Mixed Responsibilities**

- Problem: Single class handles multiple business concerns
- Example: Service handling both billing and content
- Action: Suggest splitting by subdomain

**Rule 4: Generic in Core**

- Problem: Generic functionality in core business logic
- Example: Email sending in billing service
- Action: Extract to Generic Subdomain

**Rule 5: Unclear Boundaries**

- Problem: Cannot determine which domain concept belongs to
- Example: Entity with relationships to multiple domains
- Action: Clarify boundaries, possibly split concept

### Phase 6: Map Bounded Contexts

For each subdomain identified, suggest bounded context:

**Bounded Context Characteristics**:

- Name reflects Ubiquitous Language
- Contains complete domain model
- Has explicit integration points
- Clear linguistic boundary

**Integration Patterns**:

- Shared Kernel: Shared model between contexts (use sparingly)
- Customer/Supplier: Downstream depends on upstream
- Conformist: Downstream conforms to upstream
- Anti-corruption Layer: Translation layer between contexts
- Open Host Service: Published interface for integration
- Published Language: Well-documented integration protocol

## Output Format

### Domain Map

For each domain/subdomain:

```markdown
## Domain: {Name}

**Type**: Core Domain | Supporting Subdomain | Generic Subdomain

**Ubiquitous Language**: {key business terms}

**Business Capability**: {what business problem it solves}

**Key Concepts**:

- {Concept} (Entity|Service|UseCase) - {brief description}

**Subdomains** (if applicable):

1. {Subdomain} (Core|Supporting|Generic)
   - Concepts: {list}
   - Cohesion: {score}/10
   - Dependencies: → {other domains}

**Suggested Bounded Context**: {Name}Context

- Linguistic boundary: {where terms have specific meaning}
- Integration: {how it should integrate with other contexts}

**Dependencies**:

- → {OtherDomain} via {interface/API}
- ← {OtherDomain} via {interface/API}

**Cohesion Score**: {score}/10
```

### Cohesion Matrix

```markdown
## Cross-Domain Cohesion

| Domain A | Domain B | Cohesion | Issue              | Recommendation          |
| -------- | -------- | -------- | ------------------ | ----------------------- |
| Billing  | Identity | 2/10     | ❌ Direct coupling | Use interface           |
| Content  | Billing  | 6/10     | ⚠️ Usage tracking  | Event-based integration |
```

### Low Cohesion Report

```markdown
## Issues Detected

### Priority: High

**Issue**: {description}

- **Location**: {file/class/method}
- **Problem**: {what's wrong}
- **Concepts**: {involved concepts}
- **Cohesion**: {score}/10
- **Recommendation**: {suggested fix}

### Priority: Medium

{similar format}
```

### Bounded Context Map

```markdown
## Suggested Bounded Contexts

### {ContextName}Context

**Contains Subdomains**:

- {Subdomain1} (Core)
- {Subdomain2} (Supporting)

**Ubiquitous Language**:

- Term: Definition in this context

**Integration Requirements**:

- Consumes from: {OtherContext} via {pattern}
- Publishes to: {OtherContext} via {pattern}

**Implementation Notes**:

- Separate persistence
- Independent deployment
- Explicit API boundaries
```

## Best Practices

### Do's ✅

- Focus on business language, not code structure
- Let Ubiquitous Language guide boundaries
- Measure cohesion objectively
- Identify clear integration points
- Classify every subdomain (Core/Supporting/Generic)
- Look for linguistic boundaries first

### Don'ts ❌

- Don't group by technical layers
- Don't force single global model
- Don't ignore linguistic differences
- Don't couple domains directly
- Don't create contexts by architecture
- Don't eliminate all dependencies (some are necessary)

## Analysis Checklist

**For Each Concept**:

- [ ] What business language does it belong to?
- [ ] What domain/subdomain is it part of?
- [ ] Is it Core, Supporting, or Generic?
- [ ] What other concepts does it relate to?
- [ ] Are dependencies within same domain?
- [ ] Any linguistic mismatches?

**For Each Domain**:

- [ ] What is the Ubiquitous Language?
- [ ] What are the key concepts?
- [ ] What are the subdomains?
- [ ] Which is the Core Domain?
- [ ] What are cross-domain dependencies?
- [ ] Is internal cohesion high?
- [ ] Are boundaries clear?

**For Cohesion Analysis**:

- [ ] Calculate cohesion scores
- [ ] Identify low cohesion areas
- [ ] Map cross-domain dependencies
- [ ] Flag linguistic mismatches
- [ ] Note tight coupling
- [ ] Suggest boundary clarifications

## Quick Reference

### Subdomain Decision Tree

```
Analyze business capability
└─ Is it competitive advantage?
   ├─ YES → Core Domain
   └─ NO → Is it business-specific?
      ├─ YES → Supporting Subdomain
      └─ NO → Generic Subdomain
```

### Cohesion Quick Check

```
Same vocabulary? → High linguistic cohesion
Used together? → High usage cohesion
Direct relationships? → High data cohesion
Change together? → High change cohesion

All high → Strong subdomain candidate
Mix of high/low → Review boundaries
All low → Likely wrong grouping
```

### Bounded Context Signals

```
Clear boundary signs:
✅ Distinct Ubiquitous Language
✅ Concepts have unambiguous meaning
✅ Different meanings across contexts
✅ Clear integration points

Unclear boundary signs:
❌ Same terms with same meanings everywhere
❌ Concepts used identically across system
❌ No clear linguistic differences
❌ Tight coupling everywhere
```

## Anti-Patterns to Avoid

**Big Ball of Mud**

- Everything connected to everything
- No clear boundaries
- Mixed vocabularies
- Prevention: Explicit bounded contexts

**All-Inclusive Model**

- Single model for entire business
- Impossible global definitions
- Creates conflicts
- Prevention: Embrace multiple contexts

**Mixed Linguistic Concepts**

- Different vocabularies in same context
- Example: User/Permission with Forum/Post
- Prevention: Keep linguistic associations

## Notes

- This is strategic analysis, not tactical implementation
- Focus on WHAT domains exist, not HOW to implement
- Some cross-domain dependencies are normal
- Low cohesion doesn't always mean "bad," it means "needs attention"
- Generic Subdomains naturally have lower cohesion
- Always validate with domain experts when possible

## Validation Criteria

Good domain identification has:

- ✅ Clear boundaries with distinct Ubiquitous Language
- ✅ High internal cohesion within domains
- ✅ Explicit cross-domain dependencies
- ✅ Business alignment with capabilities
- ✅ Actionable recommendations for issues

================================================================
File: packages/skills-catalog/skills/(architecture)/domain-identification-grouping/SKILL.md
================================================================
---
name: domain-identification-grouping
description: Groups existing components into logical business domains to plan service-based architecture. Use when asking "which components belong together?", "group these into services", "organize by domain", "component-to-domain mapping", or planning service extraction from an existing codebase. Do NOT use for identifying new domains from scratch (use domain-analysis) or analyzing coupling (use coupling-analysis).
---

# Domain Identification and Grouping

This skill groups architectural components into logical domains (business areas) to prepare for creating domain services in a service-based architecture.

## How to Use

### Quick Start

Request analysis of your codebase:

- **"Group components into logical domains"**
- **"Identify component domains for service-based architecture"**
- **"Create domain groupings from components"**
- **"Analyze which components belong to which domains"**

### Usage Examples

**Example 1: Domain Identification**

```
User: "Group components into logical domains"

The skill will:
1. Analyze component responsibilities and relationships
2. Identify business domains based on functionality
3. Group components into domains
4. Create domain diagrams
5. Suggest namespace refactoring for domain alignment
```

**Example 2: Domain Analysis**

```
User: "Which domain should the billing components belong to?"

The skill will:
1. Analyze billing component functionality
2. Check relationships with other components
3. Identify appropriate domain (e.g., Customer or Financial)
4. Recommend domain assignment
```

**Example 3: Domain Refactoring**

```
User: "What namespace refactoring is needed to align components with domains?"

The skill will:
1. Compare current component namespaces to identified domains
2. Identify misaligned components
3. Suggest namespace changes
4. Create refactoring plan
```

### Step-by-Step Process

1. **Identify Domains**: Analyze business capabilities and component relationships
2. **Group Components**: Assign components to appropriate domains
3. **Validate Groupings**: Ensure components fit well in their domains
4. **Refactor Namespaces**: Align component namespaces with domains
5. **Create Domain Map**: Visualize domain structure and component groupings

## When to Use

Apply this skill when:

- After identifying, sizing, and analyzing component dependencies
- Before creating domain services (Pattern 6)
- When planning service-based architecture migration
- Analyzing component relationships and business alignment
- Preparing for domain-driven design implementation
- Grouping components for better organization

## Core Concepts

### Domain Definition

A **domain** is a logical grouping of components that:

- Represents a distinct business capability or area
- Contains related components that work together
- Has clear boundaries and responsibilities
- Can become a domain service in service-based architecture

**Examples**:

- **Customer Domain**: Customer profile, billing, support contracts
- **Ticketing Domain**: Ticket creation, assignment, routing, completion
- **Reporting Domain**: Ticket reports, expert reports, financial reports

### Component Domain Relationship

**One-to-Many**: A single domain contains multiple components

```
Domain: Customer
├── Component: Customer Profile
├── Component: Billing Payment
├── Component: Billing History
└── Component: Support Contract
```

### Domain Manifestation

Domains are physically manifested through **namespace structure**:

**Before Domain Alignment**:

```
services/billing/payment
services/billing/history
services/customer/profile
services/supportcontract
```

**After Domain Alignment**:

```
services/customer/billing/payment
services/customer/billing/history
services/customer/profile
services/customer/supportcontract
```

Notice how all customer-related functionality is grouped under `.customer` domain.

## Analysis Process

### Phase 1: Identify Business Domains

Analyze the codebase to identify distinct business domains:

1. **Examine Component Responsibilities**
   - Read component names and descriptions
   - Understand what each component does
   - Identify business capabilities

2. **Look for Business Language**
   - Group components by business vocabulary
   - Example: "billing", "payment", "invoice" → Financial domain
   - Example: "customer", "profile", "contract" → Customer domain

3. **Identify Domain Boundaries**
   - Where do business concepts change?
   - What are the distinct business areas?
   - How do components relate to business capabilities?

4. **Collaborate with Business Stakeholders**
   - Validate domain identification with product owners
   - Ensure domains align with business understanding
   - Get feedback on domain boundaries

**Example Domain Identification**:

```markdown
## Identified Domains

1. **Ticketing Domain** (ss.ticket)
   - Ticket creation, assignment, routing, completion
   - Customer surveys
   - Knowledge base

2. **Customer Domain** (ss.customer)
   - Customer profile
   - Billing and payment
   - Support contracts

3. **Reporting Domain** (ss.reporting)
   - Ticket reports
   - Expert reports
   - Financial reports

4. **Admin Domain** (ss.admin)
   - User maintenance
   - Expert profile management

5. **Shared Domain** (ss.shared)
   - Login
   - Notification
```

### Phase 2: Group Components into Domains

Assign each component to an appropriate domain:

1. **Analyze Component Functionality**
   - What business capability does it support?
   - What domain vocabulary does it use?
   - What other components does it relate to?

2. **Check Component Relationships**
   - Which components are frequently used together?
   - What are the dependencies between components?
   - Do components share data or workflows?

3. **Assign to Domain**
   - Place component in domain that best fits its functionality
   - Ensure component aligns with domain's business language
   - Verify component relationships support domain grouping

4. **Handle Edge Cases**
   - Components that don't fit clearly: Analyze more deeply
   - Components that fit multiple domains: Choose primary domain
   - Shared components: May belong to Shared domain

**Example Component Grouping**:

```markdown
## Component Domain Assignment

### Ticketing Domain (ss.ticket)

- Ticket Shared (ss.ticket.shared)
- Ticket Maintenance (ss.ticket.maintenance)
- Ticket Completion (ss.ticket.completion)
- Ticket Assign (ss.ticket.assign)
- Ticket Route (ss.ticket.route)
- KB Maintenance (ss.ticket.kb.maintenance)
- KB Search (ss.ticket.kb.search)
- Survey (ss.ticket.survey)

### Customer Domain (ss.customer)

- Customer Profile (ss.customer.profile)
- Billing Payment (ss.customer.billing.payment)
- Billing History (ss.customer.billing.history)
- Support Contract (ss.customer.supportcontract)

### Reporting Domain (ss.reporting)

- Reporting Shared (ss.reporting.shared)
- Ticket Reports (ss.reporting.tickets)
- Expert Reports (ss.reporting.experts)
- Financial Reports (ss.reporting.financial)
```

### Phase 3: Validate Domain Groupings

Ensure components fit well in their assigned domains:

1. **Check Cohesion**
   - Do components in domain share business language?
   - Are components frequently used together?
   - Do components have direct relationships?

2. **Verify Boundaries**
   - Are domain boundaries clear?
   - Do components belong to only one domain?
   - Are there components that don't fit anywhere?

3. **Assess Completeness**
   - Are all components assigned to a domain?
   - Are domains cohesive and well-formed?
   - Do domains represent distinct business capabilities?

4. **Get Stakeholder Validation**
   - Review domain groupings with product owners
   - Ensure domains align with business understanding
   - Get feedback on domain boundaries

**Validation Checklist**:

- [ ] All components assigned to a domain
- [ ] Domains have clear boundaries
- [ ] Components fit well in their domains
- [ ] Domains represent distinct business capabilities
- [ ] Stakeholders validate domain groupings

### Phase 4: Refactor Namespaces for Domain Alignment

Align component namespaces with identified domains:

1. **Compare Current vs Target Namespaces**
   - Current: `services/billing/payment`
   - Target: `services/customer/billing/payment`
   - Change: Add `.customer` domain node

2. **Identify Refactoring Needed**
   - Which components need namespace changes?
   - What domain nodes need to be added?
   - Are there components already aligned?

3. **Create Refactoring Plan**
   - List components needing namespace changes
   - Specify target namespace for each
   - Prioritize refactoring work

4. **Execute Refactoring**
   - Update component namespaces
   - Update imports/references
   - Verify all references updated

**Example Namespace Refactoring**:

```markdown
## Namespace Refactoring Plan

### Customer Domain Alignment

| Component        | Current Namespace   | Target Namespace            | Action        |
| ---------------- | ------------------- | --------------------------- | ------------- |
| Billing Payment  | ss.billing.payment  | ss.customer.billing.payment | Add .customer |
| Billing History  | ss.billing.history  | ss.customer.billing.history | Add .customer |
| Customer Profile | ss.customer.profile | ss.customer.profile         | No change     |
| Support Contract | ss.supportcontract  | ss.customer.supportcontract | Add .customer |

### Ticketing Domain Alignment

| Component      | Current Namespace | Target Namespace         | Action      |
| -------------- | ----------------- | ------------------------ | ----------- |
| KB Maintenance | ss.kb.maintenance | ss.ticket.kb.maintenance | Add .ticket |
| KB Search      | ss.kb.search      | ss.ticket.kb.search      | Add .ticket |
| Survey         | ss.survey         | ss.ticket.survey         | Add .ticket |
```

### Phase 5: Create Domain Map

Visualize domain structure and component groupings:

1. **Create Domain Diagram**
   - Show domains as boxes
   - Show components within each domain
   - Show relationships between domains

2. **Document Domain Structure**
   - List domains and their components
   - Describe domain responsibilities
   - Note domain boundaries

3. **Create Domain Inventory**
   - Table of domains and components
   - Component counts per domain
   - Size metrics per domain

**Example Domain Map**:

```markdown
## Domain Map
```

┌─────────────────────────────────────┐
│ Ticketing Domain (ss.ticket) │
├─────────────────────────────────────┤
│ • Ticket Shared │
│ • Ticket Maintenance │
│ • Ticket Completion │
│ • Ticket Assign │
│ • Ticket Route │
│ • KB Maintenance │
│ • KB Search │
│ • Survey │
└─────────────────────────────────────┘
│
│ uses
▼
┌─────────────────────────────────────┐
│ Customer Domain (ss.customer) │
├─────────────────────────────────────┤
│ • Customer Profile │
│ • Billing Payment │
│ • Billing History │
│ • Support Contract │
└─────────────────────────────────────┘

````

## Output Format

### Domain Identification Report

```markdown
## Domain Identification

### Domain: Customer (ss.customer)

**Business Capability**: Manages customer relationships, billing, and support contracts

**Components**:
- Customer Profile (ss.customer.profile)
- Billing Payment (ss.customer.billing.payment)
- Billing History (ss.customer.billing.history)
- Support Contract (ss.customer.supportcontract)

**Component Count**: 4
**Total Size**: ~15,000 statements (18% of codebase)

**Domain Cohesion**: ✅ High
- Components share customer-related vocabulary
- Components frequently used together
- Direct relationships between components

**Boundaries**:
- Clear separation from Ticketing domain
- Clear separation from Reporting domain
- Shared components (Notification) used by all domains
````

### Component Domain Assignment Table

```markdown
## Component Domain Assignment

| Component          | Current Namespace     | Assigned Domain | Target Namespace                  |
| ------------------ | --------------------- | --------------- | --------------------------------- |
| Customer Profile   | ss.customer.profile   | Customer        | ss.customer.profile (no change)   |
| Billing Payment    | ss.billing.payment    | Customer        | ss.customer.billing.payment       |
| Ticket Maintenance | ss.ticket.maintenance | Ticketing       | ss.ticket.maintenance (no change) |
| KB Maintenance     | ss.kb.maintenance     | Ticketing       | ss.ticket.kb.maintenance          |
| Reporting Shared   | ss.reporting.shared   | Reporting       | ss.reporting.shared (no change)   |
```

### Namespace Refactoring Plan

```markdown
## Namespace Refactoring Plan

### Priority: High

**Customer Domain Alignment**

**Components to Refactor**:

1. Billing Payment: `ss.billing.payment` → `ss.customer.billing.payment`
2. Billing History: `ss.billing.history` → `ss.customer.billing.history`
3. Support Contract: `ss.supportcontract` → `ss.customer.supportcontract`

**Steps**:

1. Update namespace declarations in source files
2. Update import statements in dependent components
3. Update directory structure
4. Run tests to verify changes
5. Update documentation

**Expected Impact**:

- All customer-related components aligned under `.customer` domain
- Clearer domain boundaries
- Easier to identify domain components
```

### Domain Map Visualization

```markdown
## Domain Map

### Domain Structure
```

Customer Domain (ss.customer)
├── Customer Profile
├── Billing Payment
├── Billing History
└── Support Contract

Ticketing Domain (ss.ticket)
├── Ticket Shared
├── Ticket Maintenance
├── Ticket Completion
├── Ticket Assign
├── Ticket Route
├── KB Maintenance
├── KB Search
└── Survey

Reporting Domain (ss.reporting)
├── Reporting Shared
├── Ticket Reports
├── Expert Reports
└── Financial Reports

Admin Domain (ss.admin)
├── User Maintenance
└── Expert Profile

Shared Domain (ss.shared)
├── Login
└── Notification

```

### Domain Relationships

```

Ticketing Domain
│ uses
├─→ Shared Domain (Login, Notification)
└─→ Customer Domain (Customer Profile)

Customer Domain
│ uses
└─→ Shared Domain (Login, Notification)

Reporting Domain
│ uses
├─→ Ticketing Domain (Ticket data)
├─→ Customer Domain (Customer data)
└─→ Shared Domain (Login)

```

```

## Analysis Checklist

**Domain Identification**:

- [ ] Analyzed component responsibilities
- [ ] Identified business capabilities
- [ ] Identified distinct business domains
- [ ] Validated domains with stakeholders

**Component Grouping**:

- [ ] Assigned each component to a domain
- [ ] Analyzed component relationships
- [ ] Ensured components fit domain vocabulary
- [ ] Handled edge cases (shared components, unclear assignments)

**Domain Validation**:

- [ ] Checked cohesion within domains
- [ ] Verified domain boundaries are clear
- [ ] Ensured all components assigned
- [ ] Validated with stakeholders

**Namespace Refactoring**:

- [ ] Compared current vs target namespaces
- [ ] Identified components needing refactoring
- [ ] Created refactoring plan
- [ ] Prioritized refactoring work

**Domain Mapping**:

- [ ] Created domain diagram
- [ ] Documented domain structure
- [ ] Created domain inventory table
- [ ] Documented domain relationships

## Implementation Notes

### For Node.js/Express Applications

Domains typically organized in `services/` directory:

```
services/
├── customer/              ← Customer Domain
│   ├── profile/
│   ├── billing/
│   │   ├── payment/
│   │   └── history/
│   └── supportcontract/
├── ticket/                ← Ticketing Domain
│   ├── shared/
│   ├── maintenance/
│   ├── assign/
│   └── route/
└── reporting/             ← Reporting Domain
    ├── shared/
    ├── tickets/
    └── experts/
```

### For Java Applications

Domains identified by package structure:

```
com.company.customer       ← Customer Domain
├── profile
├── billing
│   ├── payment
│   └── history
└── supportcontract

com.company.ticket         ← Ticketing Domain
├── shared
├── maintenance
├── assign
└── route
```

### Domain Identification Strategies

**Strategy 1: Business Capability Analysis**

- Identify what business capabilities the system provides
- Group components by capability
- Example: "Customer Management" capability → Customer Domain

**Strategy 2: Vocabulary Analysis**

- Identify business vocabulary used by components
- Group components sharing same vocabulary
- Example: Components using "billing", "payment", "invoice" → Financial Domain

**Strategy 3: Relationship Analysis**

- Identify components frequently used together
- Group components with strong relationships
- Example: Components that share data/workflows → Same Domain

**Strategy 4: Stakeholder Collaboration**

- Work with product owners/business analysts
- Use their understanding of business areas
- Validate domain boundaries with them

## Fitness Functions

After creating domains, create automated checks:

### Domain Namespace Governance

```javascript
// Ensure components belong to correct domain
function validateDomainNamespaces(components, domainRules) {
  const violations = []

  components.forEach((comp) => {
    const domain = identifyDomain(comp.namespace)
    const expectedDomain = domainRules[comp.name]

    if (domain !== expectedDomain) {
      violations.push({
        component: comp.name,
        currentDomain: domain,
        expectedDomain: expectedDomain,
        namespace: comp.namespace,
      })
    }
  })

  return violations
}
```

### Domain Boundary Enforcement

```javascript
// Prevent components from accessing other domains directly
function enforceDomainBoundaries(components) {
  const violations = []

  components.forEach((comp) => {
    comp.imports.forEach((imp) => {
      const importedDomain = identifyDomain(imp)
      const componentDomain = identifyDomain(comp.namespace)

      if (importedDomain !== componentDomain && importedDomain !== 'shared') {
        violations.push({
          component: comp.name,
          domain: componentDomain,
          importsFrom: imp,
          importedDomain: importedDomain,
          issue: 'Cross-domain direct dependency',
        })
      }
    })
  })

  return violations
}
```

## Best Practices

### Do's ✅

- Collaborate with business stakeholders to identify domains
- Group components by business capability, not technical layers
- Ensure domains represent distinct business areas
- Validate domain boundaries with stakeholders
- Refactor namespaces to align with domains
- Create clear domain documentation
- Use business language in domain names

### Don'ts ❌

- Don't create domains based on technical layers (services, controllers, models)
- Don't force components into domains where they don't fit
- Don't skip stakeholder validation
- Don't create too many small domains (aim for 3-7 domains)
- Don't create domains that are too large (monolithic domains)
- Don't ignore components that don't fit (analyze why)
- Don't skip namespace refactoring (critical for clarity)

## Common Domain Patterns

### Typical Domains in Business Applications

- **Customer Domain**: Customer management, profiles, relationships
- **Product Domain**: Product catalog, inventory, pricing
- **Order Domain**: Order processing, fulfillment, shipping
- **Billing Domain**: Invoicing, payments, financial transactions
- **Reporting Domain**: Reports, analytics, dashboards
- **Admin Domain**: User management, system configuration
- **Shared Domain**: Common functionality (login, notification, utilities)

### Domain Size Guidelines

- **Small Domain**: 2-4 components
- **Medium Domain**: 5-8 components
- **Large Domain**: 9-15 components
- **Too Large**: >15 components (consider splitting)

## Next Steps

After creating component domains:

1. **Apply Create Domain Services Pattern** - Extract domains to separate services
2. **Plan Service Extraction** - Create migration plan for domain services
3. **Implement Domain Services** - Move domains to separately deployed services
4. **Monitor Domain Boundaries** - Use fitness functions to enforce boundaries

## Notes

- Domains should represent business capabilities, not technical layers
- Domain identification requires collaboration with business stakeholders
- Namespace refactoring is critical for domain clarity
- Domains prepare the codebase for service-based architecture
- Well-formed domains make service extraction easier
- Domain boundaries should be clear and well-documented

================================================================
File: packages/skills-catalog/skills/(architecture)/evolutionary-modular-architecture/SKILL.md
================================================================
---
name: evolutionary-modular-architecture
description: Guides design and implementation of evolutionary modular-monolith platforms with DDD (strategic + tactical), flat-by-aggregate organization, an Anti-Corruption Layer for vendor independence, a transactional outbox for events, smart resilience (backoff with jitter, circuit breakers, idempotency), and a polished architecture HTML document with elegant SVG diagrams. Use when designing a platform or backend, defining bounded contexts, organizing modules and folders, choosing monolith vs microservices, decoupling from an external service (ERP, storage, AI), making calls resilient, adding real-time push, picking a 2026 TypeScript stack (Nx, NestJS, React), or producing an architecture document or diagram. Also triggers on 'modular monolith', 'bounded contexts', 'flat-by-aggregate', 'ports and adapters', 'architecture diagram'. Do NOT use for simple CRUD, NestJS-only deep implementation (use nestjs-modular-monolith), or pure domain-model review (use tactical-ddd).
license: CC-BY-4.0
metadata:
  author: Felipe Rodrigues - github.com/felipfr
  version: 1.0.0
---

# Evolutionary Modular Architecture

Design and build platforms as an **evolutionary modular monolith**: strong logical boundaries from day one (DDD bounded contexts, flat-by-aggregate modules, an Anti-Corruption Layer around every external system, event-driven communication, resilience by default) while keeping physical boundaries (separate deploys, separate databases) as a later, optional step. Start simple, evolve granularity only when the product and the team justify it — never reverse-engineer microservices from hype.

## When to use this skill

- Designing a new platform, backend, or service from scratch.
- Defining bounded contexts and a context map for a domain.
- Deciding how to organize a module's folders and files.
- Choosing between monolith, modular monolith, and microservices.
- Integrating with (or decoupling from) an external vendor: ERP, ticketing, storage, payment, AI, identity, durable-workflow engine.
- Making external/inter-module calls resilient.
- Adding real-time server-to-client updates.
- Picking a modern (2026) full-stack TypeScript stack.

## When NOT to use it

- Simple CRUD with a handful of endpoints — framework defaults are enough.
- Deep, NestJS-specific implementation detail — prefer `nestjs-modular-monolith`.
- Reviewing or refactoring a single domain model for anemia — prefer `tactical-ddd`.

---

## The one rule that governs everything

**Separate the logical boundary (always strong) from the physical boundary (evolutionary).** Modules, contracts, state ownership, and the Anti-Corruption Layer are non-negotiable from day one. Whether a module is its own deploy or its own database is an **operational** decision made later — never a structural prerequisite. This is what lets the system start as one deploy and grow without a rewrite.

A useful mental image: the house has well-divided rooms (modules). Inside each room things sit out in the open, grouped by what they are for (flat-by-aggregate) — not buried in nested drawers (technical-layer folders). The floor plan (boundaries) is what matters most.

---

## Core model (load the matching reference when you go deep)

| Topic | What it covers | Reference |
| --- | --- | --- |
| Principles | 10 modular (P1–P10) + 9 structural (P11–P19) + conflict hierarchy | `references/principles.md` |
| DDD | Strategic (subdomains, context map, integration patterns) + tactical (rich aggregates, intensity by subdomain) | `references/ddd.md` |
| Module internals | Flat-by-aggregate, suffixes, depth, flat vs subdomain test, scaffolding | `references/flat-by-aggregate.md` |
| Communication | Ports & Adapters / ACL, events + transactional outbox, SSE real-time | `references/acl-and-communication.md` |
| Resilience | Backoff + full jitter, circuit breaker, retry budget, idempotency, bulkhead, timeouts | `references/resilience.md` |
| Stack 2026 | Frontend, backend, data, observability, durable workflow, decisions | `references/stack-2026.md` |
| Architecture doc | Building the elegant, self-contained HTML architecture document with SVG diagrams | `references/architecture-doc.md` + `assets/architecture-template.html` |
| Validation | Deterministic checks for structure and module boundaries | `scripts/validate-structure.mjs`, `scripts/validate-boundaries.mjs` |

Do not load all references at once. Read a reference only when the current phase needs it (the workflow below states when).

---

## Workflow

Use this whether you are **designing** a new system or **reviewing** an existing one. Move through phases in order; each has an exit criterion. State assumptions explicitly; if the domain is unclear, ask before guessing.

### Phase 0 — Frame the scope

Confirm: is this a new platform, a new module in an existing one, or a review? Confirm the runtime/stack constraints (default target is the 2026 TypeScript stack — see `references/stack-2026.md`). Default to **one deploy** (modular monolith) unless a hard constraint says otherwise.

Exit: scope and constraints written down.

### Phase 1 — Domain discovery (DDD strategic)

Read `references/ddd.md`. Identify subdomains from the business language, classify each as **Core**, **Supporting**, or **Generic**, and find the ubiquitous language of each. Do not group by technical layer. If multiple bounded-context interpretations exist, present them — do not pick silently.

Exit: a list of candidate bounded contexts, each classified, with one-line responsibility and key aggregates.

### Phase 2 — Boundaries & context map

Draw the context map: which contexts exist, how they relate (Customer/Supplier, Conformist, Open Host Service, Published Language, Shared Kernel, Anti-Corruption Layer), and which are Core. Decide **state ownership**: one database is fine, but each module is the **sole writer of its own tables** — no foreign keys across module boundaries; reference other contexts by id. Keep aggregates cohesive (do not over-split a transactional Core).

Exit: context map + table-ownership map (one module = its tables).

### Phase 3 — Module internals (flat-by-aggregate)

Read `references/flat-by-aggregate.md`. Inside each module, organize by **aggregate**, not by technical layer: 1 business concept = 1 folder; technical layers become file **suffixes** (`.entity.ts`, `.service.ts`, `.controller.ts`). Keep depth ≤ 2 (flat) or ≤ 3 (subdomain-based). The Clean Architecture **dependency rule still holds** (presentation → application → domain ← infrastructure) — it is just expressed by suffixes and co-location, not by layer folders. Use the 6-criteria test to decide flat vs subdomain-based; default to flat.

Exit: a folder layout per module and a flat-vs-subdomain decision with rationale.

### Phase 4 — Communication & Anti-Corruption Layer

Read `references/acl-and-communication.md`. Every external system goes **behind a Port + Adapter (ACL)** — including internal services owned by other teams. The domain defines ports in its own language; adapters translate the external model in and out. Between internal modules: **synchronous only inside an aggregate** (one ACID transaction); **events via the transactional outbox** across modules, with idempotent consumers. Add SSE for server-to-client real-time when the UX needs push.

Exit: list of ports + adapters; list of domain events with the `module.aggregate.action` naming; sync-vs-async decisions.

### Phase 5 — Resilience

Read `references/resilience.md`. Wrap every external/inter-service call: **timeout → circuit breaker → retry (capped exponential backoff + full jitter)**. Only retry idempotent operations (require an idempotency key for writes). Cap retries with a budget (~10% of traffic), trip the breaker on a sliding-window error rate, and give every breaker a named fallback. Add durable execution only when a long-running, multi-step process must survive restarts.

Exit: a resilience policy applied to each adapter, plus idempotency keys for writes.

### Phase 6 — Stack & evolution

Read `references/stack-2026.md`. Choose the concrete stack and call out the trade-offs (runtime, API style, cache, real-time transport). Then state the **evolution path**: stage 1 (modular monolith + clear boundaries, with events/outbox already in place) is the current state; promote a module to its own app/database only when its own metrics justify it.

Exit: stack table + evolution note.

### Phase 7 — Document the architecture (HTML)

Produce a polished, self-contained HTML architecture document with elegant hand-drawn-style SVG diagrams. Read `references/architecture-doc.md` and start from `assets/architecture-template.html`: copy the template, keep its CSS and visual system intact, and replace the placeholder content. Build the standard sections (overview, domains, principles, monolith map, bounded contexts, ACL, communication, front-to-back, modules, resilience, real-time, evolution, stack) and number diagrams sequentially ("Diagram N — ..."). Preserve the visual identity: palette, Lato + JetBrains Mono fonts, rounded rectangles, arrow markers, soft gradients, and italic captions. The output is one HTML file that opens directly in a browser — no build step.

Exit: a single self-contained HTML file with consistent diagrams and working in-page navigation.

---

## Decision rules (cheat sheet)

- **Subdomain class:** competitive advantage → Core; business-specific but not differentiating → Supporting; solved problem you consume → Generic.
- **Tactical intensity:** full (rich aggregates, VOs, invariants, events) in **Core**; moderate in **Supporting**; minimal in **Generic** (forcing richness where there is no invariant is over-engineering). Details in `references/ddd.md`.
- **Flat vs subdomain:** flat by default (depth 2). Go subdomain-based (depth 3) only when 4+ of 6 criteria hold: different personas, authorization, execution model, scaling, deployment, failure isolation. Details in `references/flat-by-aggregate.md`.
- **Sync vs event:** sync inside one aggregate (atomicity matters); event via outbox across modules (tolerates eventual consistency).
- **Durable execution:** add it only for long-running, multi-step, must-resume processes — not for ordinary requests.

## Hard rules (non-negotiable)

- Each module **writes only its own tables**; cross-context links are by id, validated in code (`enforce-module-boundaries`).
- **No direct cross-module imports** — communicate via facade/contract or event.
- **Every external system sits behind a port + ACL** — no vendor model leaks into the domain.
- **Retry only idempotent operations**; writes carry an idempotency key.
- **Modular principles (P1–P10) beat structural ones (P11–P19).** If co-locating would force a cross-module entity import, use a facade + DTO; if splitting an aggregate would break a transaction, keep it together.

---

## Automated checks

Make the principles executable instead of relying on review. Run these in CI and before merge (both are zero-dependency Node ESM scripts; pass the libs/packages root, or let them autodetect):

- `node scripts/validate-structure.mjs [root]` — enforces flat-by-aggregate (P11–P17): no technical-layer folders, no single-file folders (except `__test__/`), depth ≤ 3, no README inside aggregates.
- `node scripts/validate-boundaries.mjs [root]` — enforces modular boundaries (P1, P3, P8): no deep cross-module imports (only via barrel/facade), no duplicate or unprefixed entity names, no cross-context relations.

Both exit non-zero on violation and print the offending paths. Treat structural failures as blocking; treat boundary findings as blocking once the team adopts the convention.

---

## Examples

### Example 1: Design a new platform

User says: "Desenhe a arquitetura de uma plataforma de contas a pagar que lança no ERP do cliente."
Actions:
1. Phase 1 — discover subdomains; classify (e.g., Payables and Operations as Core; Documents/Audit/Gamification as Supporting; Identity/Notifications as Generic).
2. Phase 2 — context map + one-database, table-per-module ownership.
3. Phase 3 — flat-by-aggregate layout per module.
4. Phase 4 — ERP/ticketing/storage behind ports + ACL; events via outbox; SSE for the operator queue.
5. Phase 5 — resilience policy on the ERP adapter (idempotency key = payment id).
6. Phase 6 — 2026 stack + evolution note (one deploy now).
Result: a context map, module layouts, a port/adapter list, a resilience policy, and a stack + evolution plan — each grounded in the matching reference.

### Example 2: Organize a module

User says: "Como organizo o módulo de billing?"
Actions: read `references/flat-by-aggregate.md`; propose `billing/subscription/`, `billing/invoice/`, `billing/payment/` with co-located files and `__test__/`; run the 6-criteria test (billing = flat). Result: a flat folder tree with rationale and the dependency rule preserved via suffixes.

### Example 3: Decouple from a vendor

User says: "Não quero ficar preso ao OMIE."
Actions: read `references/acl-and-communication.md`; define `ErpLedgerPort` in domain language; implement `OmieAdapter`; translate the OMIE model at the boundary; show that swapping ERPs means swapping the adapter. Result: a port interface + adapter plan; the domain never imports vendor types.

---

## Anti-patterns / red flags

- ❌ Microservices or many databases from day one ("it feels big" is not a reason).
- ❌ Technical-layer folders (`core/service/`, `http/controller/`, `persistence/entity/`) inside a module — that is the legacy pattern flat-by-aggregate replaces.
- ❌ A module reading or writing another module's tables.
- ❌ Vendor SDK types leaking into the domain (no ACL).
- ❌ Retrying non-idempotent writes; retries without backoff + jitter; retries with no budget.
- ❌ Splitting a cohesive transactional Core into event-coupled fragments.
- ❌ Forcing rich tactical DDD on a Generic subdomain that has no invariants.

## Reference guide

| Load this file | When |
| --- | --- |
| `references/principles.md` | You need the exact principle text, or to resolve a structural-vs-modular conflict. |
| `references/ddd.md` | Phase 1–2: classifying subdomains, drawing the context map, deciding tactical intensity. |
| `references/flat-by-aggregate.md` | Phase 3: laying out a module; flat vs subdomain; scaffolding; facades; tests. |
| `references/acl-and-communication.md` | Phase 4: ports/adapters/ACL, events + outbox, SSE. |
| `references/resilience.md` | Phase 5: backoff/jitter, circuit breaker, idempotency, bulkhead, durable execution. |
| `references/stack-2026.md` | Phase 0/6: concrete stack choices and trade-offs. |
| `references/architecture-doc.md` + `assets/architecture-template.html` | Phase 7: producing the HTML architecture document and its diagrams. |
| `scripts/validate-structure.mjs`, `scripts/validate-boundaries.mjs` | Validation: enforcing structure and module boundaries in CI or before merge. |

================================================================
File: packages/skills-catalog/skills/(architecture)/frontend-blueprint/SKILL.md
================================================================
---
name: frontend-blueprint
description: AI frontend specialist and design consultant that guides users through a structured discovery process before generating any code. Collects visual references, design tokens, typography, icons, layout preferences, and brand guidelines to ensure the final output matches the user's vision with high fidelity. Use when the user asks to build, design, create, or improve any frontend interface — websites, landing pages, dashboards, components, apps, emails, forms, modals, or any UI element. Also triggers on "build me a UI", "design a page", "create a component", "improve this layout", "make this look better", "frontend", "interface", "redesign", or when the user provides mockups, screenshots, or design references. Do NOT use for backend logic, API design, database schemas, or non-visual code tasks.
license: CC-BY-4.0
metadata:
  author: Felipe Rodrigues - github.com/felipfr
  version: 1.0.0
---

# Frontend Blueprint

You are a senior frontend design consultant — not a code generator. Your job
is to deeply understand what the user wants before writing a single line of
code. You ask the right questions, collect references, challenge vague
requests, suggest improvements, and only generate code when you have enough
context to be accurate on the first attempt.

Your target user is a fullstack developer who knows the basics of UI but is
not a design specialist. You bridge the gap between "I know what I want but
can't articulate it" and "pixel-perfect implementation".

## Core Principles

1. **Never generate code without context.** If the user says "build me a
   landing page" with no references, your first response is ALWAYS questions
   and reference requests — never code. A wrong first draft wastes more time
   than 2 minutes of discovery.

2. **References are non-negotiable.** Always ask for visual references before
   starting. The user may not know the right words, but they know what they
   like when they see it. Screenshots, URLs, Dribbble links, Figma exports,
   even "something like Apple's website" — anything concrete beats abstract
   descriptions.

3. **Atomic delivery.** Break every project into the smallest meaningful
   units. Deliver one piece, get approval, move to the next. Never generate
   a full page in one shot — it guarantees rework.

4. **Opinionated guidance.** You are NOT a passive executor. When the user's
   choices conflict with good design practices, say so. Suggest alternatives.
   Explain WHY. But ultimately respect their decision after informing them.

5. **Fidelity over speed.** The goal is to match the user's vision exactly,
   not to ship fast. Every token spent on discovery saves 10x in rework.

## Workflow

Every project follows this sequence. Do NOT skip phases. If the user tries
to jump ahead, explain briefly why the current phase matters and proceed.

```
BRIEFING → REFERENCES → DESIGN DIRECTION → [STITCH PROTOTYPING] → EXECUTION PLAN → ATOMIC BUILD → REVIEW
```

The Stitch Prototyping phase (in brackets) is conditional — triggered when
the user has no existing mockups or needs visual validation before code.
See Phase 4 for details.

### Phase 1: Briefing

Goal: Understand WHAT the user needs and WHY.

Ask conversationally (not as a checklist dump). Adapt based on project
complexity — a simple button needs 2 questions, a full app needs more.

Key areas to cover:

- **What** are you building? (page, component, app, redesign, etc.)
- **Who** is the end user? (audience, demographics, context of use)
- **What problem** does this solve? (not just "looks nice" — the actual goal)
- **Technical constraints?** (framework, existing design system, browser support, responsive requirements)
- **Existing assets?** (brand guidelines, color palette, logos, fonts already in use)
- **Deadline or scope?** (MVP vs polished, how much time to invest)

IMPORTANT: For simple requests (a single component, a small tweak), compress
this to 1-2 targeted questions. Don't over-process small tasks. Scale your
discovery to the project size.

### Phase 2: Reference Collection

Goal: Build a concrete visual vocabulary BEFORE any design decisions.

This is the most critical phase. Request references across these dimensions:

**Must collect (always ask):**

- Visual references: "Share 2-3 screenshots, URLs, or images of designs you
  like. They don't need to be the same type of project — if you like the
  typography of site A and the layout of site B, share both and tell me what
  you like about each."
- What specifically they like in each reference: colors? layout? typography?
  spacing? animations? overall mood?

**Collect when relevant (ask based on project scope):**

- Typography preferences: serif vs sans-serif, bold vs light, specific font
  names if they have preferences
- Icon style: outlined, filled, duotone, hand-drawn, geometric, a specific
  library (Lucide, Phosphor, Heroicons, etc.)
- Color direction: dark/light theme, warm/cool tones, specific brand colors,
  accent color preferences
- Imagery style: photography, illustrations, gradients, abstract, minimal
- Motion/animation: subtle micro-interactions, dramatic transitions, none
- Layout preferences: dense/spacious, symmetric/asymmetric, grid-based/organic

**How to handle "I don't know" responses:**
When the user can't provide references or is unsure, DON'T proceed blindly.
Instead:

1. Offer 2-3 contrasting directions with concrete descriptions
2. Use well-known sites as anchors: "More like Stripe (clean, spacious) or
   more like Bloomberg (dense, data-rich)?"
3. Ask elimination questions: "What do you definitely NOT want?"
4. If building for a known brand, research their existing visual identity

CRITICAL: Do not proceed to Phase 3 until you have at least ONE concrete
visual reference or a clearly articulated direction confirmed by the user.

**Stitch as a discovery tool:** If the user has no visual references AND
is not using Figma/Sketch/Adobe XD or similar design tools, suggest Google
Stitch (stitch.withgoogle.com) as a rapid prototyping tool. Frame it as a
time-saver: "Before we write code, I can generate prompts for Google Stitch
to quickly visualize what we're building. You'll see the actual design in
seconds and we avoid rework. Want to try it?" If the user is interested,
read `references/stitch-integration.md` and proceed to Phase 4 (Stitch
Prototyping) after Phase 3. If the user has Stitch MCP connected, you can
generate designs directly.

### Phase 3: Design Direction

Goal: Synthesize references into a clear, agreed-upon direction.

Before writing code, present a **Design Direction Summary**:

```
## Design Direction

**Mood:** [describe in 2-3 words — e.g., "clean and editorial"]
**Color palette:** [primary, secondary, accent, neutrals — hex codes]
**Typography:**
  - Headings: [font name, weight, style rationale]
  - Body: [font name, weight, style rationale]
**Layout approach:** [describe — e.g., "generous whitespace, card-based, 12-col grid"]
**Icon style:** [library + style]
**Key references applied:**
  - From [ref A]: [what you're taking — e.g., "the spacing rhythm and card design"]
  - From [ref B]: [what you're taking — e.g., "the color temperature and typography pairing"]
**Intentional departures:**
  - [anything you're suggesting differently from refs, and WHY]
```

Wait for explicit approval or adjustments before proceeding.

This is also where you provide **expert opinions**: if the user's references
conflict, if their color choices have accessibility issues, if their font
pairing doesn't work — say so now. Suggest improvements with clear reasoning.

### Phase 4: Stitch Prototyping (Conditional)

Goal: Visualize the design BEFORE writing any code.

This phase activates when:

- The user has no existing mockups (Figma, Sketch, etc.)
- The user is uncertain about direction and wants to see options
- The project has multiple screens or complex layouts
- The user explicitly wants to prototype first

Read `references/stitch-integration.md` before executing this phase.

**If Stitch MCP is connected (agent has access to Stitch tools):**

1. Create a Stitch project: `create_project(title: "Project Name")`
2. Create a Design System from the approved Design Direction (Phase 3),
   mapping color palette → `customColor`/`preset`, typography → `font`,
   dark/light → `colorMode`, border radius → `roundness`
3. Generate the first screen using `generate_screen_from_text` with a
   prompt built from the Design Direction. Use the prompt templates in
   `references/stitch-integration.md` Section 4.
4. Present the generated screenshot to the user for review
5. If the user wants alternatives: use `generate_variants` with
   appropriate `creativeRange` and `aspects`
6. If the user wants edits: use `edit_screens` with targeted, specific
   prompts (one change at a time)
7. Apply the design system to all screens for consistency
8. Once all screens are approved, extract HTML via `get_screen` to use
   as a reference in the Atomic Build phase

**If Stitch MCP is NOT connected (manual workflow):**

1. Ask if the user wants to set up MCP (offer setup guidance from
   `references/stitch-integration.md` Section 3 — it covers the generic
   config pattern and API Key method)
2. If they prefer manual: generate ready-to-paste prompts following the
   Stitch prompt formula: **Idea + Theme + Content + Image (optional)**
3. Guide the user through the Stitch workflow:
   - Paste the prompt at stitch.withgoogle.com
   - Choose device type (Mobile for apps, Web for websites/dashboards)
   - Generate, review, and share screenshots back
4. Generate targeted refinement prompts one at a time based on feedback
5. Suggest using **Variants** for comparison: "In Stitch, select the
   screen → Generate → Variants. Set Creative Range to Explore and
   generate 3 options."
6. Suggest using **Edit Theme** for quick adjustments: "Select the
   screen → Generate → Edit Theme to quickly tweak colors, font, dark
   mode, or corner radius."
7. Suggest creating a **Prototype** to test interactivity: "Select the
   screen → Generate → Prototype to see hover states and scroll behavior."
8. Once approved, user downloads HTML/images from Stitch for reference

**Prompt generation rules:**

- Follow the exact formula: Idea + Theme + Content
- Use UI/UX keywords: "navigation bar", "hero section", "card layout",
  "call-to-action button", "visual hierarchy", "drop shadow"
- Set the vibe with adjectives from the Design Direction mood
- Use the Style Word Bank for creative direction (Bento Grid, Editorial,
  Glassmorphism, Brutalist, Cyberpunk, etc.)
- If the user's chosen font is not in Stitch's 29 supported fonts,
  pick the closest match and note the substitution
- Keep prompts focused — one screen/section per generation
- Refinement prompts: one major change at a time, be specific about
  WHAT to change and HOW

**Exiting this phase:**
Proceed to Phase 5 when the user has approved visual designs for all
key screens. These become the source of truth for code generation.
If the user decides to skip Stitch at any point, proceed directly to
Phase 5.

### Phase 5: Execution Plan

Goal: Break the project into atomic, deliverable units.

Present a numbered list of components/sections to build, in dependency order:

```
## Execution Plan

I'll build this in [N] steps, each one reviewed before moving on:

1. **[Component/Section]** — [brief description, ~effort indicator]
2. **[Component/Section]** — [brief description]
3. **[Component/Section]** — [brief description]
...

Starting with #1. Ready?
```

Principles for the plan:

- Each step should produce something **visually reviewable**
- Dependencies first (design tokens/base styles → layout → components → details)
- Group logically but keep steps small enough that rework affects only one piece
- For large projects, suggest a phased approach (Phase A: core structure,
  Phase B: polish and animations, Phase C: responsive/edge cases)

### Phase 6: Atomic Build

Goal: Generate code one unit at a time, validated at each step.

If Stitch Prototyping (Phase 4) was completed, use the approved Stitch
screens as the primary visual reference. When Stitch MCP is available,
retrieve the HTML code via `get_screen` and use it as a structural
starting point — but always rewrite for the target framework, following
the agreed Design Direction tokens and the project's CSS architecture.
Stitch HTML is a reference, not copy-paste material.

For each unit in the execution plan:

1. **Generate the code** following the agreed design direction precisely
2. **Explain your choices** briefly — what you did and why (especially when
   you made subjective decisions)
3. **Highlight decision points** — anything that could go either way, present
   options: "I went with X here, but Y is also valid if you prefer Z"
4. **Proactive suggestions** — if you see an opportunity to improve beyond
   what was asked, suggest it: "This would look even better with a subtle
   hover animation — want me to add it?"

After presenting each unit, explicitly ask: "Does this match your vision?
Any adjustments before I move to the next step?"

CRITICAL: If the user requests changes, apply them to the CURRENT unit
before moving forward. Never accumulate "fix later" items.

### Phase 7: Review & Polish

Goal: Final quality pass on the complete deliverable.

Once all units are approved individually:

1. Present the **integrated result** (all components together)
2. Check for **visual consistency** across components (spacing rhythm,
   color usage, typography hierarchy)
3. Suggest **polish opportunities**: micro-interactions, transitions,
   responsive refinements, accessibility improvements
4. Provide a **final opinion** as a consultant: what's strong, what could
   be better in a future iteration, what to watch out for

## Reference Files

This skill includes deep-dive references. Load them ON DEMAND, not upfront:

- **`references/design-principles.md`** — Read during Phase 3 (Design Direction)
  or Phase 6 (Atomic Build) when you need specific guidance on typography
  pairing, color systems, spacing, layout patterns, accessibility, animation,
  or icon selection. Contains detailed rules and tables for each area.

- **`references/collection-guide.md`** — Read during Phase 2 (Reference
  Collection) when the user struggles to articulate preferences. Contains
  question strategies by user confidence level, contrast pairs for quick
  alignment, and design direction templates to anchor conversations.

- **`references/stitch-integration.md`** — Read when entering Phase 4
  (Stitch Prototyping) or when the user asks about Google Stitch, MCP
  setup, or visual prototyping. Contains: Stitch prompt formula and
  templates, Style Word Bank, Design Systems mapping, Variants workflow,
  device type guidance, complete MCP tools reference (14 tools), generic
  MCP setup pattern with examples, and troubleshooting guide.

## Expert Behavior Guidelines

As a consultant, always:

- **Challenge vagueness:** "Modern and clean" means nothing. Push for
  specifics: "Modern like Vercel's site or modern like Linear's?"
- **Name the tradeoffs:** "Dense layouts show more data but can overwhelm
  new users. Given your audience, I'd suggest..."
- **Teach while building:** Briefly explain design principles when relevant.
  The user is a fullstack dev learning design — help them grow.
- **Reference real examples:** When suggesting something, anchor it to a
  real site or product the user likely knows.
- **Catch anti-patterns:** If the user asks for 7 different fonts, red text
  on green background, or a carousel for 2 items — push back respectfully
  with reasoning.
- **Suggest what they didn't ask for:** If the design would benefit from
  something the user didn't mention (dark mode toggle, skeleton loading
  states, empty states), suggest it proactively.

## Technical Quality Standards

All generated code must:

- Use semantic HTML elements
- Follow accessibility basics (contrast ratios, focus states, alt text,
  ARIA labels where needed)
- Be responsive by default (mobile-first or specify breakpoints)
- Use CSS custom properties for theming (colors, spacing, typography)
- Include meaningful comments only where intent isn't obvious
- Use the framework/library the user specified (or ask if not specified)
- Avoid inline styles — use proper CSS architecture
- Prefer modern CSS (grid, flexbox, container queries, :has(), etc.)

## Scaling to Project Size

Not every request needs the full 7-phase treatment. Scale appropriately:

**Small (single component, quick fix):**

- Phases 1-2 compressed into 1-2 questions
- Skip Phase 4 (Stitch) and Phase 5 (no plan needed for one thing)
- Phase 3 can be a quick "I'll go with X approach, sound good?"

**Medium (page, multi-component feature):**

- Full Phase 1-2
- Phase 3 as described
- Phase 4 (Stitch): Suggest if user has no mockups — one or two screens
  to validate direction before coding
- Phase 5 with 3-6 steps

**Large (full app, design system, multi-page):**

- Deep Phase 1-2, potentially multiple rounds
- Phase 3 should be thorough with explicit sign-off
- Phase 4 (Stitch): Strongly recommend — generate key screens, use
  Design Systems for consistency, use Variants to explore directions.
  This is where Stitch saves the most time.
- Phase 5 broken into phases (A, B, C...)
- Consider suggesting a design tokens/foundation step first

## Examples

### Example 1: User with clear vision

User says: "Build me a pricing page. Here's Stripe's pricing page as
reference — I like the clean layout and the toggle between monthly/annual.
Our brand colors are #1a1a2e and #e94560. Use Inter for body, and something
bolder for headings."

Actions:

1. Briefing: Quick — they gave most context. Ask only: "How many tiers?
   Any specific features to highlight? Does the page need a FAQ section?"
2. References: Already provided. Ask: "Anything you DON'T like about
   Stripe's approach?"
3. Design Direction: Present summary with their colors, suggest a heading
   font pairing, confirm layout approach.
4. Execution Plan: [pricing toggle → tier cards → feature comparison → CTA]
5. Build each step, review each.

### Example 2: Vague request

User says: "I need a dashboard"

Actions:

1. Briefing: "What kind of dashboard? Analytics, admin panel, user-facing
   metrics? Who will use it? What data will it show?"
2. After answers, References: "Share 2-3 dashboards you like. Could be from
   any product — Notion, Linear, Vercel, or anything else. What specifically
   draws you to each?"
3. If user says "I don't know": Offer contrasts — "Here are 3 directions:
   (A) Data-dense like Grafana, (B) Clean and card-based like Vercel,
   (C) Minimal with focus on one key metric. Which resonates?"
4. Only proceed to Design Direction after concrete alignment.

### Example 3: Unsure user — Stitch prototyping flow

User says: "I need a dashboard but I'm not sure what I want"

Actions:

1. Briefing: Gather context — type of dashboard, audience, data to show
2. References: User can't provide any. Offer contrasts to narrow direction
3. Design Direction: Present summary based on alignment
4. Stitch Prototyping: "Since you don't have mockups, let's visualize
   this before coding. I'll generate Stitch prompts for 2-3 key screens."
   - Generate prompt: "A data analytics dashboard for SaaS metrics.
     Clean, minimal, light theme with blue accents. Sidebar navigation
     with Home, Analytics, Users, Settings. Main area with 4 KPI cards
     at top, a line chart showing monthly growth, and a data table below."
   - If MCP available: create project, design system, generate screen
   - If not: user pastes prompt in stitch.withgoogle.com
   - User reviews, requests "make the sidebar darker"
   - Generate edit prompt: "Change the sidebar background to a dark navy
     (#1a1a2e). Update sidebar text and icons to white."
   - Generate variants to compare layout options
   - User approves final version
5. Execution Plan: [design tokens → sidebar → KPI cards → chart → table]
6. Build each step using Stitch screenshot as reference

### Example 4: Redesign of existing UI

User says: "This component looks bad, make it better" [shares screenshot]

Actions:

1. Analyze current state: Identify specific issues (spacing, typography
   hierarchy, color contrast, layout problems)
2. Share analysis: "Here's what I see: [issues]. Before I fix it — what's
   the surrounding context? Are there brand guidelines to follow?"
3. Collect minimal references if none exist
4. Present 1-2 improvement directions, get alignment
5. Implement the chosen direction

## What This Skill is NOT

- Not a code-first generator — discovery always comes first
- Not limited to any framework — works with React, Vue, Svelte, plain
  HTML/CSS, or whatever the user needs
- Not just about "looking pretty" — good design solves problems
- Not a replacement for a design system — but can help build one
- Not a "make it pop" button — every decision has reasoning behind it
- Not dependent on Stitch — the full workflow works without it, but
  Stitch dramatically accelerates visual validation when available

================================================================
File: packages/skills-catalog/skills/(architecture)/legacy-migration-planner/SKILL.md
================================================================
---
name: legacy-migration-planner
description: Use when planning legacy system migrations, codebase modernization, monolith decomposition, microservices consolidation, cross-language rewrites, or framework upgrades. Invoke for strangler fig pattern, incremental migration strategy, or refactoring roadmaps. Do NOT use for domain analysis (use domain-analysis), component sizing (use component-identification-sizing), or step-by-step decomposition plans (use decomposition-planning-roadmap).
license: CC-BY-4.0
metadata:
  author: Felipe Rodrigues - github.com/felipfr
  version: 1.0.0
---

# Legacy Migration Planner

Senior migration architect that produces comprehensive, evidence-based migration plans using the Strangler Fig pattern. You create plans — you do not implement them. Other agents or developers execute the plan you produce.

## Core Principles

These are non-negotiable. Violating any of these invalidates your output.

1. **Never assume.** If you encounter an acronym, term, pattern, or technology you are not 100% certain about, stop and either research it (web search, context7) or ask the user. Say "I don't know what X means — can you clarify?" rather than guessing.
2. **Always cite evidence.** Every claim in your output must reference either a specific `file:line` from the user's codebase or a verified external URL. No unreferenced assertions.
3. **Always research before recommending.** Before suggesting any technology, pattern, or approach, use web search and context7 (when available) to verify it is current, maintained, and appropriate. Never recommend based solely on training data.
4. **Minimize token consumption.** Write output files per domain. Never dump entire file contents — reference by `file:line` ranges. Keep each output file focused on one bounded context.
5. **Direction-agnostic.** This skill handles ANY migration direction: monolith to microservices, microservices to modular monolith, microfrontends to SPA, cross-language, cross-framework, or any combination.

## Workflow

Every engagement follows two mandatory phases. Never skip RESEARCH. Never start PLAN without completing RESEARCH.

```
RESEARCH (mandatory)                    PLAN (mandatory)
├─ 1. Codebase deep analysis            ├─ 5. Define migration direction
├─ 2. Domain/bounded context mapping    ├─ 6. Design seams and facades
├─ 3. Stack research (web + context7)   ├─ 7. Per-domain migration files
└─ 4. Risk and dependency mapping       └─ 8. Consolidated roadmap
│                                        │
└─ Output: ./migration-plan/research/   └─ Output: ./migration-plan/domains/
```

### RESEARCH Phase

Load `references/research-phase.md` for detailed instructions.

1. **Analyze the codebase** — Read the project structure, entry points, configuration files, and dependencies. Map every module and its responsibility. Cite every finding as `file:line`.
2. **Identify bounded contexts** — Group related modules into candidate domains. Load `references/assessment-framework.md` for the domain identification method.
3. **Research current and target stacks** — Use web search and context7 to gather up-to-date documentation on both the current stack and the target stack (if migrating cross-framework/language). Document version compatibility, migration guides, and known pitfalls.
4. **Map risks and dependencies** — Identify integration points, shared databases, circular dependencies, and external service couplings. Load `references/assessment-framework.md` for the risk matrix method.

Output: Write findings to `./migration-plan/research/` with one file per concern (e.g., `dependency-map.md`, `domain-candidates.md`, `stack-research.md`, `risk-assessment.md`).

### PLAN Phase

Load `references/plan-phase.md` for detailed instructions.

5. **Define migration direction** — Based on RESEARCH findings, determine the appropriate strategy. Load `references/strangler-fig-patterns.md` for pattern selection.
6. **Design seams and facades** — Identify where to cut the system. Define the facade/router layer that will enable incremental migration. Load `references/frontend-backend-strategies.md` for stack-specific patterns.
7. **Write per-domain migration plans** — One file per bounded context in `./migration-plan/domains/`. Each file contains: current state (with file:line refs), target state, migration steps, testing strategy (load `references/testing-safety-nets.md`), rollback plan, and success metrics.
8. **Write consolidated roadmap** — `./migration-plan/00-roadmap.md` with phase sequencing, dependencies between domains, risk mitigation timeline, and success criteria.

## Output Structure

```
./migration-plan/
├── 00-roadmap.md                    # Consolidated roadmap, phases, timeline
├── research/
│   ├── dependency-map.md            # Module dependencies with file:line refs
│   ├── domain-candidates.md         # Identified bounded contexts
│   ├── stack-research.md            # Current + target stack analysis
│   └── risk-assessment.md           # Risk matrix with mitigations
└── domains/
    ├── 01-domain-{name}.md          # Per-domain migration plan
    ├── 02-domain-{name}.md
    └── ...
```

## Reference Guide

Load references based on the current phase and need. Do not preload all references.

| Topic                   | Reference                                   | Load When                                                |
| ----------------------- | ------------------------------------------- | -------------------------------------------------------- |
| Research methodology    | `references/research-phase.md`              | Starting RESEARCH phase                                  |
| Plan methodology        | `references/plan-phase.md`                  | Starting PLAN phase                                      |
| Strangler Fig patterns  | `references/strangler-fig-patterns.md`      | Choosing migration pattern, designing seams              |
| Assessment and risks    | `references/assessment-framework.md`        | Mapping dependencies, scoring risks, identifying domains |
| Testing strategies      | `references/testing-safety-nets.md`         | Designing safety nets for each domain                    |
| Stack-specific patterns | `references/frontend-backend-strategies.md` | Frontend or backend migration specifics                  |

## Constraints

### MUST DO

- Research every technology recommendation via web search before including it
- Use context7 for library documentation when available
- Cite `file:line` for every codebase observation
- Ask the user when encountering unknown terms, acronyms, or ambiguous requirements
- Produce one output file per domain to keep context manageable
- Include rollback strategy for every migration step
- Validate that current stack versions match what is actually in the codebase (package.json, requirements.txt, etc.)

### MUST NOT DO

- Guess the meaning of acronyms, internal terms, or business logic
- Recommend technologies without web search verification
- Write implementation code (this skill produces plans, not code)
- Assume migration direction without evidence from RESEARCH
- Skip the RESEARCH phase or combine it with PLAN
- Reference files or lines that were not actually read
- Include unreferenced claims in any output file

================================================================
File: packages/skills-catalog/skills/(architecture)/modular-decomposition/SKILL.md
================================================================
---
name: modular-decomposition
description: Runs a sequenced monolith-to-modular pipeline that sizes and inventories components, finds shared domain duplication, addresses flattening and hierarchy issues, analyzes coupling, then groups components into candidate domain-aligned units, with optional embedded DDD strategic analysis for bounded contexts. Use when asking how to split a monolith, size components before extraction, find duplicated domain logic, clean up module hierarchy, measure coupling between modules, or group components into services. Do NOT use for phased extraction roadmaps or prioritization without the prior analysis steps (use decomposition-planning-roadmap after this pipeline), end-to-end legacy migration strategy writeups (use legacy-migration-planner), pure infrastructure capacity sizing, or when you only need DDD without the structural pipeline (install domain-analysis standalone).
---

# Modular Decomposition

This skill runs the **Patterns 1–5** analysis pipeline before service extraction. Each pattern is plain markdown under `references/`; load the file for that step and execute it against the user’s codebase.

## How to Use

### Quick start (what users can say)

- **Full pipeline:** “Run modular decomposition Patterns 1 through 5 on this repo,” “Analyze this monolith for splitting—inventory, coupling, and domain grouping.”
- **Single early step:** “Identify and size components here,” “Find duplicated domain logic across modules,” “Analyze coupling between our packages.”
- **With DDD lens:** “Group components into domains and check bounded contexts,” “Use DDD strategic design on this codebase before we group services.”

If the user only wants **extraction order, phases, or migration roadmap** after analysis exists, use **decomposition-planning-roadmap** instead. If they need a **full legacy migration plan** (strangler fig, research, multi-stack), use **legacy-migration-planner** as well or instead of this skill when that is the primary ask.

### How the agent should run it

1. **Scope:** Confirm the task is structural analysis (inventory → coupling → grouping), not roadmap authoring. If unclear, ask once whether they want the full ordered pipeline or a subset.
2. **Order:** Run patterns **1 → 2 → 3 → 4 → 5** in that order. Do not skip a step unless the user explicitly limits scope; if they do, state which patterns were skipped and how that limits later conclusions.
3. **Load references:** For each pattern, open the matching `references/pattern-NN-*.md` file and follow its instructions. Use the optional `*-quick-reference.md` for the same number when a short checklist is enough.
4. **Carry context forward:** Reuse outputs from earlier patterns in later ones (e.g. component inventory from Pattern 1 informs coupling in 4 and grouping in 5). Reference concrete paths, modules, or tables from previous steps.
5. **Domain language (Pattern 5):** If subdomains or bounded contexts need grounding beyond structure, read `references/domain-analysis.md` **before or alongside** Pattern 5. Optionally open `references/domain-analysis-quick-reference.md` or `references/domain-analysis-examples.md` for condensed rules or illustrations.
6. **Deliver:** Produce clear, actionable findings per pattern or one consolidated report—always tied to evidence from the repository (files, dependencies, metrics), not generic advice.

### Usage examples

**Example 1 — Full pipeline**

```
User: "We're going to split this monolith—run the full decomposition analysis (Patterns 1–5)."

Agent: Execute patterns 1→5 in order, loading each references/pattern-NN-*.md, preserving outputs between steps, then summarize cross-cutting recommendations.
```

**Example 2 — Coupling after inventory**

```
User: "We already have a rough module list—focus on coupling (Pattern 4) and then domain grouping (Pattern 5)."

Agent: If no prior inventory exists in the thread, either run Pattern 1 briefly or derive an explicit module list from the repo before 4 and 5. State any assumptions.
```

**Example 3 — DDD before grouping**

```
User: "Map bounded contexts and language, then group components into domains."

Agent: Read references/domain-analysis.md (and optional quick reference/examples) in parallel with or immediately before Pattern 5; align Pattern 5 groupings with linguistic boundaries where evidence supports it.
```

## Prerequisites

- Complete **Pattern N** before starting Pattern N+1 unless the user explicitly narrows scope. Later patterns depend on earlier results (for example, inventory and structure inform coupling and grouping).
- If business vocabulary, subdomains, or bounded contexts are uncertain, use `references/domain-analysis.md` **before or alongside Pattern 5** (see Bounded contexts below).

## Ordered workflow (Patterns 1–5)

| Step | Pattern                            | Primary reference                                                                                          |
| ---- | ---------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| 1    | Identify and size components       | `references/pattern-01-identify-and-size.md` (optional: `pattern-01-identify-and-size-quick-reference.md`) |
| 2    | Common domain detection            | `references/pattern-02-common-domain.md` (optional: `pattern-02-common-domain-quick-reference.md`)         |
| 3    | Flattening / hierarchy             | `references/pattern-03-flattening.md` (optional: `pattern-03-flattening-quick-reference.md`)               |
| 4    | Coupling analysis                  | `references/pattern-04-coupling.md`                                                                        |
| 5    | Domain identification and grouping | `references/pattern-05-domain-grouping.md` (optional: `pattern-05-domain-grouping-quick-reference.md`)     |

## Pattern 6 — planning and extraction

**Pattern 6** (_create domain services / extraction_) is not duplicated here. After Pattern 5, switch to **decomposition-planning-roadmap** for phased extraction order, milestones, and migration-style planning. For full legacy migration strategy (strangler-fig, cross-stack rewrites, research-heavy plans), optionally use **legacy-migration-planner** in addition.

## Bounded contexts and DDD strategic design

- **Patterns 1–4** focus on **structural** inventory, duplication, hierarchy, and **coupling** between parts of the codebase.
- **Pattern 5** produces **candidate** groupings aligned with **solution-space** boundaries (which components belong together as services).
- **Strategic DDD** (subdomains, bounded contexts, ubiquitous language) is covered in `references/domain-analysis.md`, with optional `domain-analysis-quick-reference.md` and `domain-analysis-examples.md`. Use it when you need to validate or refine boundaries against business language, not only folder structure.

================================================================
File: packages/skills-catalog/skills/(architecture)/modular-design-principles/SKILL.md
================================================================
---
name: modular-design-principles
description: >
  Technology-agnostic guidance for modular systems: bounded contexts, clear boundaries,
  composability, state isolation, explicit contracts, failure containment, scaffolding workflows,
  split/merge criteria, sub-units inside a context, and compliance review signals. Use when
  designing or reviewing module structure, service boundaries, package layout, cross-cutting
  dependencies, "how should we split this?", modularity assessments, coupling between domains,
  greenfield context design, or architecture discussions without assuming a specific framework,
  language, or repository layout. Do NOT use for executing the full Patterns 1–5 repo
  decomposition pipeline or per-pattern inventories (use modular-decomposition), phased
  extraction roadmaps as the main deliverable (use decomposition-planning-roadmap), or
  end-to-end legacy migration strategy (use legacy-migration-planner).
---

# Modular Design Principles

Use this skill when reasoning about **structure and boundaries** in any codebase. It intentionally avoids framework names, folder conventions, and tooling — map principles to your stack locally.

## What to load

| Task | Where |
|------|--------|
| Principles table + violations + workflows (this file) | `SKILL.md` |
| Per-principle definition, agent rules, abstract examples | `references/principles.md` |

---

## Layered mental model

- **Composition roots** (applications, hosts, runners): wire modules together; keep orchestration thin.
- **Modules / bounded contexts**: cohesive units of behavior and data ownership; each should be understandable and testable on its own.
- **Shared kernels** (use sparingly): only stable, truly cross-cutting concepts; resist turning them into a grab-bag of “everything everyone needs.”

How you physically lay this out (mono repo, multi repo, packages, libraries) is a **delivery choice**, not the definition of modularity. The principles below still apply.

---

## The ten principles

| # | Principle | Intent |
|---|-----------|--------|
| 1 | **Well-defined boundaries** | A small, stable **public surface**; everything else is internal. Consumers depend on contracts, not internals. |
| 2 | **Composability** | Modules can be used alone or combined without special knowledge of each other’s internals. |
| 3 | **Independence** | No hidden shared mutable state across boundaries; each module should be testable in isolation (with fakes or test doubles at the edges). |
| 4 | **Individual scale** | Resources (compute, storage, rate limits, batch size) can be tuned **per module** where it matters, without rewriting others. |
| 5 | **Explicit communication** | Cross-module interaction uses **documented contracts** (APIs, events, messages, shared types) — not incidental coupling. |
| 6 | **Replaceability** | Dependencies on other modules are expressed through **interfaces or protocols** so implementations can change. |
| 7 | **Deployment independence** | Modules do not assume they share a process, host, or release cadence unless that is an explicit architectural decision. |
| 8 | **State isolation** | Each module **owns** its persistent state and naming; no silent sharing of the same logical data store or ambiguous global names across boundaries. |
| 9 | **Observability** | Each module can be diagnosed on its own: logs, metrics, traces, health — attributable to the unit that emitted them. |
| 10 | **Fail independence** | Failures are **contained** (timeouts, bulkheads, circuit breaking, idempotency) so one module’s outage does not blindly cascade. |

**Principle 8** is often the hardest: ambiguous ownership of data or names is a frequent source of “works until it doesn’t” integration bugs.

For **depth** (rules for agents + abstract examples per principle), load `references/principles.md`.

---

## Typical violations (stated abstractly)

1. **Colliding concepts** — the same name or schema for different things in different modules, or duplicate “global” definitions that diverge over time.
2. **Reach-through persistence** — one module reading or writing another module’s tables, buckets, or documents **without** going through an agreed contract.
3. **Centralized data ownership** — a single persistence layer that registers and exposes **all** stores for **all** modules, encouraging hidden coupling.
4. **Logic at the edge** — business rules in transport adapters (HTTP handlers, UI, CLI) instead of domain/application code.
5. **Edge talking to storage directly** — adapters depending on low-level persistence APIs instead of use cases or application services.
6. **Unscoped transactions** — writes that span boundaries without clear transaction ownership and failure semantics.
7. **Leaky exports** — repositories, internal services, or implementation types exposed as the module’s public API.
8. **Facades that aren’t thin** — “public” entry points that embed querying, mapping, or policy instead of delegating to the right layer inside the module.

---

## Creating a bounded context (workflow)

Use when introducing a **new** cohesive area of the system (greenfield module or extracted domain).

1. **Scope and language** — Name the context; list core nouns/verbs (**ubiquitous language**). Reject vague names that collide with other contexts.
2. **Responsibilities** — What decisions happen **only** here? What is explicitly *out* of scope?
3. **State ownership** — Which facts are **authoritative** in this context? Where are they stored conceptually (even if storage tech is undecided)?
4. **Public contract** — Operations and/or events other contexts may use. Version or evolve this contract intentionally.
5. **Integrations** — For each neighbor: sync call, async message, shared read model, or batch sync? Document **consistency** (immediate, eventual) and **failure** behavior.
6. **Invariants and lifecycles** — What must always be true inside this boundary? What starts/completes a lifecycle?
7. **Isolation check** — Can you test core behavior **without** spinning up unrelated contexts (fakes at ports)?
8. **Observability** — How will you trace a request or job through **this** context with clear identifiers?

**Cross-module interaction** (while designing): prefer the **minimal** contract; define **timeouts**, **retries**, **idempotency** for async; avoid “temporary” direct store access as a shortcut.

---

## When to split or merge

**Default:** **fewer boundaries** until real pain appears — “flat is often better” than premature fragmentation. Splitting adds coordination, versioning, and operational cost.

### Six-criteria test (favor split when several are true)

| # | Criterion | Question |
|---|-----------|----------|
| 1 | **Language** | Do the sub-areas use **different vocabulary** or conflicting definitions of the same word? |
| 2 | **Rate of change** | Do parts **change on different cadences** or for unrelated reasons (most edits touch one side)? |
| 3 | **Scale / SLO** | Do parts need **different** throughput, latency, or availability targets? |
| 4 | **Consistency** | Do they need **different transaction boundaries** (cannot share one atomic write model cleanly)? |
| 5 | **Ownership** | Would **different teams** or clear ownership lines reduce conflict and review churn? |
| 6 | **Pain signal** | Is there **observable** integration pain: ripple effects, fear of change, unclear who owns a bug? |

**Cohesion / coupling (qualitative).** Favor **high cohesion** inside a module and **low, explicit coupling** between modules. If the only motivation is “files got big” or “folder aesthetics,” **merge or wait**.

### When to merge or not split yet

- Boundaries are **artificial** (same language, same lifecycle, constant cross-calls).
- Splitting would **duplicate** logic or data without a clear **single writer** rule.
- Team is not ready to **own** contracts, versioning, and ops for extra units.

### Decision prompts (short)

- Would separation **reduce** accidental coupling more than it **increases** coordination cost?
- Is there a natural **ubiquitous language** boundary, or only a technical seam?

---

## Sub-units inside a bounded context

Sometimes one outer boundary is right, but inside it there are **named sub-areas** (subdomains, feature areas). Principles still apply **within** the context.

**Ownership**

- Each sub-unit should **own** its slice of model and persistence concerns where possible — avoid one mega registration layer that wires **every** store and repository for **every** sub-unit in one place (encourages reach-through and hidden coupling).

**Cross-sub-unit access**

- Prefer **internal application APIs** or **thin internal facades** (same context, explicit surface) over peers importing each other’s storage types directly.
- For async flows, prefer **enriched payloads** so handlers do not **chat** across sub-units for data that could travel with the event/command.

**Shared kernel inside the context**

- Small, stable shared types or enums can live in a **narrow shared area** — but resist a growing “utils” dump that becomes the real coupling point.

**Anti-pattern:** A single “persistence” or “data” sub-module that becomes the **only** place that knows about all tables/documents for all sub-units, and everyone else reaches through it — same problems as cross-context reach-through, **inside** the boundary.

---

## Architecture compliance pass

Use for **reviews** or **audits** without assuming tooling. Treat items as **signals**, not proof — confirm with domain experts.

### Dependency and API signals

- **Inbound vs outbound:** Dependencies should align with your chosen architecture (e.g. domain at the center, adapters outside). **Inward** leaks of infrastructure types into core logic are a smell.
- **Public surface:** Can you list **exported** operations/events/types without including storage or internal services? If not, boundaries are leaky.
- **Neighbor imports:** Types or clients from **module A** used in **module B** — are they only **contract** types, or persistence/implementation types?

### Persistence and data signals

- **Reach-through:** References to another context’s **physical** data (schema, collection, bucket name) outside an agreed contract.
- **Naming collisions:** Same logical name for different things, or shared global IDs without a documented mapping rule.
- **Transaction ownership:** Writes that span contexts without a clear **saga**, **outbox**, or **single-owner** rule and documented failure cases.

### Operational signals

- **Blame:** Incidents where “we don’t know which module owns this row/behavior” → ownership or observability gap.
- **Cascades:** One dependency’s slowdown or failure takes down unrelated user journeys → missing **timeouts**, **bulkheads**, or **degradation** paths.

### Severity heuristic (for reporting)

| Tier | Meaning |
|------|--------|
| **P0** | Data corruption risk, security boundary violation, or cross-context persistence with no contract |
| **P1** | Unclear ownership, leaky public API, missing failure semantics at boundaries |
| **P2** | Observability gaps, composability smells, tech debt that increases future coupling |

**Maturity note:** Scoring is **qualitative** unless the team defines numeric gates. Use trends: fewer P0/P1 over time, clearer contracts.

---

## Quick checklist (before proposing structure)

- [ ] Public API is minimal; internals are not exported casually.
- [ ] Names and storage ownership are unambiguous per module.
- [ ] No cross-module persistence shortcuts without an explicit contract.
- [ ] Business rules sit behind a clear application/domain layer, not only in adapters.
- [ ] Cross-module calls have explicit failure and timeout behavior.
- [ ] Observability can answer “which module failed and why?” without spelunking.
- [ ] If the context has sub-units: each has clear ownership; no monolithic “registers everything” persistence grab-bag.

---

## Relationship to stack-specific skills

When a project has **concrete conventions** (framework modules, DI, repository patterns, folder layout, codegen, CI checks), prefer those documents for **how** to implement. Use **this** skill for **why** boundaries exist and **what** good modular design optimizes for — so stack-specific advice stays aligned with the same principles.

================================================================
File: packages/skills-catalog/skills/(architecture)/react-composition-patterns/SKILL.md
================================================================
---
name: react-composition-patterns
description: React composition patterns that scale. Use when refactoring components with boolean prop proliferation, building flexible component libraries, or designing reusable APIs. Triggers on tasks involving compound components, render props, context providers, or component architecture. Includes React 19 API changes. Do NOT use for React/Next.js performance optimization (use react-best-practices instead).
license: MIT
metadata:
  author: vercel
  version: '1.0.0'
---

# React Composition Patterns

Composition patterns for building flexible, maintainable React components. Avoid
boolean prop proliferation by using compound components, lifting state, and
composing internals. These patterns make codebases easier for both humans and AI
agents to work with as they scale.

## When to Apply

Reference these guidelines when:

- Refactoring components with many boolean props
- Building reusable component libraries
- Designing flexible component APIs
- Reviewing component architecture
- Working with compound components or context providers

## Rule Categories by Priority

| Priority | Category                | Impact | Prefix          |
| -------- | ----------------------- | ------ | --------------- |
| 1        | Component Architecture  | HIGH   | `architecture-` |
| 2        | State Management        | MEDIUM | `state-`        |
| 3        | Implementation Patterns | MEDIUM | `patterns-`     |
| 4        | React 19 APIs           | MEDIUM | `react19-`      |

## Quick Reference

### 1. Component Architecture (HIGH)

- `architecture-avoid-boolean-props` - Don't add boolean props to customize
  behavior; use composition
- `architecture-compound-components` - Structure complex components with shared
  context

### 2. State Management (MEDIUM)

- `state-decouple-implementation` - Provider is the only place that knows how
  state is managed
- `state-context-interface` - Define generic interface with state, actions, meta
  for dependency injection
- `state-lift-state` - Move state into provider components for sibling access

### 3. Implementation Patterns (MEDIUM)

- `patterns-explicit-variants` - Create explicit variant components instead of
  boolean modes
- `patterns-children-over-render-props` - Use children for composition instead
  of renderX props

### 4. React 19 APIs (MEDIUM)

> **⚠️ React 19+ only.** Skip this section if using React 18 or earlier.

- `react19-no-forwardref` - Don't use `forwardRef`; use `use()` instead of `useContext()`

## How to Use

Read individual rule files for detailed explanations and code examples:

```
rules/architecture-avoid-boolean-props.md
rules/state-context-interface.md
```

Each rule file contains:

- Brief explanation of why it matters
- Incorrect code example with explanation
- Correct code example with explanation
- Additional context and references

## Full Compiled Document

For the complete guide with all rules expanded: `AGENTS.md`

================================================================
File: packages/skills-catalog/skills/(architecture)/tactical-ddd/SKILL.md
================================================================
---
name: tactical-ddd
description: Detects anemic domain models, validates and refactors them into rich domain models, and enforces tactical DDD patterns (Entities, Value Objects, Aggregates, Domain Services, Domain Events). Use when the user asks to validate, review, or check domain models or DDD code; detect anemia; refactor domain objects; improve encapsulation; or mentions terms like "anemic model", "rich domain", "aggregate", "value object", "domain event", "ubiquitous language", "is this good DDD", "does this follow DDD", or "check my domain". Do NOT use for module or service boundary design, architectural decomposition, strategic DDD context mapping, or code outside the domain layer (DTOs, controllers, infrastructure adapters).
---

# Tactical DDD — Rich Domain Modeling

## Workflow

Determine the user's intent first:

| Intent | Phases to run |
|--------|--------------|
| "validate / review / check / is this correct?" | Phase 1 + 2 only → report findings, ask before refactoring |
| "fix / refactor / improve / clean up" | Phase 1 + 2 + 3 |
| "how should I design / model this?" | Load [reference.md](reference.md) directly |

### Phase 1 — Detect
Load [detection.md](detection.md) and scan the target code for anemia signals. Produce a severity score and list of affected classes.

### Phase 2 — Assess
For each affected class, determine the correct building block:

| Has unique identity tracked over time? | Has invariants tying multiple objects? | → Building Block |
|----------------------------------------|----------------------------------------|-----------------|
| Yes | — | **Entity** |
| No | — | **Value Object** |
| Yes (root) + children with shared invariants | Yes | **Aggregate** |
| Operation spans multiple Aggregates/doesn't belong to any | — | **Domain Service** |

Prefer Value Objects over Entities. Prefer small Aggregates over large ones.

**If intent was validate/review**: stop here. Report findings using the output format below. Ask "Would you like me to apply these fixes?" before proceeding.

### Phase 3 — Refactor
Load [refactoring.md](refactoring.md) for step-by-step moves. Apply in this order:
1. Replace setter chains with a single expressive method
2. Move service logic into the Aggregate that owns it
3. Add business guards at the top of each method
4. Publish a Domain Event after each successful state change
5. Replace primitive types with Value Objects

For deep pattern questions (boundary design, event modeling, service vs. entity decision), load [reference.md](reference.md).

---

## Quick Anemia Signals (scan first)

```
public setX() / public setY()        → behaviour should be encapsulated
service.doX(entity, ...)              → logic likely belongs in entity
entity.setA(); entity.setB(); ...     → setter chain = missing intent method
no domain methods beyond getters      → pure data bag
```

---

## Golden Rules

1. **Behaviour with data** — Objects own both state and the operations that change it
2. **Ubiquitous Language** — Method names come from the domain, not CRUD (`commitTo`, not `setStatus`)
3. **Small Aggregates** — Root + Value Objects by default; add child Entities only for true invariants
4. **One transaction = one Aggregate** — Cross-Aggregate rules use eventual consistency via Domain Events
5. **Reference by ID** — Never hold object references to other Aggregates
6. **Value Objects first** — Use Entities only when individual identity is essential
7. **Domain Services sparingly** — Excessive services → anemic model
8. **Protect invariants** — The Aggregate is the last line of defence; never trust the caller

---

## Output Format

When reviewing code, report:

```
## Anemia Diagnosis: <ClassName>

Severity: [None | Mild | Moderate | Severe]

Issues:
- <description of problem>

Recommended refactoring:
- <specific move from refactoring.md>
```

When refactoring, show a before/after diff for each class touched.
