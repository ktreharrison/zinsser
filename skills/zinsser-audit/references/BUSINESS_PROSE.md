# Business & Product Prose Guidelines

When auditing non-engineering documents (PRDs, BRDs, Project Charters, Vision Docs, ADRs, Reports, and Emails), the goal is to move from "Corporate-Speak" to "Executive Clarity."

## 1. The Information Hierarchy (The Pyramid Principle)
Executive communication must be "Top-Down." Do not build up to a conclusion; start with it.
- **The Rule:** The "Peak" (the conclusion, the ask, or the main finding) must be the first thing the reader sees.
- **The Structure:**
    1. **The Peak:** The core conclusion or recommendation.
    2. **The Pillars:** The 3-5 key supporting arguments.
    3. **The Base:** The raw data and detailed evidence.
- **Agent Fix:** If the document is "Bottom-Up" (details first, conclusion last), restructure it to lead with the conclusion.

## 2. The TL;DR & One-Pager Constraints
Attention is the scarcest resource in the enterprise.
- **The TL;DR Protocol:** Every document over 500 words MUST have a 1-2 sentence "Bottom Line" at the top.
- **The One-Pager Pressure:** If a document is sprawling, identify "bloat" sections. Convert narrative descriptions into concise tables or bulleted lists to strive for a one-page summary.
- **Agent Fix:** Synthesize a TL;DR from the existing content and flag sections that can be compressed into a "one-pager" format.

## 3. The "BLUF" Protocol (Emails & Slack)
Professional communication should follow the **Bottom Line Up Front (BLUF)** pattern.
- **The Rule:** The "Ask" or the "Conclusion" must appear in the first two sentences.
- **Agent Fix:** Move the call-to-action (CTA) from the end of the email to the top. Remove social filler ("I hope this finds you well") if it delays the core message by more than one sentence.

## 4. Outcome vs. Activity (Monthly Reports)
Reports often fail by listing *what was done* (activity) instead of *what changed* (outcome).
- **The Rule:** Every bullet point must lead with a result, not a task.
- **Bad:** "Held three meetings with the marketing team to discuss the new landing page." (Activity)
- **Zinsser-Fixed:** "Increased landing page conversion by 12% after aligning with the marketing team." (Outcome)

## 5. Replacing Puffery with Metrics
Business prose often uses "Magnitude Words" to hide a lack of data.
- **Forbidden Words:** *Significantly, substantially, exponentially, vastly, cutting-edge, seamless, robust.*
- **The Fix:** Replace the magnitude word with a number, a percentage, or a concrete comparison.
- **Example:** "Significantly reduce churn" $\rightarrow$ "Reduce churn from 5% to 2%."

## 6. The "Actionable" PRD/BRD
A requirement is not a requirement if it cannot be verified.
- **The Rule:** Every "feature" must be described as a user action and a measurable outcome.
- **Agent Fix:** Flag vague goals ("Improve user experience") and prompt for a verifiable metric ("Reduce checkout time by 2 seconds").

## 7. ADRs (Architecture Decision Records)
ADRs must be a record of *thought*, not just a record of *result*.
- **The Rule:** The "Context" section must explicitly state the trade-offs considered.
- **Agent Fix:** If the ADR only lists the chosen solution, flag it as "Lacking Trade-off Analysis." Prompt the agent to ask: "What were the two other options we rejected, and why?"

## 8. The "Vision" vs. "Fluff" Balance
A Vision document should be inspiring, but not hollow.
- **The Rule:** For every "Visionary" statement, there must be one "Concrete" pillar.
- **Agent Fix:** If the document is a string of superlatives, prompt the agent to insert a "How we actually do this" section.
