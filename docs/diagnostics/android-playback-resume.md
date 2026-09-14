# Android idle playback resume

Tracking bug: [LLPR/PEZZOTTIFY-14](https://crumbles.lelloman.com/w/LLPR/PEZZOTTIFY/14).
Targeted diagnostic: `android/player/src/main/java/com/lelloman/pezzottify/android/player/PlaybackDiagnostics.kt`.
Every record includes `ticket=LLPR/PEZZOTTIFY-14`, under the `PlaybackDiagnostics` tag.

## Observed on 2026-09-14 (Europe/Rome)

OPPO CPH2493, app 0.5.1645 (1645). After car Bluetooth listening the previous
afternoon, reopening and pressing Play did not resume. ADB confirmed a connected
controller and active media session, Media3 STATE_IDLE (1), 21 queued items,
index 7, position 110123 ms. Pause at 10:35:19.041 and Play at 10:35:26.625 changed
playWhenReady but neither playback state nor position progressed.

PID 7263 had existed since Sep 12 21:29:40: whole-process recreation after the
car session is ruled out for this occurrence, but service/player recreation
within that process is not. Historical logs explaining entry into idle were lost.
An explicit stop, playback error, or lifecycle event remains possible. Neither
Bluetooth nor another app taking audio focus is an established cause.

The app also reported playing and accumulated listening time while frozen:
its exposed playing flag currently reflects playWhenReady, not actual playback.
This patch does not redefine that shared UI/domain contract or change listening
sync (400/429 responses were also observed; causality is unconfirmed).

## Fix and scope

Explicit Play through setIsPlaying or toggle (including reconnect paths) calls
prepare first when idle with a nonempty queue. It does not reload, clear, or seek.
Pause, empty queues, buffering, ready and ended retain their existing behavior.
See [Media3 Player](https://developer.android.com/reference/androidx/media3/common/Player#prepare()).
This repairs the observed inability to resume, not the unknown original trigger.

## Capture

Install a build containing this change first, with the device owner's approval.
In Settings → Logging enable **Save logs to file** before the next long pause;
ensure the log level includes Info. Afterwards use **Share logs** promptly.
Existing files rotate at roughly 5 MB total; busy logs can still evict evidence.
For a controlled reproduction, stream the narrow logcat filter to a host file:

```sh
adb -s DEVICE_SERIAL logcat -v threadtime 'PlaybackDiagnostics:I' '*:S' > playback-resume.log
```

Do not clear logcat before capturing the incident. Replace DEVICE_SERIAL with
the current connection (wireless endpoints change). Capture before restarting,
then note local time, build, visible Play/Pause state and whether position moves.
Shared full app logs may contain unrelated sensitive data; review/redact them.
Targeted records exclude media metadata, stream URLs and exception messages;
caller package names are retained for stop-command attribution.

## Interpretation / controlled follow-up

- `run` changes across process lifetimes; `instance` identifies service/controller
  owners; `player` identifies an object within the process. `pid` and `elapsedMs`
  complement the log's wall time. New service/player with the same run indicates
  recreation inside the existing process. Missing destroy logs alone do not prove a kill.
- `state_changed`, `play_when_ready_changed reason=N`, `suppression_changed`,
  `is_playing_changed` and `player_error code=N` come from the service's actual
  player even while the UI is absent. State: 1 idle, 2 buffering, 3 ready, 4 ended.
  Reason integers are Media3 1.8.0 Player constants (e.g. playWhenReady reason 2
  is audio-focus loss). `requested` and `playing` deliberately remain separate.
- `controller_command` records incoming stop/play-pause/prepare requests with
  caller and command integer; it is a request trace, not proof of completion.
  Media3 1.8.0 command values: play/pause 1, prepare 2, stop 3.
  `app_stop` / `app_clear_session` identify local app stop paths.
- `becoming_noisy_pause`, `task_removed`, `service_destroy`,
  `release_player_and_session`, `disconnected` and reconnect events distinguish
  explicit release from a connected-but-idle player. Abrupt OS kills cannot log
  their own teardown; correlate with system process history when available.
- `request_playback` followed by `prepare_idle_resume` should lead to buffering /
  ready and advancing position. A following error identifies a failed recovery.

With owner approval, separately test: pause → background → reopen; Bluetooth
disconnect; another media app taking focus; explicit session stop → Play; and
process/service recreation. Capture each separately so causes aren't mixed.
The stop test should preserve queue/index/position and resume; focus interruption
may pause/suppress without entering idle. Do not force-stop, clear data or disrupt
active listening without approval. None of these new-build device tests have yet
been performed by this implementation.

Unit regression command: `cd android && ./gradlew :player:testDebugUnitTest`.
