# heshun — 通用中文输入法引擎核心（全平台共享）

通用输入法解码核心，支持**形码（郑码）+ 音码（全拼 / 自然码双拼）**。
Rust 编写，通过 C FFI 供各平台外壳链接：Windows TSF / macOS IMK / Linux fcitx5 / Android / iOS。

```
形码: 郑码6.6.txt       音码: *.dict.yaml (Rime 格式)
   │ hs-build              │ hs-build --pinyin
   ▼                       ▼
zhengma.bin (ZMD1)      pinyin.bin (ZPY1)
   │                       │
   └──────► Engine ───────┘
              │ zm_engine_load() 自动识别魔数
              ▼
           Session (每个输入框一个)
              │ feed() / select() / backspace()
              ▼
          候选 / 上屏（C FFI: zm_* 函数）
```

## 三种方案

| 方案 | 类型 | 字典 | 编译命令 | 核心机制 |
|---|---|---|---|---|
| 郑码6.6 | 形码 | `郑码6.6.txt`（编码\t字词） | `hs-build 郑码6.6.txt zhengma.bin` | base-27 二分前缀查表，满4码唯一自动上屏 |
| 全拼 | 音码 | `pinyin_simp.dict.yaml`（字词\t拼音\t词频） | `hs-build --pinyin ...` | 音节分段 + 词频 DP 组句 |
| 自然码双拼 | 音码 | 共享全拼字典 + algebra 键位映射 | `hs-build --pinyin ...` | Algebra 双拼键→全拼 → DP 组句 |

## 模块结构

| 模块 | 文件 | 职责 |
|---|---|---|
| 代数引擎 | `src/algebra.rs` | Rime 代数规则：xform/derive/abbrev/erase/xlit（双拼键位映射核心） |
| 形码字典 | `src/dict.rs` | ZMD1 二进制，base-27 编码，前缀=两次二分 |
| 音码字典 | `src/pinyin.rs` | ZPY1 二进制，多音节词，exact/prefix 二分 |
| DP 组句 | `src/composer.rs` | 连续拼音 → 动态规划分词 → 词频排序候选句 |
| 会话引擎 | `src/engine.rs` | 统一 Session：Table(形码) / Script(音码) 双模式 |
| C FFI | `src/ffi.rs` + `include/heshun.h` | zm_* 函数，跨平台外壳链接 |

## 构建

```bash
cargo build --release
cargo test          # 32 个单元测试
```

产物（`target/release/`）：
- `libheshun.a` — staticlib（推荐链接方式）
- `heshun.dll` — cdylib
- `hs-build` / `hs-demo` / `hs-bench` — 工具

## 工具

```bash
# 形码：郑码6.6.txt → zhengma.bin（自动探测 UTF-8/UTF-16，过滤 PUA）
hs-build 郑码6.6.txt zhengma.bin
#   ✓ 条目: 61716

# 音码：Rime .dict.yaml → pinyin.bin（自动跳过 frontmatter，解析百分比/整数词频）
hs-build --pinyin pinyin_simp.dict.yaml pinyin_simp.bin
#   ✓ 条目: 65125

# 交互演示（自动识别 ZMD1/ZPY1）
hs-demo zhengma.bin      # 郑码：字母=输入 1-9=选词 空格=首选
hs-demo pinyin_simp.bin  # 全拼：同上

# 性能基准
hs-bench zhengma.bin
```

## 二进制格式

### ZMD1（形码）
```
u32 magic = 0x31444D5A ("ZMD1")
u32 version = 1
u32 entry_count
u32 blob_len
entry_count × { u32 code, u32 word_off, u16 word_len }   // code 升序
blob_len × u8   // 字词 UTF-8 拼接
```
编码 = base-27 大端（a=1..z=26, 0=终止符）补齐 4 位；前缀查询 = 两次 `partition_point`。

### ZPY1（音码）
```
u32 magic = 0x3159505A ("ZPY1")
u32 version = 1
u32 entry_count
u32 blob_len
entry_count × { u32 code_off, u16 code_len, u32 word_off, u16 word_len, u32 weight }
blob_len × u8   // 拼音编码 + 字词 UTF-8 拼接
```
拼音编码去空格（`"zhong guo"` → `"zhongguo"`），按 code 升序，同码按词频降序。

## 词频格式兼容

`hs-build --pinyin` 的 `parse_weight` 兼容三种词频列：
- 空/缺省 → 词频 0（罕见字排最后）
- 百分比 `"100%"` → 10000, `"64.53%"` → 6453（luna_pinyin 格式）
- 纯整数 `"0"/"1"/"106588"`（pinyin_simp 格式）

## C FFI（`include/heshun.h`）

```c
zm_handle* eng  = zm_engine_load("zhengma.bin");  // 自动识别 ZMD1/ZPY1
zm_handle* sess = zm_session_new(eng);

char* committed = NULL;
int r = zm_feed(sess, 'z', &committed);  // 0拒绝 1等待 2自动上屏
zm_str_free(committed);

char* cands = zm_candidates(sess, 9);    // "词\x01码\x02词\x01码…"
char* word  = zm_select_first(sess);     // 空格首选
zm_str_free(cands); zm_str_free(word);

zm_session_free(sess);
zm_engine_free(eng);
```

## 已知限制（Phase 3 待实现）

- **词频模型简化**：纯词频加和（无语言模型），组句时多字词优先级已通过「精确匹配优先」缓解，
  但单字 vs 多字词的排序仍不及 Rime 的八股文语言模型。
- **反查 / 标点 / 键绑定 / 中英切换 / 用户词典**：尚未实现（FFI 已预留扩展点）。

## 各平台外壳路线（待实现）

| 平台 | 方案 | 说明 |
|---|---|---|
| Windows | TSF (C++/COM) | 最难；候选窗 + `ITfTextInputProcessorEx` |
| macOS | IMK (ObjC/Swift) | `IMKInputController` 子类 |
| Linux | fcitx5 插件 或 独立进程 | 也可转 fcitx5 table 格式零代码 |
| Android | InputMethodService + JNI | Kotlin，键盘 UI 自绘 |
| iOS | Keyboard Extension | 沙盒限制，内存 ~60MB |
