# heshun-tsf

Windows TSF (Text Services Framework) shell for the `heshun` Rust input-method engine.

## Scope of the current minimal implementation

- Registers a COM `ITfTextInputProcessorEx` text service named **heshun 郑码**.
- Intercepts `a-z`, Backspace, Escape, Space, and `1-9` through `ITfKeyEventSink`.
- Loads `schemas/zhengma66.schema.yaml` through the Rust C ABI.
- Commits returned text into the focused application using a synchronous TSF edit session.
- Candidate UI and composition/preedit are intentionally deferred. In this stage use Space or `1-9` to select candidates.

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

## Register / unregister

Run either script from an x64 Native Tools Command Prompt for VS 2022. It requests UAC elevation automatically when required:

```bat
scripts\register.bat build-tsf\bin\heshun_tsf.dll
scripts\unregister.bat build-tsf\bin\heshun_tsf.dll
```

The TSF profile API writes system CTF configuration and requires elevation. The scripts register COM, then register or remove the TSF language profile. Do not use `regsvr32` alone for installation: that only registers the COM server, not the keyboard profile.

## Test

```bat
ctest --test-dir build-tsf --output-on-failure
```
