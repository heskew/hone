---
id: explain_spending
version: 2
task_type: reasoning
---

# System

Analyze this spending change and explain likely reasons. Return JSON only.
{{#if feedback}}

## User Preferences

{{feedback}}
{{/if}}

# User

Category: {{category}}
Previous period: ${{baseline_amount}}/month avg ({{baseline_tx_count}} transactions/month avg)
Current period: ${{current_amount}} ({{current_tx_count}} transactions)
Change: {{change_direction}} by {{percent_change}}%
Top merchants this period: {{merchants_list}}
New merchants this period: {{new_merchants_list}}

Return format:
{"summary": "one sentence summary", "reasons": ["reason 1", "reason 2", "reason 3"]}

Rules:
- Be concise and specific
- Focus on observable patterns (frequency, new merchants, amount changes)
- Connect the dots between merchant patterns and spending changes
- Don't speculate about personal reasons
- Maximum 3 reasons
- If spending increased and there are new merchants, mention them
- If spending increased with more transactions, note the frequency change

Example:
Category: Dining
Previous period: $200/month (10 transactions/month)
Current period: $340 (22 transactions)
Change: increased by 70%
Top merchants: DoorDash: $180 (12 transactions), Uber Eats: $90 (6 transactions), Starbucks: $30 (4 transactions)
New merchants: Uber Eats

Output: {"summary": "Dining spending shifted significantly toward food delivery services", "reasons": ["Started using Uber Eats this month adding $90", "DoorDash orders more than doubled in frequency", "Combined delivery spending now accounts for 80% of dining budget"]}
