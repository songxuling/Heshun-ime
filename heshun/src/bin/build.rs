//! hs-build: 码表编译工具（形码 + 音码）
//!
//! 用法:
//!   hs-build <码表.txt> [输出.bin]           — 形码（郑码6.6.txt → ZMD1）
//!   hs-build --pinyin <dict.yaml> [输出.bin] — 音码（luna_pinyin.dict.yaml → ZPY1）
//!
//! 形码输入: 编码<TAB>字词（UTF-8/UTF-16 LE/BE 自动探测）
//! 音码输入: 字词<TAB>拼音[<TAB>词频]（Rime .dict.yaml 格式，含 frontmatter）

use std::io::Read;

fn read_text(path: &str) -> Result<String, String> {
    let mut raw = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| format!("打不开 {path}: {e}"))?
        .read_to_end(&mut raw)
        .map_err(|e| format!("读 {path} 失败: {e}"))?;
    if raw.len() >= 2 && raw[0] == 0xFF && raw[1] == 0xFE {
        return decode_u16(&raw[2..], u16::from_le_bytes, "utf-16-le");
    }
    if raw.len() >= 2 && raw[0] == 0xFE && raw[1] == 0xFF {
        return decode_u16(&raw[2..], u16::from_be_bytes, "utf-16-be");
    }
    match std::str::from_utf8(&raw) {
        Ok(s) => Ok(s.trim_start_matches('\u{feff}').to_string()),
        Err(_) => Err("不是 UTF-8/UTF-16（带BOM）".into()),
    }
}

fn decode_u16(
    raw: &[u8],
    from_bytes: fn([u8; 2]) -> u16,
    name: &str,
) -> Result<String, String> {
    let units: Vec<u16> = raw
        .chunks_exact(2)
        .map(|c| from_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16(&units).map_err(|e| format!("{name} 解码失败: {e}"))
}

/// 跳过 Rime .dict.yaml frontmatter（--- 到 ... 之间），返回数据行起始行号(0-based)。
fn skip_frontmatter(text: &str) -> usize {
    let mut lines = text.lines();
    let mut idx = 0;
    let mut in_fm = false;
    for line in &mut lines {
        if line.trim() == "---" {
            in_fm = true;
        }
        if in_fm && line.trim() == "..." {
            return idx + 1;
        }
        idx += 1;
    }
    0 // 无 frontmatter
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("用法:");
        eprintln!("  hs-build <码表.txt> [输出.bin]                      — 形码");
        eprintln!("  hs-build --pinyin <dict.yaml> [输出.bin]            — 音码（全拼）");
        eprintln!("  hs-build --pinyin --zrm <dict.yaml> [输出.bin]      — 音码（自然码双拼）");
        eprintln!("  hs-build --schema <schema.yaml>                     — 从 schema 构建");
        std::process::exit(1);
    }

    // --schema 模式
    if args[1] == "--schema" {
        if args.len() < 3 { eprintln!("缺 schema 文件"); std::process::exit(1) }
        build_from_schema(&args[2]);
        return;
    }

    // 解析 flags
    let mut pinyin_mode = false;
    let mut zrm_mode = false;
    let mut src: Option<String> = None;
    let mut dst: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--pinyin" => pinyin_mode = true,
            "--zrm" => zrm_mode = true,
            _ => {
                if src.is_none() {
                    src = Some(args[i].clone());
                } else if dst.is_none() {
                    dst = Some(args[i].clone());
                }
            }
        }
        i += 1;
    }

    let Some(src) = src else {
        eprintln!("缺码表文件");
        std::process::exit(1);
    };
    let dst = dst.unwrap_or_else(|| {
        if pinyin_mode {
            if zrm_mode { "pinyin_zrm.bin".to_string() } else { "pinyin.bin".to_string() }
        } else {
            "zhengma.bin".to_string()
        }
    });

    let text = match read_text(&src) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("错误: {e}");
            std::process::exit(1);
        }
    };

    if pinyin_mode {
        let algebra_rules = if zrm_mode {
            Some(algebra_strs_from_natural_code())
        } else {
            None
        };
        build_pinyin(&text, &src, &dst, algebra_rules)
    } else {
        build_table(&text, &src, &dst)
    }
}

fn build_table(text: &str, src: &str, dst: &str) {
    let mut entries: Vec<(u32, String)> = Vec::new();
    let mut skipped = 0usize;
    let mut pua = 0usize;
    for (ln, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut it = line.splitn(2, '\t');
        let code = it.next().unwrap().trim();
        let word = it.next().unwrap_or("").trim();
        if word.is_empty() {
            skipped += 1;
            continue;
        }
        if word.chars().any(|c| matches!(c, '\u{e000}'..='\u{f8ff}')) {
            pua += 1;
            continue;
        }
        match heshun::dict::encode_code(code) {
            Some(v) => entries.push((v, word.to_string())),
            None => {
                skipped += 1;
                if skipped <= 5 {
                    eprintln!("第{}行编码非法，跳过: {code:?}", ln + 1);
                }
            }
        }
    }

    let dict = heshun::dict::Dict::from_entries(entries);
    let mut buf = Vec::new();
    dict.save(&mut buf).expect("序列化失败");
    std::fs::write(dst, &buf).expect("写输出失败");

    println!(
        "✓ {} → {}\n  条目: {}（跳过 {}, PUA {}）\n  体积: {:.1} KB",
        src,
        dst,
        dict.len(),
        skipped,
        pua,
        buf.len() as f64 / 1024.0
    );
}

fn build_pinyin(text: &str, src: &str, dst: &str, algebra_rules: Option<Vec<String>>) {
    use heshun::algebra::Algebra;
    use heshun::pinyin::PinyinDict;
    use heshun::zrm::ZrmMap;

    let start = skip_frontmatter(text);
    let mut entries: Vec<(String, String, u32)> = Vec::new();
    let mut skipped = 0usize;

    for (ln, line) in text.lines().enumerate() {
        if ln < start {
            continue;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut it = line.splitn(3, '\t');
        let word = it.next().unwrap_or("").trim().to_string();
        let code = it.next().unwrap_or("").trim().to_string();
        let weight_str = it.next().unwrap_or("").trim();
        if word.is_empty() || code.is_empty() {
            skipped += 1;
            continue;
        }
        let weight = parse_weight(weight_str);
        entries.push((code, word, weight));
    }

    let mut dict = PinyinDict::from_entries(entries);

    // 双拼模式：有 algebra 规则则构建反向映射并内嵌
    if let Some(rules) = algebra_rules {
        let alg = Algebra::from_strings(&rules).unwrap_or_else(|e| {
            eprintln!("algebra 规则解析失败: {e}");
            std::process::exit(1)
        });
        let mut syllables: Vec<String> = Vec::new();
        // 从所有条目的拼音码中提取单音节（含空格拆开）
        for line in text.lines().skip(start) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut it = line.splitn(3, '\t');
            let _word = it.next();
            let code = it.next().unwrap_or("").trim();
            for part in code.split(|c| c == ' ' || c == '\'') {
                if !part.is_empty() {
                    syllables.push(part.to_string());
                }
            }
        }
        let zrm = ZrmMap::build(&syllables, &alg);
        dict = dict.with_zrm(zrm);
    }

    let mut buf = Vec::new();
    dict.save(&mut buf).expect("序列化失败");
    std::fs::write(dst, &buf).expect("写输出失败");

    let mode_label = if dict.zrm().is_some() { "（双拼）" } else { "（全拼）" };
    println!(
        "✓ {} → {} {}\n  条目: {}（跳过 {}）\n  体积: {:.1} KB",
        src,
        dst,
        mode_label,
        dict.entry_count(),
        skipped,
        buf.len() as f64 / 1024.0
    );
}

/// 解析词频列。支持三种格式：
/// - 空/缺省：罕见字，词频 0（排最后）
/// - 百分比："100%" → 10000, "64.53%" → 6453（luna_pinyin 格式）
/// - 纯数字："0"/"1"/"2"（pinyin_simp 格式）
fn parse_weight(s: &str) -> u32 {
    let s = s.trim();
    if s.is_empty() {
        return 0;
    }
    if let Some(stripped) = s.strip_suffix('%') {
        let f: f64 = stripped.parse().unwrap_or(0.0);
        return (f * 100.0).round() as u32;
    }
    s.parse().unwrap_or(0)
}

/// 自然码双拼规则字符串集合（供 --zrm 使用）
fn algebra_strs_from_natural_code() -> Vec<String> {
    [
        "erase/^xx$/", "derive/^([jqxy])u$/$1v/", "derive/^([aoe])([ioun])$/$1$1$2/",
        "xform/^([aoe])(ng)?$/$1$1$2/", "xform/iu$/Q/", "xform/[iu]a$/W/",
        "xform/[uv]an$/R/", "xform/[uv]e$/T/", "xform/ing$|uai$/Y/",
        "xform/^sh/U/", "xform/^ch/I/", "xform/^zh/V/", "xform/uo$/O/",
        "xform/[uv]n$/P/", "xform/i?ong$/S/", "xform/[iu]ang$/D/",
        "xform/(.)en$/$1F/", "xform/(.)eng$/$1G/", "xform/(.)ang$/$1H/",
        "xform/ian$/M/", "xform/(.)an$/$1J/", "xform/iao$/C/",
        "xform/(.)ao$/$1K/", "xform/(.)ai$/$1L/", "xform/(.)ei$/$1Z/",
        "xform/ie$/X/", "xform/ui$/V/", "xform/(.)ou$/$1B/", "xform/in$/N/",
        "xlit/QWRTYUIOPSDFGHMJCKLZXVBN/qwrtyuiopsdfghmjcklzxvbn/",
    ].iter().map(|s| s.to_string()).collect()
}

/// 从 schema.yaml 构建码表二进制。
fn build_from_schema(schema_path: &str) {
    use heshun::schema::SchemaConfig;
    use std::path::Path;

    let sc_path = Path::new(schema_path);
    let sc = SchemaConfig::load(sc_path).unwrap_or_else(|e| {
        eprintln!("加载 schema 失败: {e}");
        std::process::exit(1)
    });

    let base = sc_path.parent().unwrap_or(Path::new("."));
    let output = if sc.dictionary.file.is_empty() {
        format!("{}.bin", sc.schema.schema_id)
    } else {
        base.join(&sc.dictionary.file).to_string_lossy().into_owned()
    };

    let source = if sc.dictionary.source.is_empty() {
        eprintln!("schema 缺 dictionary.source（源文件）");
        std::process::exit(1)
    } else {
        base.join(&sc.dictionary.source).to_string_lossy().into_owned()
    };

    let text = match read_text(&source) {
        Ok(t) => t,
        Err(e) => { eprintln!("读源文件失败: {e}"); std::process::exit(1) }
    };

    if sc.is_table() {
        build_table(&text, &source, &output);
    } else if sc.is_script() {
        let algebra_rules = sc.speller.algebra.clone();
        build_pinyin(&text, &source, &output, algebra_rules);
    } else {
        eprintln!("未知引擎类型: {}", sc.engine.engine_type);
        std::process::exit(1)
    }
}