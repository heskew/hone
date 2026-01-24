---
id: analyze_duplicates
version: 2
task_type: reasoning
---

# System

Analyze these subscription services and explain their overlap. Return JSON only.
{{#if feedback}}

## User Preferences

{{feedback}}
{{/if}}

# User

Category: {{category}}
Services: {{services}}

Return format:
{"overlap": "what these services have in common (1 sentence)", "unique_features": [{"service": "Service Name", "unique": "what makes it different (brief)"}]}

Rules:
- Be concise - one short sentence for overlap, one brief phrase per service
- Focus on content/features, not pricing
- Use the exact service names provided

Examples:

Services: Netflix, Disney+, HBO Max
Output: {"overlap": "All offer on-demand streaming of movies and TV series", "unique_features": [{"service": "Netflix", "unique": "International content, mature themes, extensive originals"}, {"service": "Disney+", "unique": "Family content, Marvel, Star Wars, Pixar exclusives"}, {"service": "HBO Max", "unique": "HBO originals, Warner Bros theatrical releases"}]}

Services: Spotify, Apple Music
Output: {"overlap": "Both offer unlimited music streaming with similar catalogs", "unique_features": [{"service": "Spotify", "unique": "Better discovery algorithms, podcasts, free tier"}, {"service": "Apple Music", "unique": "Seamless Apple ecosystem integration, spatial audio"}]}

Services: Dropbox, Google One, iCloud
Output: {"overlap": "All provide cloud storage and file sync across devices", "unique_features": [{"service": "Dropbox", "unique": "Best cross-platform sync, Paper collaboration"}, {"service": "Google One", "unique": "Integrates with Gmail/Photos, family sharing"}, {"service": "iCloud", "unique": "Seamless Apple device backup and sync"}]}
