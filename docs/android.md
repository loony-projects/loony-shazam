# Android client

Native Android client for Loony Shazam, written in Kotlin with Jetpack
Compose. Lives in [`android/`](../android/). This document covers the
architecture, the recognition state machine, key implementation decisions,
and how to build/run it against a local backend.

## Tech stack

Kotlin, Jetpack Compose (Material 3), Kotlin Coroutines/Flow, Android
`AudioRecord` (raw mic capture), Retrofit + OkHttp, Jetpack ViewModel,
Navigation Compose, Room (local history), Coil (image loading). No DI
framework - a small hand-rolled `AppContainer` (see
`android/app/src/main/java/com/loonyshazam/app/LoonyShazamApp.kt`) wires up
dependencies for the process lifetime, which is proportionate for a
single-Activity app with no per-screen scoping needs.

## Architecture summary

```
MainActivity
  -> LoonyShazamNavHost (Navigation Compose: home / result / history / history_detail)
       -> HomeScreen        (ui/home)     -- mic button, permission handling
       -> ResultScreen       (ui/result)   -- recognized / not-found / share
       -> HistoryScreen       (ui/history)  -- Room-backed list, empty state
       -> HistoryDetailScreen (ui/history)  -- read-only detail for a past entry

RecognitionViewModel (viewmodel/)
  -> AudioSource (audio/AudioRecorder.kt)       -- mic capture, PCM16 Flow
  -> RecognitionRepository (data/repository/)    -- upload + parse + persist
       -> RecognitionApi (data/network/, Retrofit)
       -> HistoryDao (data/db/, Room)

HistoryViewModel (viewmodel/) -> RecognitionRepository -> HistoryDao
```

Two interfaces exist specifically to keep the ViewModels unit-testable
without a real microphone, real network, or Robolectric:

- `AudioSource` (implemented by `AudioRecorder`) - `RecognitionViewModel`
  depends on the interface; tests substitute `FakeAudioSource`.
- `RecognitionRepository` (implemented by `DefaultRecognitionRepository`) -
  both ViewModels depend on the interface; tests substitute
  `FakeRecognitionRepository`.

`MusicProvider` (`data/MusicProvider.kt`) is a deliberate extensibility
seam for a future "open in Spotify/Apple Music" deep link. The shipped
implementation (`NoOpMusicProvider`) always returns `null` - no real
provider credentials or API calls are wired up.

## State machine

`RecognitionViewModel` (`viewmodel/RecognitionViewModel.kt`) drives:

```
Idle -> Listening -> Processing -> Result
                                 -> NotFound
                                 -> Error
        Listening -> PermissionDenied   (SecurityException from AudioSource)
        (any)     -> PermissionDenied   (RECORD_AUDIO denied by the user)
```

- **Idle**: initial state, and where `reset()` returns to after a terminal
  state.
- **Listening(elapsedSeconds)**: recording is in progress; the elapsed
  counter updates as PCM chunks arrive, capped at
  `AudioConfig.MAX_RECORDING_SECONDS` (12s).
- **Processing**: recording finished, WAV upload/recognition in flight.
- **Result(song, match, recognizedAt)**: backend returned
  `recognized: true`. The song is persisted to Room here, before the state
  transition, so the History screen is guaranteed to reflect the result the
  moment it's shown.
- **NotFound(reason)**: backend returned `recognized: false` (reason is
  currently always `"NO_MATCH"` per the API contract). Nothing is
  persisted.
- **Error(message)**: a network-level failure (after the bounded retry) or
  a well-formed 4xx/5xx from the backend. The server's `message` field is
  surfaced directly when available.
- **PermissionDenied**: `RECORD_AUDIO` isn't granted, or was revoked
  mid-recording.

Recording and network I/O both run inside `viewModelScope`, so both are
cancelled automatically when the ViewModel is cleared. `HomeScreen` also
calls `cancelListening()` from a `Lifecycle.Event.ON_STOP` observer, so
backgrounding the app mid-recording releases the `AudioRecord` immediately
rather than continuing to record (or leaking it) in the background.

## minSdk choice: 26

`minSdk = 26` (Android 8.0 / Oreo). `AudioRecord`'s buffer-size and format
APIs used here have been stable since well before API 26, and 26 covers the
large majority of active devices without carrying compatibility shims for
very old audio HALs. `compileSdk`/`targetSdk = 35` because that's the
highest platform actually installed in the build environment used for this
project (`android-35`, `android-36.1` are present; 35 is the well-tested
stable target - see `android/app/build.gradle.kts` for the same rationale
inline).

## Audio capture

`audio/AudioRecorder.kt` wraps `android.media.AudioRecord` (not
`MediaRecorder` - the product needs raw PCM samples to encode into WAV
itself, not a compressed container) behind a cold coroutine `Flow<AudioChunk>`:

- Tries sample rates in order `[44100, 22050, 16000, 11025, 8000]` Hz
  (`AudioConfig.CANDIDATE_SAMPLE_RATES`), using the first one for which
  `AudioRecord.getMinBufferSize` succeeds - a standard fallback ladder for
  devices/emulators whose mic HAL doesn't support 44.1kHz capture.
- Mono, 16-bit PCM (`ENCODING_PCM_16BIT` / `CHANNEL_IN_MONO`).
- Capped at `AudioConfig.MAX_RECORDING_SECONDS` (12s).
- Runs on `Dispatchers.IO` via `flowOn`; the blocking `AudioRecord.read`
  calls never touch the UI thread.
- Cooperative cancellation: the `try/finally` around the recording loop
  guarantees `AudioRecord.stop()`/`release()` run even if the collecting
  coroutine is cancelled mid-recording (ViewModel cleared, or
  `cancelListening()` called explicitly) - no leaked native audio
  resources.

`audio/PcmAccumulator.kt` buffers incoming `ShortArray` chunks into one
growable primitive array (not `ArrayList<Short>`, which would box every
sample - roughly 4x the memory and GC churn for the ~0.5-1M samples a
12-second capture produces).

`audio/WavEncoder.kt` writes a minimal canonical 44-byte PCM WAV header by
hand (no external library needed - it's a small, fixed, well-documented
binary layout) around the accumulated PCM16 samples, entirely in memory.

## API integration

`data/network/RecognitionApi.kt` is a Retrofit interface for
`POST /api/v1/recognitions/audio` (multipart, single part named `audio`,
content-type `audio/wav`), matching the contract in
[architecture.md](architecture.md) and the Rust backend's
`recognition_handler.rs` exactly - including that `song.id` is a **string**
on the wire (a stringified `i64`), and that both success and no-match
responses come back as HTTP 200, distinguished by the `recognized` field.

`data/repository/RecognitionRepository.kt` (`DefaultRecognitionRepository`):

- Builds the multipart request and calls the API.
- Applies **at most one retry**, and only for network-level (`IOException`)
  failures - a well-formed HTTP error response is never retried, since
  retrying it wouldn't help.
- OkHttp is configured with explicit timeouts (10s connect / 20s read /
  20s write, see `AppContainer`) so a hung request can't block the UI
  indefinitely.
- On a non-2xx response, parses the `{"error", "message"}` body and
  surfaces `message` (falling back to `error`, then a generic
  "HTTP {code}" message) - never a raw stack trace.
- Maps the DTO layer (`data/network/dto/`) to small domain models
  (`data/model/Song.kt`) so the rest of the app never depends on wire
  shapes directly.

Base URL is injected via `buildConfigField` per build type
(`android/app/build.gradle.kts`), never hardcoded in Kotlin source:

- **debug**: `http://localhost:8080/`, reached via `adb reverse tcp:8080
  tcp:8080` (see "Pointing the debug build at a local backend" below) —
  this works uniformly for a real device over USB and an emulator alike,
  unlike the older `10.0.2.2` NAT alias, which only resolves inside a
  standard AVD and does nothing on physical hardware.
- **release**: `https://api.example.com/` - an explicit placeholder. This
  is **not** a real, deployed domain; a real release build must override it
  with the actual production API host before shipping.

## Room schema

Single table, `history` (`data/db/HistoryEntity.kt`):

| column | type | notes |
|---|---|---|
| `id` | `Long` (PK, autogenerate) | local row id |
| `songId` | `String` | backend's song id (stringified i64) |
| `title` | `String` | |
| `artist` | `String` | |
| `album` | `String?` | |
| `artworkUrl` | `String?` | |
| `recognizedAt` | `Long` | epoch millis |

The raw query audio is **never** written to the database or disk - only
this small metadata row is persisted, matching the backend's own "never
retain query audio" policy (see [architecture.md](architecture.md)). The
WAV bytes built for upload live only in memory for the duration of the
HTTP request.

## Build / run

Prerequisites: JDK 17, Android SDK with `android-35` platform and matching
build-tools installed (the project pins `compileSdk = 35`).

```bash
cd android
echo "sdk.dir=/path/to/Android/Sdk" > local.properties   # gitignored, create locally

./gradlew assembleDebug   # builds app/build/outputs/apk/debug/app-debug.apk
./gradlew test            # JVM unit tests (ViewModel, repository, WAV encoder)
```

### Pointing the debug build at a local backend

The debug build targets `http://localhost:8080/` — meaning the device's
*own* `localhost`, not this machine's. Reaching the actual backend needs
`adb reverse tcp:8080 tcp:8080`, which forwards the device's
`localhost:8080` to this machine's `localhost:8080` over the adb
connection (USB for a real device, the emulator's own adb link for an
AVD — the same command works for both, which is the whole point of using
it instead of the old `10.0.2.2` NAT alias that only ever worked for
emulators).

`scripts/dev/install_android.sh` (`make install-android`) does all of
this for you:
```bash
make dev            # or make dev-local — get the backend running first
make install-android
```
It builds the debug APK, confirms exactly one device/emulator is
attached (or takes `--device <id>` when more than one is), runs `adb
reverse`, warns (without failing) if the backend isn't actually
responding yet, installs with `-r` (safe to re-run — reinstalls over
itself), and launches the app. `--no-build` skips the Gradle build if you
already have a fresh APK; `--no-launch` installs without opening it.
`scripts/dev/uninstall_android.sh` (`make uninstall-android`) removes it.

> **Cleartext HTTP note**: since `targetSdk >= 28`, Android blocks plain
> HTTP traffic by default — without an exception, the debug build would
> compile and pass unit tests fine but fail to actually connect to the
> local backend at runtime (a class of bug unit tests alone can't catch,
> since JVM unit tests don't enforce Android's network security policy).
> `app/src/debug/res/xml/network_security_config.xml` +
> `app/src/debug/AndroidManifest.xml` grant a narrow cleartext exception
> for `localhost`/`127.0.0.1`/`10.0.2.2` in **debug builds only** — this
> debug-only source set is never merged into a release build, which keeps
> the platform default of requiring HTTPS everywhere.

**Caveat**: `adb reverse` rules don't persist across a device reboot or
USB reconnect — re-run `make install-android` (or just `adb reverse
tcp:8080 tcp:8080`) if recognition calls start failing after either.

To point at a different host entirely (e.g. a real deployed backend
instead of a local one), change the `debug` block's
`buildConfigField("String", "BASE_URL", ...)` in `app/build.gradle.kts`.

## Tests

Run with `./gradlew test` (JVM unit tests, no emulator/device required):

- `audio/WavEncoderTest.kt` - verifies the 44-byte header's RIFF/WAVE/fmt
  /data markers, sample rate/channel/bit-depth encoding, little-endian
  sample byte order, and the empty-input edge case.
- `audio/PcmAccumulatorTest.kt` - verifies growth past initial capacity and
  partial-length appends.
- `data/repository/RecognitionRepositoryTest.kt` - uses OkHttp
  `MockWebServer` (a real embedded HTTP server, not a mocked Retrofit
  interface) to verify: the multipart request's field name/content-type,
  successful/not-recognized JSON parsing against the exact example payloads
  in the API contract, a 4xx error surfacing the server's `message`, and
  that a network-level failure is retried exactly once.
- `viewmodel/RecognitionViewModelTest.kt` - drives the full state machine
  (`Idle -> Listening -> Processing -> Result | NotFound | Error |
  PermissionDenied`, plus `reset()`) through `FakeAudioSource` and
  `FakeRecognitionRepository`, using Turbine to assert the exact sequence
  of `StateFlow` emissions.

## Known limitations / reviewer notes

- No instrumented (`androidTest`) UI tests - no emulator/device was
  available in the build environment used for this project. All screens
  are exercised only via the compiler (Compose preview functions were not
  added) and the unit tests above cover the ViewModel/repository/audio
  logic underneath them.
- Coil (`AsyncImage`) is used for artwork with default caching/sizing - no
  custom cache size, placeholder crossfade, or request-level tuning has
  been applied.
- `MusicProvider` is a real interface with a wired-up call site in both
  `ResultScreen` and `HistoryDetailScreen`, but its only implementation
  (`NoOpMusicProvider`) always returns `null` - the "Open in music app"
  button is present but disabled until a real provider is implemented.
- The release `BASE_URL` (`https://api.example.com/`) is an intentional
  placeholder and must be replaced before any real release build.
- History has no delete/clear action yet - entries accumulate
  indefinitely (metadata only, no audio, so the storage footprint is
  small).
