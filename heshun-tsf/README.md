# heshun-tsf

Windows TSF (Text Services Framework) shell for the `heshun` Rust input-method engine.

## Scope of the current minimal implementation

- Registers one COM `ITfTextInputProcessorEx` text service named **heshun**.
- Intercepts `a-z`, Backspace, Escape, Space, and `1-9` through `ITfKeyEventSink`.
- Loads `zhengma66.schema.yaml` or `pinyin_full.schema.yaml` through the Rust C ABI, selected internally by the input-method toggle.
- Commits returned text into the focused application through an asynchronous TSF edit session.
- Shows a native non-activating candidate window; Composition/preedit underline remains deferred.

## Current behavior

- System-wide Zhengma and full Pinyin input in TSF-aware Windows applications. Press `Ctrl+`` to switch between them inside the same Windows input method entry.
- Native non-activating candidate window showing the pending code and candidates; Space chooses the first candidate and `1-9` choose by number.
- Backspace edits the pending code while one exists, then returns to the host application to delete committed text once the pending code is empty.
- Escape clears pending code. `Delete` remains owned by the host application.
- Tap Shift alone to toggle Chinese and English modes. A Shift key used with another key does not toggle modes. `Ctrl+`` switches Zhengma and full Pinyin and clears an unfinished composition.

## Build

1. Build the Rust engine for MSVC first from workspace root:

   ```bat
   cargo build -p heshun --release --target x86_64-pc-windows-msvc
   ```

2. Open **x64 Native Tools Command Prompt for VS 2022** and run:

   ```bat
   cmake -S heshun-tsf -B build-tsf -G Ninja -DCMAKE_BUILD_TYPE=Release
   cmake --build build-tsf
   ```

The build copies `heshun.dll` and `heshun/schemas/` beside `heshun_tsf.dll`.

## Package

After a successful release build, create a standalone install directory:

```bat
heshun-tsf\scripts\package.bat build-tsf\bin
```

It produces `heshun-tsf\dist\` containing the DLLs, profile tool, schemas, documentation, and self-contained register/unregister scripts. From that directory, run `register.bat` with no parameter; it requests UAC when necessary.

## Register / unregister

Run either script from an x64 Native Tools Command Prompt for VS 2022. It requests UAC elevation automatically when required:

```bat
scripts\register.bat build-tsf\bin\heshun_tsf.dll
scripts\unregister.bat build-tsf\bin\heshun_tsf.dll
```

The TSF profile API writes system CTF configuration and requires elevation. The scripts register COM and one `heshun` language profile. Unregistering also removes the two old Zhengma/Pinyin profiles from earlier builds. Do not use `regsvr32` alone for installation: that only registers the COM server, not the keyboard profile.

## Test

```bat
ctest --test-dir build-tsf --output-on-failure
```
