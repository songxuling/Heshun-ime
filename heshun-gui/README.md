# heshun-gui

`heshun` 输入法引擎的跨平台 GUI 演示程序。它不是系统输入法外壳，而是用于验证郑码、全拼和自然码双拼引擎行为的独立桌面程序。

## 使用

开发目录中双击：

```text
启动 heshun-gui.bat
```

若要生成可移动的发布目录，先用 MSVC Rust 工具链构建 release，再双击：

```text
打包 heshun-gui.bat
```

产物在 `dist/`。复制整个 `dist/` 目录到其他位置后，双击其中的 `启动 heshun-gui.bat` 即可运行。

## 目录布局

```text
heshun-gui/
├─ heshun-gui.exe                 # 仅发布包中存在
├─ 启动 heshun-gui.bat
├─ schemas/
│  ├─ *.schema.yaml
│  ├─ zhengma.bin
│  ├─ pinyin_simp.bin
│  └─ pinyin_zrm.bin
└─ data/
   ├─ zhengma66.userdb.json
   ├─ pinyin_full.userdb.json
   └─ double_pinyin_zrm.userdb.json
```

`data/` 下的用户词典在首次选词并正常关闭 GUI 后自动创建。三份方案的学习记录互不混用。

## 键盘操作

| 按键 | 行为 |
|---|---|
| 字母 | 输入编码 |
| `1`–`9` | 选择当前候选页的第 1–9 项 |
| 空格 | 选择当前候选页首选 |
| Backspace | 有预编辑时删除一码；无预编辑时删除编辑区光标前一个字符 |
| Escape | 取消当前预编辑和候选，不删除已上屏文本 |
| PageUp / PageDown | 候选翻页 |
| 鼠标点击编辑区 | 移动插入点；后续上屏文本插入该位置 |
| 鼠标点击候选 | 选择该候选 |

## 郑码反查

郑码方案下，输入反引号加拼音，例如：

```text
`zhong
```

候选会附带对应的郑码编码，例如 `中 [j/jivv]`。

## 资源错误

程序会从 EXE 所在目录或其父目录查找 `schemas/`。若 schema 或码表缺失，窗口顶部会显示错误信息，而不会直接 panic。
