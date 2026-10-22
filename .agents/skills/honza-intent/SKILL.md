---
name: honza-intent
description: Resolve ambiguity in Honza Kristek's Czech requests or corrections after a misunderstanding. Use automatically when intended meaning remains unclear; typos alone and clear requests do not need this skill.
---

# Understand Honza's intent

Recheck nearby messages and relevant inspected artifacts before asking Honza to repeat himself. Treat typo-heavy Czech as normal; do not infer personality traits or extra requirements.

- Preserve the active goal. Apply later corrections to the affected details unless Honza replaces the goal.
- Resolve references such as “it”, “those edges” and “like the picture”. Distinguish reported symptoms from suggested causes.
- Preserve quantities, lower/upper bounds, negations, discussion-only requests and ownership boundaries. Never turn discussion into implementation.
- Separate requirements, verified facts and assumptions. For a safe likely interpretation, briefly state the assumption when useful and continue authorized work. If competing meanings materially change the action, ask one focused question before dependent work.

Example: “tlustsi ty trubky aspon trikrat, ne vetsi cisla” means tube diameter at least 3× the current diameter, with digit height preserved.

Only when a brief helps execution or handoff, summarize outcome, relevant state, constraints and acceptance criteria. Prefer exact references over copied history; omit unrelated detail without erasing uncertainty.

Reply concisely in Czech; technical handoffs may use English. Do not routinely print rewritten prompts, launch an interpretation agent, escalate models or retrieve old chats. Reuse this interpretation during the same task; do not reread unchanged instructions every turn. Follow project guidance. Do not claim measured savings without evidence.
