---
name: zinsser-audit
description: Audits professional prose for cadence, AI-generated slop, and clarity. Use when editing technical or business documents, including design docs, PRDs, BRDs, ADRs, plans, runbooks, SOPs, and service-transition documents.
---

# Zinsser Prose Audit

This skill provides the operational framework for using the `zinsser` CLI to recover a human voice and executive clarity in professional writing.

## Core Workflow

Follow this sequence for every audit:

1. **Audit**: Run `zinsser check <file>` to identify style and cadence violations.
2. **Analyze**: Map the `rule_id` to the remediation strategy in [references/REMEDIATION.md](references/REMEDIATION.md).
3. **Remediate**: Apply the fix (e.g., front-load the verb, insert a punch-line).
4. **Verify**: Run `zinsser score <file>` to ensure the Craft Grade has improved.

## Specialized Audits

**Technical Specifications**:
When auditing specs, consult the [RFC 8174 Guidelines](references/RFC8174.md) to ensure structural compliance and correct use of requirement language (MUST/SHOULD).

**Business & Product Prose (PRDs, Charters, Visions, Reports)**:
When auditing business-facing documents, consult the [Business Prose Guidelines](references/BUSINESS_PROSE.md) to apply the Pyramid Principle, BLUF/TL;DR constraints, and replace corporate puffery with concrete value.

**Operational Documents (Designs, Runbooks, SOPs, Service Transitions)**:
Consult [Operational Document Review](references/OPERATIONS_PROSE.md). Preserve execution-critical details; report ambiguity rather than inventing procedures or claiming technical completeness.

**Philosophy & Standards**:
For the core principles of Anti-Slop and Structural Craft, see [references/PHILOSOPHY.md](references/PHILOSOPHY.md).

## Tooling
Commands are executed as bash calls to the `zinsser` binary.
- `zinsser check <path>`: Returns a list of `Diagnostic` objects.
- `zinsser score <path>`: Returns a `DocumentScore` scorecard.
