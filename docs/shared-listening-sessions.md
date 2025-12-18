# Shared Listening Sessions

**Status**: Future feature (brainstorming placeholder)

## Concept

Allow multiple users to share a playback session, enabling "listening together" experiences where friends or family can:
- Listen to the same music simultaneously
- See what's playing in real-time
- Potentially control playback together

## Questions to Explore

- Who owns the session? (host model vs. collaborative)
- Can everyone control playback, or only the host?
- How do users join a shared session? (invite link, code, friend list)
- How is audio sync handled across different network latencies?
- Should chat/reactions be part of the feature?
- Privacy: who can see that you're in a shared session?
- What happens when the host leaves?
- Can users have different audio quality settings?
- How does this interact with server-side playback state per-user?

## Potential Use Cases

1. **Remote listening party**: Friends listen to an album together while chatting
2. **Family sharing**: Parent controls music playing throughout the house
3. **DJ mode**: One person curates music for a group
4. **Discover together**: Browse and play music collaboratively

## Technical Considerations

- Builds on top of server-side playback state feature
- Need session management (create, join, leave, end)
- Multi-user WebSocket rooms
- Latency synchronization challenges
- Permission model for who can control what

---

*This document is a placeholder for future brainstorming. No implementation planned yet.*
