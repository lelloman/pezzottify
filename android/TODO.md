## [android]

We can consider V1 ready once all of the task without V2 mark are completed.

### [ready for coding]

- We should somehow insert into the code A) the build variant B) the build version C) The git commit, then we can show it in the profile screen for now
- In the album screen, when the user clicks on the album, instead of openeing the album screen, the app crashes
- When the user performs a login, the username should be persisted so that if the user logs out, the username field of the login screen can be pre-filled

### [to refine]

- Add custom theme palettes beyond system/light/dark (e.g., Material colors like blue, green, purple, or themed palettes like 'Midnight', 'Sunset', 'Forest')
- The synchronizer seems to have a bug (skim through the git log though, maybe we fix it already), when the server responds 403 it keeps hammering the endpoint in an infinite loop
- Create a db for logout operations so that if a user logout, the server call doesn't need to happen immediately, plus in case of immediate error it can be retried
- Create a "memory pressure" component that can detect how much memory can we use for caches and such (imagine caching stuff from the db, pre-loading audio, images)
- Add an in-memory immediate cache layer in ContentResolver, only after a "memory pressure" component
- The search screen is dangerously empty, maybe show latest searches? or what?
- Add a contextual menu to queue albums and tracks (add to current playlist instead of replacing)
- We should probably think about collecting information about storage used and how it is used. For instance, it would be cool to cache audio files, but the feature needs to be storage-aware.
- We need to plan a "download" feature for albums and tracks
- Check that the behavior of the audio playback is system-friendly in terms of audio events (pause on focus lost, pause on headphones unplugged and such)
- We should gather statistics on what's being listened. Like what track, how long. This will be useful for discovery features.
- We should introduce user's liked content, this will populate the library's screen. (what do we show when the user doesn't have anything yet?)
- Make a plan to introduce the user's playlist feature
- I want E2E tests for the UI module. The reason the app is architected this way, is that we can test the UI only, without any other dependencies, making the ui a puppet basically.
- Once the listening stats are implemented, we can add a "jump back in" component in the home screen

### [done]

- ~Check the styling of the login screen, I now see a light text color in the email input even if the theme is light~
- ~BUG when the user logs out the music keeps playing!~
- ~Add base URL field to login screen for configuring server URL~
- ~Add a confirmation dialog to the logout operation~
- ~Add a small right padding to the bottom player track and artists column, when swiping, the 2 track info basically touch~
- ~Update the play/pause button to have just the triangle or the 2 bars in contrast color (white in dark theme, black in light theme)~
- ~Artist discography static is not persisted and is re-fetched every time from the server~
- ~Add shuffle and repeat functionality to the player (both domain logic and UI controls)~
- ~Make the player screen collapsable with a swipe down~
- ~Make the profile icon adapt to the themes, for instance I see it black even if the theme is dark~
- ~When clicking on "Home" the navigation doesn't reset to root.~
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
