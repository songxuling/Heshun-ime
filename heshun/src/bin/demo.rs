//! hs-demo: 交互式输入演示（支持形码/音码/双拼）
//!
//! 用法:
//!   hs-demo [字典.bin]                 — 魔数自动检测
//!   hs-demo --schema <schema.yaml>     — 从 schema 加载（推荐）
//!
//! 输入字母 = 按键，数字1-9 = 选词，空格 = 首选，- = 退格，. = 清空，q = 退出

use std::io::{self, BufRead, Write};
use heshun::engine::{Engine, FeedResult, SchemaKind};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("用法: hs-demo [字典.bin] | hs-demo --schema <schema.yaml>");
        std::process::exit(1);
    }

    let (engine, label) = if args[1] == "--schema" {
        if args.len() < 3 {
            eprintln!("缺 schema 文件");
            std::process::exit(1);
        }
        load_from_schema(&args[2])
    } else {
        load_from_bin(&args[1])
    };

    let mut s = engine.session();

    println!("{}。字母=输入 1-9=选词 空格=首选 -=退格 .=清空 q=退出", label);
    let stdin = io::stdin();
    loop {
        print!("\n[{}] > ", s.pending());
        io::stdout().flush().unwrap();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        for ch in line.trim_end_matches(['\n', '\r']).chars() {
            match ch {
                'q' => return,
                ' ' => {
                    if let Some(w) = s.select_first() {
                        println!("  ⇧ 上屏: {w}");
                    }
                }
                '-' => {
                    s.backspace();
                }
                '.' => s.clear(),
                '1'..='9' => {
                    if let Some(w) = s.select(ch as usize - '0' as usize) {
                        println!("  ⇧ 上屏: {w}");
                    } else {
                        println!("  ✗ 无该候选");
                    }
                }
                'a'..='z' => match s.feed(ch) {
                    FeedResult::Committed(w) => println!("  ⇧ 自动上屏: {w}"),
                    FeedResult::Waiting => {}
                    FeedResult::Rejected => println!("  ✗ 拒绝 '{ch}'"),
                },
                _ => println!("  （忽略 '{ch}'）"),
            }
        }
        if !s.pending().is_empty() {
            println!("  候选:");
            for (i, c) in s.candidates(9).iter().enumerate() {
                println!("    {}. {} ({})", i + 1, c.word, c.code);
            }
        }
    }
}

/// 从 schema.yaml 加载引擎。
fn load_from_schema(path: &str) -> (Engine, String) {
    let sc_path = std::path::Path::new(path);
    let eng = Engine::from_schema_file(sc_path).unwrap_or_else(|e| {
        eprintln!("schema 加载失败: {e}");
        std::process::exit(1)
    });
    // 用 SchemaConfig 获取方案名（比手抠 YAML 更可靠）
    let name = heshun::schema::SchemaConfig::load(sc_path)
        .map(|sc| sc.schema.name)
        .unwrap_or_else(|_| "?".to_string());
    (eng, format!("{} 方案 demo", name))
}

/// 从二进制字典加载引擎（魔数检测）。
fn load_from_bin(path: &str) -> (Engine, String) {
    let data = std::fs::read(path).unwrap_or_else(|e| {
        eprintln!("读 {path} 失败: {e}");
        std::process::exit(1)
    });
    if data.len() < 4 {
        eprintln!("文件太小，不是有效码表");
        std::process::exit(1)
    }
    let magic = u32::from_le_bytes(data[..4].try_into().unwrap());

    match magic {
        0x31444D5A => {
            let dict = heshun::dict::Dict::load(&data).unwrap_or_else(|e| {
                eprintln!("码表加载失败: {e}");
                std::process::exit(1)
            });
            let count = dict.len();
            let eng = Engine::new(SchemaKind::Table {
                dict,
                max_code_len: 4,
                auto_select: true,
                auto_select_pattern: Some("^[a-z]{4}$".into()),
            });
            (eng, format!("郑码引擎 demo（{} 条目）", count))
        }
        0x3159505A => {
            let dict = heshun::pinyin::PinyinDict::load(&data).unwrap_or_else(|e| {
                eprintln!("码表加载失败: {e}");
                std::process::exit(1)
            });
            let has_zrm = dict.zrm().is_some();
            let count = dict.entry_count();
            let eng = Engine::new(SchemaKind::Script { dict });
            let mode = if has_zrm { "自然码双拼" } else { "全拼" };
            (eng, format!("{}引擎 demo（{} 条目）", mode, count))
        }
        _ => {
            eprintln!("未知码表格式 (magic={:08X})", magic);
            std::process::exit(1)
        }
    }
}