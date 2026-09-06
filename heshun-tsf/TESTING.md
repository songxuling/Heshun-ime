# Windows TSF shell: verified implementation notes

## Target

`heshun-tsf/` is the native Windows TSF shell for the Rust `heshun` engine. The current vertical slice includes composition/preedit and candidate plumbing in addition to direct commits:

```text
ITfKeyEventSink → heshun C ABI → ITfEditSession → ITfInsertAtSelection
```

Composition updates use a separate edit session after `StartComposition`; candidate display is an independent non-activating window path. Real-host rendering and the complete lifecycle matrix remain to be verified.

## Double-pinyin TSF path

The TSF shell selects one of the six `double_pinyin_*.schema.yaml` files through the language-bar menu. Runtime `pending` remains the raw key buffer, while runtime `preedit` and `preedit_cursor` are written to the host composition. Layout keys that overlap punctuation (Microsoft and Shitong `VK_OEM_1`) are routed through `ToUnicodeEx` and the Rust runtime before the pending-symbol punctuation fallback.

The current automated checks cover schema/resource presence in the copied runtime bundle, C ABI layout, key-event conversion, and TSF interface contracts. They do not replace manual Notepad/Word verification of actual double-pinyin key input and host rendering.

## Required interfaces

`HeshunTextService` implements:

- `ITfTextInputProcessorEx`: `Activate`, `ActivateEx`, `Deactivate`
- `ITfKeyEventSink`: key query/consume callbacks

During `ActivateEx`, it obtains `ITfKeystrokeMgr` from the thread manager and calls:

```cpp
AdviseKeyEventSink(client_id, sink, TRUE)
```

During `Deactivate`, it unadvises the sink before releasing the manager and engine/session objects.

## Commit rule

`ITfInsertAtSelection::InsertTextAtSelection` needs `TfEditCookie`; that cookie is only valid inside `ITfEditSession::DoEditSession`. Key callbacks must request a sync read/write edit session and perform UTF-8-to-UTF-16 conversion before the edit session uses normal insertion (`flags = 0`).

## Build requirements

Use matching x64 MSVC builds for Rust and C++:

```bat
cargo build -p heshun --release --target x86_64-pc-windows-msvc
cmake -S heshun-tsf -B build-tsf -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build build-tsf
```

The Rust static library carries WinSock/filesystem dependencies, so the TSF CMake target links `ws2_32`, `ntdll`, and `userenv` alongside `ole32` and `advapi32`.

On this Windows SDK, `msctf.h` exists but `msctf.lib` does not; do not link `msctf.lib`.

## Registration boundary

- `regsvr32` verifies COM registration of the DLL.
- A COM activation probe verifies `CoCreateInstance` can create `ITfTextInputProcessorEx`.
- Actual `ITfInputProcessorProfiles::Register` / `AddLanguageProfile` and keyboard-category registration write CTF configuration. Run `scripts\register.bat` from an elevated x64 VS Developer Command Prompt when changing the registered DLL.
- Run `scripts\unregister.bat` from the same elevation context to remove profile/category and COM registration.

## Verified locally

- `heshun_tsf.dll` builds with VS 2022 Build Tools / MSVC x64.
- Required COM exports pass CTest.
- `regsvr32` COM register/unregister roundtrip passes.
- Registered DLL passes `CoCreateInstance(CLSID_HeshunTextService, IID_ITfTextInputProcessorEx)`.
- Core Rust release build and the current TSF CTest baseline must be recorded separately from real-host verification; a successful COM probe does not prove composition or host input.
- `HOST_MATRIX.md` contains the reproducible automated checks and the Notepad/Word/WPF/UWP/browser/manual DPI acceptance matrix.
