# heshun-tsf 宿主验收矩阵

## 自动验证

| 项目 | 命令/范围 | 结果 |
|---|---|---|
| Rust runtime | `cargo build -p heshun --release --target x86_64-pc-windows-msvc` | 已通过 |
| x64 CMake/Ninja | `cmake -S . -B build-tsf -G Ninja -DCMAKE_BUILD_TYPE=Release`；`cmake --build build-tsf` | 已通过 |
| 导出契约 | CTest `tsf_binary_exports` | 已通过 |
| ABI 契约 | CTest `tsf_abi_header_contract` | 已通过 |
| 键盘转换契约 | CTest `tsf_key_event_contract` | 已通过 |
| TSF 接口契约 | CTest `tsf_tsf_interface_contract` | 已通过 |

## COM/TSF 边界

| 检查 | 命令 | 结果 |
|---|---|---|
| COM 激活 | `build-tsf\\bin\\heshun_tsf_activation_probe.exe` | 阻塞：`0x80040154`，DLL 尚未注册 |
| 注册/注销 | `scripts\\register.bat build-tsf\\bin\\heshun_tsf.dll` / `scripts\\unregister.bat ...` | 待管理员权限执行 |

## 真实宿主手工矩阵

在管理员注册成功后执行，每项记录组合输入、候选窗口、预编辑属性、光标移动、焦点切换、外部关闭和快捷键行为：

| 宿主 | x64/x86 | 状态 | 重点 |
|---|---|---|---|
| Notepad | x64 | 通过 | 已验证组合、候选、提交、Esc、焦点切换 |
| Word | x64 | 待手工 | 预编辑属性、滚动、窗口移动、分页 |
| WPF 文本框 | x64 | 待手工 | `GetTextExt`、CUAS、候选定位 |
| UWP 文本框 | x64 | 待手工 | UIElement、候选归属和焦点 |
| Chromium/浏览器输入框 | x64 | 待手工 | 快捷键保留、组合取消、滚动 |
| 多显示器/不同 DPI | x64 | 待手工 | 每显示器 DPI、边界避让、最小化恢复 |

## 执行步骤

1. 在 x64 VS Developer Command Prompt 中以管理员身份运行注册脚本。
2. 启动目标宿主并切换到 heshun 输入法配置文件。
3. 按上述矩阵逐项记录通过/失败及日志 `heshun-tsf.log`。
4. 完成后运行注销脚本，并重新执行 COM 激活探针确认注册边界。
