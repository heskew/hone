---
id: classify_subscription
version: 1
task_type: fast_classification
---

# System

Is this merchant a subscription service with recurring billing, or regular retail? Return JSON only.

# User

Merchant: "{{merchant}}"

Return format:
{"is_subscription": true/false, "confidence": 0.0-1.0, "reason": "brief explanation"}

SUBSCRIPTION services (recurring monthly/yearly billing):
- Streaming: Netflix, Hulu, Disney+, HBO Max, Spotify, Apple Music
- Software: Adobe, Microsoft 365, Dropbox, Google Workspace
- Fitness: Gyms, Peloton, fitness apps
- Meal kits: HelloFresh, Blue Apron
- Subscriptions boxes: Birchbox, Dollar Shave Club
- Memberships: Amazon Prime, Costco membership, Audible
- Cloud services: AWS, iCloud, Google One
- News/Media: NYTimes, WSJ, Patreon

RETAIL (regular purchases, not subscriptions):
- Grocery stores: Trader Joe's, Safeway, Whole Foods, Costco (store purchases)
- Restaurants: McDonald's, Starbucks, Chipotle, local restaurants
- Gas stations: Shell, Chevron, BP
- General retail: Target, Walmart, Amazon (individual purchases)
- Pharmacies: CVS, Walgreens
- Department stores: Macy's, Nordstrom
- Hardware: Home Depot, Lowe's
- Online shopping: individual Amazon purchases, eBay

Examples:
Input: "NETFLIX"
Output: {"is_subscription": true, "confidence": 0.99, "reason": "streaming service"}

Input: "TRADER JOE'S"
Output: {"is_subscription": false, "confidence": 0.99, "reason": "grocery store"}

Input: "SPOTIFY"
Output: {"is_subscription": true, "confidence": 0.99, "reason": "music streaming"}

Input: "HELLO FRESH"
Output: {"is_subscription": true, "confidence": 0.95, "reason": "meal kit service"}

Input: "STARBUCKS"
Output: {"is_subscription": false, "confidence": 0.95, "reason": "coffee shop"}

Input: "PLANET FITNESS"
Output: {"is_subscription": true, "confidence": 0.95, "reason": "gym membership"}
