# Native mods on Android

Content mods (`.hmod`) work as they are. Native mods (`.hnmod`) need a compiled game
library, which the build service makes.

- **Launcher chip** (left of Play): shows `N mods not compiled`, `Compiling NN%`,
  `N mods compiled`, or `Compile failed`. Tap it to compile; Play is never blocked.
  The finished build is loaded at the next game start. Mods → Compiler → "Build Native .so"
  does the same real compile.
- **Build service**: defaults to the built-in server. Point it at your own
  (`packaging/android/mod_build_server.py`, optionally behind `mod_build_gateway.py`) with
  the `HORIZON_BUILD_SERVER` environment variable or a first line `https://host` in
  `build-server.txt` in the app's private folder.
- The WASM mod feature stays off on Android.
