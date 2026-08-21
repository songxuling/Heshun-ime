# Heshun IME

Heshun（和顺）是一个 Rust 编写的通用中文输入法项目。目前包含：

- `heshun/`：跨平台输入法解码核心，支持郑码 6.6、全拼、自然码双拼；提供 Rust API 和 C FFI。
- `heshun-gui/`：基于 egui 的跨平台 GUI 演示程序，用于验证编码、候选、反查和用户词典行为。

## 项目结构

```text
Heshun-ime/
├─ heshun/          # 核心引擎、码表构建工具、C FFI、测试与 schemas
├─ heshun-gui/      # GUI demo 和 Windows 启动/打包脚本
├─ LICENSE
└─ Cargo.toml       # Rust workspace
```

## 快速验证

```bash
cargo test --workspace
cargo check --workspace
```

## GUI

Windows 下可进入 `heshun-gui/` 双击：

```text
启动 heshun-gui.bat
```

打包可移动的 GUI 目录：

```text
打包 heshun-gui.bat
```

详见各子项目的 README：

- [核心引擎](heshun/README.md)
- [GUI demo](heshun-gui/README.md)
