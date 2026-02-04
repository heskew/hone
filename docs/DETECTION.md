---
title: Detection Algorithms
description: How Hone identifies wasteful spending
date: 2026-01-24
---

# Detection Algorithms

Hone uses seven detection algorithms to identify wasteful spending.

## Subscription Pre-Filter

Before creating subscriptions, a two-layer filter reduces false positives:

1. Check `merchant_subscription_cache` for cached classifications
2. User exclusions (`source='user_override'`) take precedence over Ollama
3. Ollama classifies unknown merchants as SUBSCRIPTION or RETAIL
4. Retail merchants (grocery stores, gas stations) are skipped

## Zombie Detection

Identifies forgotten recurring charges.

1. Group transactions by (account_id, merchant) for account-specific subscriptions
2. Identify recurring patterns (consistent amount, regular interval)
3. Flag if: recurring 3+ months AND user hasn't acknowledged
4. Skip excluded subscriptions (user marked "not a subscription")

### Stale Acknowledgment Re-check

Acknowledged subscriptions are re-checked after 90 days (configurable via `acknowledgment_stale_days`):

1. Track `acknowledged_at` timestamp when user acknowledges subscription
2. If acknowledgment is older than threshold (default 90 days), treat as unacknowledged
3. Subscription will be flagged as zombie again, prompting user to re-confirm
4. Re-acknowledging updates the timestamp, resetting the 90-day window
5. Legacy subscriptions without timestamp are treated as freshly acknowledged

## Price Increase Detection

Tracks subscription price changes.

1. Track subscription amounts over time
2. Alert if: current > 3-months-ago by >5% or >$1
3. Skip excluded subscriptions

## Duplicate Detection

Finds multiple services in the same category.

1. Categorize subscriptions (streaming, music, cloud storage, etc.)
2. Alert if: 2+ active subscriptions in same category
3. Ollama explains overlap and unique features (when enabled)

## Auto-Cancellation Detection

Detects subscriptions that stopped charging.

1. Check acknowledged subscriptions for missed expected charges
2. Apply grace period:
   - 7 days for monthly
   - 3 days for weekly
   - 30 days for yearly
3. Auto-mark as cancelled if expected charge date + grace period has passed
4. Feeds into savings report

## Resume Detection

Catches reactivated subscriptions.

1. Check cancelled subscriptions for new matching transactions
2. If new charge found after cancellation, reactivate subscription
3. Create Resume alert to notify user
4. Mark as acknowledged to prevent immediate zombie flagging

## Spending Anomaly Detection

Flags unusual spending patterns.

1. Compare current month spending by category vs 3-month rolling average baseline
2. Alert if: increase >30% OR decrease >40% from baseline
3. Only trigger if baseline >= $50 (avoid noise from low-spend categories)
4. Ollama provides explanation with summary and reasons when available
5. Re-analysis: can re-run with different models via API/UI

## Tip Discrepancy Detection

Identifies transactions where the bank amount is significantly higher than the receipt total (potential tip or tax discrepancy).

1. Compare linked transactions' bank amount vs `expected_amount` (populated from receipt total)
2. Alert if: `abs(bank_amount) - abs(expected_amount) > tip_discrepancy_threshold`
3. Default threshold: $0.50 (configurable via `DetectionConfig`)

## Subscription Detection Thresholds

### Strict Pattern Matching (Default)
- Requires 3+ transactions
- 5% max amount variance
- 70% interval consistency
- Reduces false positives from regular shopping

### Smart Detection (Ollama-Enhanced)
When Ollama confirms merchant is a subscription service (>=70% confidence):
- 50% amount variance allowed (for variable charges like utilities)
- 50% interval consistency
- 2 minimum transactions
- Catches metered services (AWS, utility bills)

Configuration via `DetectionConfig`:
- `smart_amount_variance`
- `smart_interval_consistency`
- `smart_min_transactions`
- `ollama_confidence_threshold`
