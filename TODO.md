
## [web]
- In Album.vue, make 1 access to data.tracks[trackId] instead of one for each element
- Make right-click contextual menu for track, albums and artists (after user playlists) 
- Make left and right panel collapsable
- T Y P E S C R I P T?????
- Make titles and texts on single line and sliding if too long
- Use a logger instead console.logging all over the place
- Implement UNDO or history for playback playlists
- ~Make track content page~
- ~Style scrollbars~
- ~Make album content page~
- ~Add right panel to show current playlist~
- ~Bind space key to toggle play/pause~
- ~Persist data like playback status and volume and globalconfig~
- ~Add global config flag to disable images~
- ~Make artists names clickable~
- ~Log requests/responses to stdout~

## [catalog-server]
- Add user roles
- Add user saved albums, tracks and artists
- Add user playlists
- Create User identity/authentication db
- Create catalog db
- Wrap ids so that the type can be embedded in them
- Show requests in logs
- Add cache directive to responses

## [agents]
- Make artist info retrieval agent
- Make music score retrieval agent
