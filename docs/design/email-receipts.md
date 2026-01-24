---
title: Email Receipts
description: Email forwarding for receipt capture
date: 2026-01-24
---

Forward receipt emails to Hone for automatic capture and matching.

## Architecture

```
Email (receipt@yourdomain.com)
        ↓
Cloudflare Email Routing
        ↓
Cloudflare Worker (hone-receipt-worker)
        ↓ POST /api/receipts/email
Hone API (with API key auth)
        ↓
Receipt stored → AI parsing → Auto-match
```

## Flow

1. User forwards receipt email to `receipts@yourdomain.com`
2. Cloudflare Email Routing triggers the worker
3. Worker extracts:
   - Attachments (PDF, images)
   - Embedded images from HTML body
   - Email metadata (subject, from, date)
4. Worker POSTs each attachment to Hone's new `/api/receipts/email` endpoint
5. Hone processes as normal receipt-first workflow

## API Changes

### New Endpoint: `POST /api/receipts/email`

Accepts multipart form data with email metadata hints.

**Request:**
```
Content-Type: multipart/form-data

file: <binary attachment>
email_subject: "Your Amazon order #123-456"
email_from: "ship-confirm@amazon.com"
email_date: "2026-01-23T10:30:00Z"
original_filename: "receipt.pdf"
```

**Response:** Same as `POST /api/receipts`

**Differences from regular upload:**
- Stores `source: "email"` for audit trail
- Uses email metadata as parsing hints
- Accepts PDF files (converts to images for AI parsing)

### Database Changes

Add `source` column to receipts table:
```sql
ALTER TABLE receipts ADD COLUMN source TEXT DEFAULT 'upload';
-- Values: 'upload', 'email'
```

Add `email_metadata` column for debugging:
```sql
ALTER TABLE receipts ADD COLUMN email_metadata TEXT;
-- JSON: {"subject": "...", "from": "...", "date": "..."}
```

## Cloudflare Worker

### Setup

1. Create worker: `wrangler init hone-receipt-worker`
2. Configure secrets:
   ```bash
   wrangler secret put HONE_API_KEY
   wrangler secret put HONE_API_URL  # e.g., https://hone.yourdomain.com
   ```
3. Set up Email Routing:
   - Cloudflare Dashboard → Email → Email Routing
   - Create route: `receipts@yourdomain.com` → Worker

### Worker Code

```typescript
// src/index.ts
import PostalMime from 'postal-mime';

interface Env {
  HONE_API_KEY: string;
  HONE_API_URL: string;
}

export default {
  async email(message: EmailMessage, env: Env): Promise<void> {
    const parser = new PostalMime();
    const email = await parser.parse(message.raw);

    const metadata = {
      subject: email.subject || '',
      from: email.from?.address || message.from,
      date: email.date || new Date().toISOString(),
    };

    // Process attachments
    for (const attachment of email.attachments || []) {
      // Skip non-receipt files
      if (!isReceiptFile(attachment)) continue;

      await uploadToHone(env, attachment, metadata);
    }

    // Also check for embedded images in HTML
    if (email.html) {
      const embeddedImages = extractEmbeddedImages(email);
      for (const image of embeddedImages) {
        await uploadToHone(env, image, metadata);
      }
    }
  },
};

function isReceiptFile(attachment: Attachment): boolean {
  const validTypes = [
    'image/jpeg', 'image/png', 'image/webp', 'image/heic',
    'application/pdf'
  ];
  return validTypes.includes(attachment.mimeType);
}

async function uploadToHone(
  env: Env,
  attachment: Attachment,
  metadata: EmailMetadata
): Promise<void> {
  const formData = new FormData();
  formData.append('file', new Blob([attachment.content], { type: attachment.mimeType }));
  formData.append('email_subject', metadata.subject);
  formData.append('email_from', metadata.from);
  formData.append('email_date', metadata.date);
  formData.append('original_filename', attachment.filename || 'receipt');

  const response = await fetch(`${env.HONE_API_URL}/api/receipts/email`, {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${env.HONE_API_KEY}`,
    },
    body: formData,
  });

  if (!response.ok) {
    console.error(`Upload failed: ${response.status} ${await response.text()}`);
  }
}

function extractEmbeddedImages(email: ParsedEmail): Attachment[] {
  // Extract CID-referenced images from attachments
  // These are often the actual receipt images in HTML emails
  return (email.attachments || []).filter(a =>
    a.contentId && a.mimeType.startsWith('image/')
  );
}
```

### Dependencies

```json
{
  "dependencies": {
    "postal-mime": "^2.0.0"
  }
}
```

### wrangler.toml

```toml
name = "hone-receipt-worker"
main = "src/index.ts"
compatibility_date = "2024-01-01"

[vars]
# Non-secret config here

# Email routing binding
# No explicit binding needed - Cloudflare routes email to the worker
```

## PDF Handling

Many email receipts are PDFs. Options:

1. **Store PDF, parse later** - Save the PDF, mark for manual review
2. **Convert to image** - Use pdf.js or similar to render pages as images for AI
3. **Extract text** - Use pdf.js to extract text, skip vision model

Recommended: Option 1 for MVP, add conversion later if needed.

For PDFs:
- Store with `.pdf` extension
- Set status to `pending` (can still auto-match on email metadata)
- UI shows PDF icon, allows viewing
- AI parsing skipped (vision models need images)

## Security Considerations

### Sender Validation (Required)

Prevent unauthorized receipt submissions via email spoofing.

**Recommended: Allowlist + DKIM verification**

```typescript
// Worker config (stored as secrets)
const ALLOWED_SENDERS = ['you@gmail.com', 'partner@gmail.com'];

export default {
  async email(message: EmailMessage, env: Env): Promise<void> {
    // 1. Check sender allowlist
    const sender = message.from.toLowerCase();
    const allowedSenders = env.ALLOWED_SENDERS.split(',').map(s => s.trim().toLowerCase());
    if (!allowedSenders.includes(sender)) {
      console.log(`Rejected: sender ${sender} not in allowlist`);
      return;
    }

    // 2. Verify DKIM passed (prevents spoofing)
    const authResults = message.headers.get('authentication-results') || '';
    const dkimPass = authResults.includes('dkim=pass');
    const spfPass = authResults.includes('spf=pass');

    if (!dkimPass && !spfPass) {
      console.log(`Rejected: ${sender} failed authentication (DKIM: ${dkimPass}, SPF: ${spfPass})`);
      return;
    }

    // Proceed with processing...
  },
};
```

**Alternative: Secret email address**

For forwarding services that break DKIM (e.g., some email clients):
```
receipts+s3cr3t7ok3n@yourdomain.com
```

Worker extracts and validates the token:
```typescript
const match = message.to.match(/receipts\+([^@]+)@/);
const token = match?.[1];
if (token !== env.EMAIL_SECRET_TOKEN) {
  return; // reject
}
```

### Other Security Measures

1. **Rate limiting** - Track submissions per sender, reject if > 10/hour
2. **Size limits** - Reject attachments > 10MB (same as web upload)
3. **File type validation** - Only accept known receipt formats (images, PDF)
4. **API key rotation** - Dedicated key for email worker, easy to rotate
5. **Audit logging** - Log all accepted/rejected submissions for review

## Email Metadata as Hints

The email subject and sender often contain useful info:

- Amazon: "Your Amazon.com order #123-4567890"
- Apple: "Your receipt from Apple"
- Uber: "Your Tuesday morning trip with Uber"

The AI parsing prompt can use these hints:
```
Email subject: "Your Amazon.com order #123"
Email from: "auto-confirm@amazon.com"

[existing receipt parsing prompt]
```

This helps when:
- Receipt image is low quality
- Receipt is partial/cropped
- Multiple merchants in one image

## Implementation Plan

### Phase 1: API endpoint
1. Add `source` and `email_metadata` columns to receipts
2. Create `POST /api/receipts/email` endpoint
3. Accept multipart form with email hints
4. Store metadata, process as normal receipt

### Phase 2: Worker
1. Create Cloudflare Worker project
2. Implement email parsing with postal-mime
3. Deploy and configure Email Routing
4. Test with real emails

### Phase 3: PDF support (optional)
1. Add PDF rendering to images
2. Or: text extraction for simpler receipts

## Testing

### Local testing
```bash
# Simulate worker POST
curl -X POST http://localhost:3000/api/receipts/email \
  -H "Authorization: Bearer $HONE_API_KEY" \
  -F "file=@receipt.jpg" \
  -F "email_subject=Your Amazon order" \
  -F "email_from=ship@amazon.com" \
  -F "email_date=2026-01-23T10:00:00Z"
```

### Worker testing
```bash
# Use wrangler to test locally
wrangler dev

# Send test email (requires email routing configured)
# Or use Cloudflare's email testing tools
```

## File Changes Summary

| File | Change |
|------|--------|
| `crates/hone-core/src/models.rs` | Add source, email_metadata to Receipt/NewReceipt |
| `crates/hone-core/src/db/receipts.rs` | Update schema, add columns |
| `crates/hone-server/src/handlers/receipts.rs` | Add upload_email_receipt handler |
| `crates/hone-server/src/lib.rs` | Add route |
| `workers/hone-receipt-worker/` | New Cloudflare Worker project |
| `docs/deployment.md` | Add email setup instructions |
