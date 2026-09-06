//! heshun GUI 发布包所依赖的核心集成回归检查。
use heshun::engine::Engine;
use std::path::PathBuf;

fn schema_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schemas")
}

#[test]
fn all_gui_schemas_load() {
    let root = schema_root();
    for id in [
        "zhengma66",
        "pinyin_full",
        "double_pinyin_zrm",
        "double_pinyin_flypy",
        "double_pinyin_mspy",
        "double_pinyin_abc",
        "double_pinyin_pyjj",
        "double_pinyin_st",
    ] {
        let path = root.join(format!("{id}.schema.yaml"));
        Engine::from_schema_file(&path).unwrap_or_else(|e| panic!("{id}: {e}"));
    }
}

#[test]
fn zhengma_reverse_lookup_exposes_codes() {
    let path = schema_root().join("zhengma66.schema.yaml");
    let engine = Engine::from_schema_file(&path).unwrap();
    let mut session = engine.session();
    for ch in "`zhong".chars() { session.feed(ch); }
    let candidates = session.candidates(9);
    assert!(candidates.iter().any(|c| c.word == "中" && c.code.contains("jivv")));
}

#[test]
fn schema_userdict_paths_are_per_scheme() {
    let root = schema_root();
    let expected = [
        ("zhengma66", "zhengma66.userdb.json"),
        ("pinyin_full", "pinyin_full.userdb.json"),
        ("double_pinyin_zrm", "double_pinyin_zrm.userdb.json"),
        ("double_pinyin_flypy", "double_pinyin_flypy.userdb.json"),
        ("double_pinyin_mspy", "double_pinyin_mspy.userdb.json"),
        ("double_pinyin_abc", "double_pinyin_abc.userdb.json"),
        ("double_pinyin_pyjj", "double_pinyin_pyjj.userdb.json"),
        ("double_pinyin_st", "double_pinyin_st.userdb.json"),
    ];
    for (id, filename) in expected {
        let text = std::fs::read_to_string(root.join(format!("{id}.schema.yaml"))).unwrap();
        assert!(text.contains(filename), "{id} must have its own user dictionary");
    }
}

#[test]
fn double_pinyin_layouts_translate_china() {
    let root = schema_root();
    for (id, keys) in [
        ("double_pinyin_zrm", "vsgo"),
        ("double_pinyin_flypy", "vsgo"),
        ("double_pinyin_mspy", "vsgo"),
        ("double_pinyin_abc", "asgo"),
        ("double_pinyin_pyjj", "vygo"),
        ("double_pinyin_st", "aygo"),
    ] {
        let engine = Engine::from_schema_file(root.join(format!("{id}.schema.yaml")).as_path())
            .unwrap_or_else(|e| panic!("{id}: {e}"));
        let mut session = engine.session();
        for ch in keys.chars() {
            session.feed(ch);
        }
        assert!(
            session.candidates(9).iter().any(|candidate| candidate.word == "中国"),
            "{id} must translate {keys} to 中国"
        );
    }
}

#[test]
fn microsoft_double_pinyin_accepts_semicolon_key() {
    let path = schema_root().join("double_pinyin_mspy.schema.yaml");
    let engine = Engine::from_schema_file(&path).unwrap();
    let mut session = engine.session();
    assert_eq!(session.feed('a'), heshun::FeedResult::Waiting);
    assert_eq!(session.feed(';'), heshun::FeedResult::Waiting);
    assert_eq!(session.pending(), "a;");
}
