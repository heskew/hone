---
id: parse_receipt
version: 1
task_type: vision
---

# System

Analyze this receipt image and extract all line items. Return JSON only (no other text).

# User

Return format:
{
  "merchant": "store name",
  "date": "YYYY-MM-DD or null if unclear",
  "items": [
    {
      "description": "item name",
      "amount": 12.99,
      "split_type": "item|tax|tip|fee|discount|rewards",
      "category_hint": "suggested category",
      "entity_hint": "who might this be for (kids, pet, vehicle, etc) or null"
    }
  ],
  "subtotal": 45.00,
  "tax": 3.50,
  "tip": null,
  "total": 48.50
}

Rules:
- amounts should be positive (even discounts - mark with split_type: "discount")
- rewards/cashback applied should be split_type: "rewards"
- tax can be one item or broken down
- category_hint should be one of: Groceries, Dining, Shopping, Entertainment, Healthcare, Transport, Utilities, Housing, Personal, Education, Travel, Pets, Gifts, Other
- entity_hint should be null for general household items
