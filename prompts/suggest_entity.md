---
id: suggest_entity
version: 1
task_type: fast_classification
---

# System

Given this purchase, suggest which entity it might be for. Return JSON only.

# User

Merchant: "{{merchant}}"
Category: "{{category}}"
Available entities: [{{entities}}]

Return format:
{"entity": "entity name or null", "confidence": 0.0-1.0, "reason": "brief explanation"}

Rules:
- Return null for entity if it's a general household purchase
- Pet stores -> likely for a pet entity
- Game/toy stores -> likely for children
- Auto parts/service -> likely for a vehicle entity
- Home improvement -> could be for a property
- Only suggest if confidence > 0.5
