---
title: Backup System
description: Backup and restore for Hone
date: 2026-01-24
---

Hone includes a robust backup system that creates encrypted, compressed database snapshots that are safe to run while the server is active.

## Overview

- **Safe while running**: Uses SQLCipher's `sqlcipher_export()` to create consistent copies
- **Encrypted**: Backups maintain the same encryption as the source database
- **Compressed**: Gzip compression reduces backup size
- **Pluggable**: `BackupDestination` trait allows multiple storage backends
- **Retention**: Automatic pruning keeps the most recent N backups

## CLI Commands

### Create a Backup

```bash
# Create backup with auto-generated timestamp name
hone backup create

# Create backup with custom name
hone backup create --name my-backup.db.gz

# Create backup in custom directory
hone backup create --dir /path/to/backups
```

### List Backups

```bash
# List all backups
hone backup list

# List backups in custom directory
hone backup list --dir /path/to/backups
```

### Restore from Backup

```bash
# Restore to default database location
hone backup restore hone-2024-01-15-143022.db.gz

# Restore with custom backup directory
hone backup restore hone-2024-01-15-143022.db.gz --dir /path/to/backups

# Force overwrite existing database
hone backup restore hone-2024-01-15-143022.db.gz --force
```

### Prune Old Backups

```bash
# Delete old backups, keeping 7 most recent (default)
hone backup prune

# Keep only 3 most recent
hone backup prune --keep 3

# Skip confirmation prompt
hone backup prune --keep 5 -y
```

## REST API

### Create Backup

```bash
POST /api/backup
Content-Type: application/json

{"name": "optional-custom-name.db.gz"}
```

Response:
```json
{
  "name": "hone-2024-01-15-143022.db.gz",
  "path": "/home/user/.local/share/hone/backups/hone-2024-01-15-143022.db.gz",
  "size": 12345,
  "accounts": 5,
  "transactions": 1234,
  "subscriptions": 12,
  "encrypted": true,
  "compressed": true
}
```

### List Backups

```bash
GET /api/backup
```

Response:
```json
[
  {
    "name": "hone-2024-01-15-143022.db.gz",
    "path": "/home/user/.local/share/hone/backups/hone-2024-01-15-143022.db.gz",
    "size": 12345,
    "created_at": "2024-01-15T14:30:22Z",
    "encrypted": true,
    "compressed": true
  }
]
```

### Prune Backups

```bash
POST /api/backup/prune
Content-Type: application/json

{"keep": 7}
```

Response:
```json
{
  "deleted_count": 3,
  "deleted_names": ["hone-2024-01-10-120000.db.gz", "..."],
  "retained_count": 7,
  "bytes_freed": 45678
}
```

### Get Specific Backup

```bash
GET /api/backup/:name
```

Response:
```json
{
  "name": "hone-2024-01-15-143022.db.gz",
  "path": "/home/user/.local/share/hone/backups/hone-2024-01-15-143022.db.gz",
  "size": 12345,
  "created_at": "2024-01-15T14:30:22Z",
  "encrypted": true,
  "compressed": true
}
```

### Delete Backup

```bash
DELETE /api/backup/:name
```

Response:
```json
{
  "deleted": true,
  "name": "hone-2024-01-15-143022.db.gz"
}
```

### Restore from Backup

```bash
POST /api/backup/:name/restore
Content-Type: application/json

{"force": true}
```

The `force` parameter is required and must be `true` to confirm the restore operation. Restoring will overwrite the current database.

Response:
```json
{
  "restored": true,
  "backup_name": "hone-2024-01-15-143022.db.gz",
  "message": "Database restored successfully. Server restart recommended."
}
```

**Important**: After restoring, you should restart the server to ensure all connections use the restored database.

### Verify Backup

```bash
POST /api/backup/verify
Content-Type: application/json

{"name": "hone-2024-01-15-143022.db.gz"}
```

Verification restores the backup to a temporary location and runs test queries to confirm it's readable.

Response:
```json
{
  "valid": true,
  "name": "hone-2024-01-15-143022.db.gz",
  "accounts": 5,
  "transactions": 1234,
  "message": "Backup verified successfully"
}
```

## Configuration

### Default Backup Location

Backups are stored in the platform-specific local data directory:

| Platform | Default Path |
|----------|-------------|
| Linux    | `~/.local/share/hone/backups/` |
| macOS    | `~/Library/Application Support/hone/backups/` |
| Windows  | `C:\Users\<User>\AppData\Local\hone\backups\` |

### Custom Backup Directory

Use `--dir` flag to override per-command:

```bash
hone backup create --dir /mnt/nas/hone-backups
hone backup list --dir /mnt/nas/hone-backups
```

### Encryption

Backups inherit the encryption key from the source database:
- If `HONE_DB_KEY` is set, backups are encrypted with the same derived key
- The same passphrase is required to restore

## Automated Backups

### Using Cron (Linux/macOS)

Create a daily backup at 2 AM, keeping 7 days:

```bash
# Edit crontab
crontab -e

# Add this line (replace <YOUR_PASSPHRASE> with your actual passphrase)
0 2 * * * cd /path/to/hone && HONE_DB_KEY="<YOUR_PASSPHRASE>" ./hone backup create && ./hone backup prune --keep 7 -y >> /var/log/hone-backup.log 2>&1
```

### Using Systemd Timer (Linux)

Create `/etc/systemd/system/hone-backup.service`:

```ini
[Unit]
Description=Hone Database Backup

[Service]
Type=oneshot
User=hone
# Set your passphrase in a secure credentials file or use EnvironmentFile=
Environment="HONE_DB_KEY=<YOUR_PASSPHRASE>"
WorkingDirectory=/opt/hone
ExecStart=/opt/hone/hone backup create
ExecStartPost=/opt/hone/hone backup prune --keep 7 -y
```

Create `/etc/systemd/system/hone-backup.timer`:

```ini
[Unit]
Description=Daily Hone Backup

[Timer]
OnCalendar=*-*-* 02:00:00
Persistent=true

[Install]
WantedBy=timers.target
```

Enable:

```bash
sudo systemctl enable --now hone-backup.timer
```

### Built-in Scheduler (Recommended)

The server includes a built-in backup scheduler that can be enabled via environment variables. This is the recommended approach as it handles backups and pruning automatically without external cron/systemd configuration.

**Environment Variables:**

| Variable | Description | Default |
|----------|-------------|---------|
| `HONE_BACKUP_SCHEDULE` | Backup interval in hours (e.g., `24` for daily, `168` for weekly) | Not set (disabled) |
| `HONE_BACKUP_RETENTION` | Number of backups to keep | `7` |
| `HONE_BACKUP_DIR` | Custom backup directory | Platform default |

**Example:**

```bash
# Daily backups, keep 7
export HONE_BACKUP_SCHEDULE=24
export HONE_BACKUP_RETENTION=7

# Start the server
hone serve
```

When configured, the scheduler:
1. Waits for the configured interval (doesn't backup immediately on startup)
2. Creates a timestamped backup
3. Prunes old backups according to retention policy
4. Logs all operations via tracing (visible in server logs)

**Docker with Built-in Scheduler:**

```yaml
# docker-compose.yml
services:
  hone:
    image: ghcr.io/heskew/hone-money:latest
    volumes:
      - hone-data:/data
      - hone-backups:/backups
    environment:
      - HONE_DB_KEY
      - HONE_BACKUP_SCHEDULE=24
      - HONE_BACKUP_RETENTION=7
      - HONE_BACKUP_DIR=/backups
```

### Docker Deployment (Manual)

When running in Docker, mount a backup volume:

```yaml
# docker-compose.yml
services:
  hone:
    image: ghcr.io/heskew/hone-money:latest
    volumes:
      - hone-data:/data
      - hone-backups:/backups
    environment:
      # Set via .env file or shell environment
      - HONE_DB_KEY

volumes:
  hone-data:
  hone-backups:
```

Run backup via docker exec:

```bash
docker exec hone /app/hone backup create --dir /backups
```

Or add to your host's crontab:

```bash
0 2 * * * docker exec hone /app/hone backup create --dir /backups && docker exec hone /app/hone backup prune --dir /backups --keep 7 -y
```

## NAS Backup (Recommended for Offsite)

For offsite backup, point `HONE_BACKUP_DIR` to a mounted NAS share. This is simpler than cloud storage and provides immediate protection.

**Why NAS works well:**
- Backup files are small (gzip compressed, typically 1-5 MB for years of transactions)
- Single file write - no chatty protocol overhead
- Sub-second transfer over home network
- Scheduler runs in background at off-hours

**Setup:**

1. Mount your NAS (e.g., via NFS, SMB, or built-in share)
2. Configure the backup directory:

```bash
# In your .env or docker-compose.yml
HONE_BACKUP_DIR=/mnt/nas/hone-backups
HONE_BACKUP_SCHEDULE=24
HONE_BACKUP_RETENTION=14  # Keep 2 weeks on NAS
```

**Docker with NAS:**

```yaml
# docker-compose.yml
services:
  hone:
    image: ghcr.io/heskew/hone-money:latest
    volumes:
      - hone-data:/data
      - /mnt/nas/hone-backups:/backups  # NAS mount
    environment:
      - HONE_DB_KEY
      - HONE_BACKUP_SCHEDULE=24
      - HONE_BACKUP_RETENTION=14
      - HONE_BACKUP_DIR=/backups
```

## Future: Cloud Backup (R2)

For cloud offsite backup, the system includes a stub for Cloudflare R2. When implemented:

```bash
HONE_R2_BUCKET=your-bucket-name
HONE_R2_ACCESS_KEY_ID=your-access-key
HONE_R2_SECRET_ACCESS_KEY=your-secret-key
HONE_R2_ENDPOINT=https://<account_id>.r2.cloudflarestorage.com
```

## Backup Format

Backups are SQLite database files:
- Created via `sqlcipher_export()` for consistency
- Encrypted with SQLCipher if source is encrypted
- Compressed with gzip (`.db.gz` extension)
- Named with timestamp: `hone-YYYY-MM-DD-HHMMSS.db.gz`

To inspect a backup manually (if unencrypted or you have the key):

```bash
# Decompress
gunzip -k hone-2024-01-15-143022.db.gz

# Open with sqlite3 (or sqlcipher for encrypted)
sqlite3 hone-2024-01-15-143022.db
sqlite> .tables
sqlite> SELECT COUNT(*) FROM transactions;
```

## Troubleshooting

### "Failed to access backup directory"

The backup directory doesn't exist or isn't writable. Either:
- Create it manually: `mkdir -p ~/.local/share/hone/backups`
- Use `--dir` to specify a different location

### "sqlcipher_export failed"

This can happen if:
- The database is corrupted
- Disk is full
- Permission issues with temp directory

Check disk space and try running with verbose logging:

```bash
hone --verbose backup create
```

### Backup is 0 bytes or very small

The database might be empty or the export failed silently. Verify your database has data:

```bash
hone dashboard
```

### Restore fails with encryption error

The backup was created with a different `HONE_DB_KEY`. You must use the same passphrase that was active when the backup was created.
