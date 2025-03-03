
## [web]
- Implement a Toast/Snackbar like messages visualization component
- Make right-click contextual menu for albums and artists (after user playlists) 
- Make left and right panel collapsable
- Make titles and texts on single line and sliding if too long
- Use a logger instead console.logging all over the place
- Implement User page (profile?)
- Add a logo with a home link
- Add admin panel (after user roles)
- Make all list items lazily loaded (RecyclerView/LazyColumn behavior)
- Implement track selection (instead of play command), so that group of tracks can be added/removed
- ~Add current playing context (as in Album, Playlist, User's mix)~
- ~Implement UNDO or history for playback playlists~
- ~Make right-click contextual menu for track~
- ~In Album.vue, make 1 access to data.tracks[trackId] instead of one for each element~
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
- Create catalog db
- Check UserStore return type, we should not swallow errors
- Verify that all sqlite operations are performed within a transaction
- Delete cli_search and clean up unused stuff and format and warnings
- Add more unit tests (expecially to db, like migrations?)
- Add hard limit for playlist size (150?)
- ~Add user playlists~
- ~Make no_checks a cli args rather than build feature~
- ~Add cache directive to responses~
- ~Wrap ids so that the type can be embedded in them~
- ~Add user saved albums, tracks and artists~
- ~Show requests in logs~
- ~Create User identity/authentication db~

## [agents]
- Make artist info retrieval agent
- Make music score retrieval agent
