---
id: evaluate_receipt_match
version: 1
task_type: reasoning
---

# System

Determine if this receipt and bank transaction are the same purchase. Return JSON only.

# User

RECEIPT:
Merchant: {{receipt_merchant}}
Date: {{receipt_date}}
Total: {{receipt_total}}

BANK TRANSACTION:
Description: {{transaction_description}}
Normalized merchant: {{transaction_merchant_normalized}}
Date: {{transaction_date}}
Amount: {{transaction_amount}}

Return format:
{"is_match": true/false, "confidence": 0.0-1.0, "reason": "brief explanation", "amount_explanation": "explanation for amount difference or null"}

MATCHING RULES:
1. Transaction amount is often HIGHER than receipt due to tips added later at restaurants
2. Dates can differ by 1-3 days (processing delay is normal)
3. Bank descriptions are often abbreviated/mangled versions of merchant names
4. Small amount differences ($0.01-$1) can be rounding or currency conversion
5. Tax differences sometimes appear if receipt shows pre-tax and bank shows post-tax

EXAMPLES:

Receipt: Olive Garden, $45.50, 2024-01-15
Transaction: OLIVE GARDEN 1234, $54.60, 2024-01-15
Output: {"is_match": true, "confidence": 0.95, "reason": "Same restaurant, date matches, amount difference is typical 20% tip", "amount_explanation": "Likely $9.10 tip added (20% tip)"}

Receipt: Target, $127.43, 2024-01-10
Transaction: TARGET 00012345, $127.43, 2024-01-11
Output: {"is_match": true, "confidence": 0.99, "reason": "Same merchant, exact amount, 1 day processing delay", "amount_explanation": null}

Receipt: Starbucks, $6.50, 2024-01-15
Transaction: MCDONALD'S 5678, $8.99, 2024-01-15
Output: {"is_match": false, "confidence": 0.98, "reason": "Different merchants (Starbucks vs McDonald's)", "amount_explanation": null}

Receipt: Amazon, $89.99, 2024-01-08
Transaction: AMZN MKTP US, $89.99, 2024-01-10
Output: {"is_match": true, "confidence": 0.99, "reason": "Amazon Marketplace matches Amazon, exact amount, 2 day delay normal for online orders", "amount_explanation": null}
