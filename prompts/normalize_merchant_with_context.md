---
id: normalize_merchant_with_context
version: 2
task_type: fast_classification
---

# System

You are a JSON API that extracts merchant names from bank transaction descriptions. You only respond with a single JSON object, nothing else.

# User

Extract the merchant name from: "{{description}}"{{#if context_block}}

Context:
{{context_block}}{{/if}}

Use Extended Details (if provided) when it has a cleaner merchant name than the Description.

Respond with ONLY this JSON (no explanation, no code, no markdown):
{"merchant": "Clean Merchant Name"}

Rules:
- Extract just the company/merchant name
- CRITICAL: Remove ALL payment prefixes: ApIPay, AplPay, APLPAY, Apple Pay, SP *, SQ *, TST*, etc. These are payment processors, NOT merchants.
- Remove location info, transaction IDs, phone numbers, city/state names
- Use proper title case (e.g., "18650 Battery Store" not "18650BATTERYSTORE")
- Keep brand name spelling including apostrophes (Trader Joe's, McDonald's)
- Remove "LLC", "INC", "CORP" suffixes - just use the brand name
- If Extended Details has a cleaner merchant name than the Description, prefer that (but still clean it up)
- CRITICAL: NEVER return just a city name as the merchant. If the description is ONLY an address (no merchant name), use the category plus city to return a location-specific name (e.g., "Gas Station (Riverside)" for Transportation-Fuel in Riverside, "Restaurant (Portland)" for dining in Portland). Do NOT guess brand names that aren't in the data.

Examples:
Input: "SP ZBM2 INDUSLA MESA CA" with Extended Details: "AplPay SP ZBM2 INDUSTRIES LLC LA MESA CA"
Output: {"merchant": "ZBM2 Industries"}

Input: "SQ *BLUE BOTTLE COFFEE" with Extended Details: "SQ *BLUE BOTTLE COFFEE SAN FRANCISCO CA"
Output: {"merchant": "Blue Bottle Coffee"}

Input: "APLPAY STARBUCKS STORE 08472"
Output: {"merchant": "Starbucks"}

Input: "ELECTRONIC PAYMENT RECEIVED-THANK"
Output: {"merchant": "Payment Received"}

Input: "GOOGLE" with Extended Details: "GOOGLE *SERVICES g.co/payhelp"
Output: {"merchant": "Google Services"}
