//! hs-bench: 引擎性能基准（形码/音码通用）
//!
//! 用法: hs-bench [码表.bin]
//! 自动识别 ZMD1(形码) / ZPY1(音码)

use std::time::Instant;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "zhengma.bin".into());
    let data = std::fs::read(&path).expect("读码表失败（先 hs-build）");
    if data.len() < 4 {
        eprintln!("文件太小");
        return;
    }
    let magic = u32::from_le_bytes(data[..4].try_into().unwrap());

    match magic {
        0x31444D5A => bench_table(&data, &path),
        0x3159505A => bench_pinyin(&data, &path),
        _ => eprintln!("未知码表格式 (magic={:08X})", magic),
    }
}

fn bench_table(data: &[u8], _path: &str) {
    let t0 = Instant::now();
    let dict = heshun::dict::Dict::load(data).expect("加载失败");
    println!(
        "加载 {} 条目: {:.3} ms（bin {:.1} KB）",
        dict.len(),
        t0.elapsed().as_secs_f64() * 1000.0,
        data.len() as f64 / 1024.0
    );

    let samples = ["j", "ji", "jiv", "jivv", "a", "aa", "zj", "wz", "zzzz", "bq"];
    let n = 200_000u32;
    let t0 = Instant::now();
    let mut hits = 0usize;
    for i in 0..n {
        let q = samples[(i % samples.len() as u32) as usize];
        hits += dict.prefix(q, 9).len();
    }
    let dt = t0.elapsed();
    println!(
        "前缀候选(limit 9) ×{}: 总 {:.1} ms，平均 {:.0} ns/次（命中 {} 条）",
        n,
        dt.as_secs_f64() * 1000.0,
        dt.as_nanos() as f64 / n as f64,
        hits
    );

    let t0 = Instant::now();
    for i in 0..n {
        let q = samples[(i % samples.len() as u32) as usize];
        hits += dict.exact(q).len();
    }
    let dt = t0.elapsed();
    println!(
        "精确匹配 ×{}: 总 {:.1} ms，平均 {:.0} ns/次（累计命中 {} 条）",
        n,
        dt.as_secs_f64() * 1000.0,
        dt.as_nanos() as f64 / n as f64,
        hits
    );
}

fn bench_pinyin(data: &[u8], _path: &str) {
    let t0 = Instant::now();
    let dict = heshun::pinyin::PinyinDict::load(data).expect("加载失败");
    println!(
        "加载 {} 条目: {:.3} ms（bin {:.1} KB）",
        dict.entry_count(),
        t0.elapsed().as_secs_f64() * 1000.0,
        data.len() as f64 / 1024.0
    );

    let samples = ["w", "wo", "zho", "zhong", "gu", "guo", "x"];
    let n = 50_000u32;
    let t0 = Instant::now();
    let mut hits = 0usize;
    for i in 0..n {
        let q = samples[(i % samples.len() as u32) as usize];
        hits += dict.exact(q).len();
        hits += dict.prefix(q).len();
    }
    let dt = t0.elapsed();
    println!(
        "exact+prefix ×{}: 总 {:.1} ms，平均 {:.0} ns/次（命中 {} 条）",
        n,
        dt.as_secs_f64() * 1000.0,
        dt.as_nanos() as f64 / (n as f64 * 2.0),
        hits
    );
}