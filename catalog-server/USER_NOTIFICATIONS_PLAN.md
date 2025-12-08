# User Notifications System Plan

## Implementation Status

| Feature | Status | Notes |
|---------|--------|-------|
| Overall | ⏳ Not Started | Initial planning only, detailed spec needed |

---

## Overview

A system for notifying users about events relevant to them, such as completed download requests, new content from followed artists, weekly suggestions, and system announcements.

## TODO: Spec this out (Detailed planning required before implementation)

### Topics to Cover

- [ ] Notification types and categories
- [ ] Delivery mechanisms (in-app, push, email?)
- [ ] Persistence and read/unread state
- [ ] Real-time delivery (WebSocket integration?)
- [ ] Notification preferences per user
- [ ] Batch notifications (daily digest vs immediate)
- [ ] API endpoints for fetching/marking notifications
- [ ] Retention policy (how long to keep old notifications)

### Known Notification Types

| Type | Trigger | Priority |
|------|---------|----------|
| Download completed | User's requested album finished downloading | High |
| Download failed | User's requested album failed after retries | High |
| New content | Followed artist released new album | Medium |
| Weekly suggestions | Personalized recommendations ready | Low |
| System announcement | Admin broadcast message | Varies |

### Potential Database Schema

```sql
CREATE TABLE user_notifications (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    notification_type TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT,
    data TEXT,                    -- JSON payload for deep linking
    read INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL,
    expires_at INTEGER,           -- Optional expiration
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX idx_notifications_user ON user_notifications(user_id, read, created_at);
```

### API Endpoints (Draft)

```
GET /v1/user/notifications          -- List notifications (paginated)
GET /v1/user/notifications/unread   -- Count of unread
POST /v1/user/notifications/:id/read -- Mark as read
POST /v1/user/notifications/read-all -- Mark all as read
DELETE /v1/user/notifications/:id   -- Dismiss notification
```

### Configuration (via TOML)

```toml
[notifications]
enabled = true
retention_days = 30
max_per_user = 100
```

### Dependencies

- TOML Configuration System (for settings)
- Background Jobs System (for batch notifications like weekly digest)
- WebSocket support (for real-time delivery - optional)

### Used By

- Download Manager (request completed/failed notifications)
- Expansion Agent (new content notifications - Phase 2)
- Weekly Suggestions feature (future)
- Admin announcements (future)

### Design Considerations

- Should notifications be pushed in real-time or polled?
- How to handle users with many unread notifications?
- Should we support notification grouping (e.g., "5 albums finished downloading")?
- Mobile push notifications (requires separate infrastructure)?
- Email notifications (requires email service integration)?
