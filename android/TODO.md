## [android]

- Add custom theme palettes beyond system/light/dark (e.g., Material colors like blue, green, purple, or themed palettes like 'Midnight', 'Sunset', 'Forest')
- The synchronizer seems to have a bug, when the server responds 403 it keeps hammering the endpoint in an infinite loop
- Artist discography static is not persisted and is re-fetched every time from the server
- Create a db for logout operations so that if a user logsout, the server call doesn't need to
  happen immediately, plus in case of immediate error it can be retried
- Create a "memory pressure" component that can detect how much memory can we use for caches and such (imagine caching stuff from the db, pre-loading audio, images)
- Add an in-memory immediate cache layer in ContentResolver, only after a "memory pressure" component
- The search screen is dangerously empty, maybe show latest searches? or what?
- Add a contextual menu to queue albums and tracks (add to current playlist instead of replacing)
- Add shuffle and repeat functionality to the player (both domain logic and UI controls)
- When clicking on "Home" the navigation doesn't reset to root. 

- ~Once "current-playback" playlist is in place, add a screen for it so that the user can see it, modify it directly, save it as user-playlist~

- ~Add a player screen that is open when the user clicks on the bottom player~
- ~Implement a "current-playback" playlist, like the web does. So, a user can start the playback of an album, ok, but then, it should be able to QUEUE albums and tracks.~
- ~Make the album's image clickable and shown in full screen~
- ~BUG, when first opening an album screen, if you click on any track different than the first one, it still starts the first one. AFter that, it works normally on any albums, i suspect the cause to be in the player, not the UI~
- ~In the album screen, somehow highlight a track if it's currently playing~
- ~Make the play button accent color~
- ~Not all statics screen views are counted into recently viewed element, only the searched on.~
- ~In the album screen, remove the pictures from the track, remove the id, and make it a row, in the first space there should be track name above and artists compound names component below, then on the right there should be the duration~
- ~Put picture in artist screen and make the screen scroll behave like album (collapsing the picture)~
- ~Improve artist screen appearance: load actual artist picture and make scroll behavior collapse the image as user scrolls down~
- ~Make the scroll behavior in album screen collapse the album image. There's too little room for seeing the tracks, once the user start scroll down the image should collapse into a bar.~
- ~Remove all database migrations and reset StaticsDb version to 1 (clean slate for development)~
- ~Show tracks list in the album screen~
- ~When loading a track in the player from album screen, the next button doesn't work~
- ~Show related artists in the artist screen~
- ~Make a component to load an album's and an artists picture with intelligent size selection and fallback~
- ~Show tracks list in album page~
- ~Show all albums in the artist screen~
- ~Show track/artists info in small player~
