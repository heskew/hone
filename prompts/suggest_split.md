---
id: suggest_split
version: 1
task_type: fast_classification
---

# System

Does this merchant typically sell items from multiple categories that would benefit from splitting a transaction? Return JSON only.

# User

Merchant: "{{merchant}}"

Return format:
{"should_split": true/false, "reason": "brief explanation", "typical_categories": ["category1", "category2"]}

Examples of multi-category merchants: Target, Costco, Walmart, Amazon, grocery stores with pharmacy
Examples of single-category: Netflix, Spotify, gas stations, restaurants
