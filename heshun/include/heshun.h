/* heshun.h — 通用中文输入法引擎 C FFI
 *
 * 链接: libheshun.a (staticlib) 或 heshun.dll/.so (cdylib)
 * 所有返回 char* 的字符串必须用 hs_str_free 释放。
 * Engine/Session 句柄非线程安全：每个输入框一个 Session，单线程操作。
 */
#ifndef HESHUN_H
#define HESHUN_H

#ifdef __cplusplus
extern "C" {
#endif

typedef void hs_handle;

/* 生命周期 */
hs_handle* hs_engine_load(const char* bin_path);     /* 失败 NULL */
hs_handle* hs_engine_load_schema(const char* schema_path); /* 从 schema.yaml 加载; 失败 NULL */
void       hs_engine_free(hs_handle* eng);
hs_handle* hs_session_new(hs_handle* eng);            /* 失败 NULL */
void       hs_session_free(hs_handle* sess);

/* 输入。feed 返回: 0=拒绝, 1=等待, 2=自动上屏(文本经 out_committed) */
int   hs_feed(hs_handle* sess, char ch, char** out_committed);
char* hs_select(hs_handle* sess, int idx);            /* 1-based; 失败 NULL */
char* hs_select_first(hs_handle* sess);               /* 空格首选; 无候选 NULL */
int   hs_backspace(hs_handle* sess);                  /* 1=成功 0=已空 */
void  hs_clear(hs_handle* sess);

/* 中英切换（ascii_composer） */
int   hs_ascii_mode(hs_handle* sess);                 /* 0=中文 1=西文 */
void  hs_set_ascii_mode(hs_handle* sess, int ascii);  /* 设置西文模式 */

/* 状态 */
char* hs_pending(hs_handle* sess);                    /* 当前编码 */
char* hs_candidates(hs_handle* sess, int limit);      /* "词\x01码\x02词\x01码…"; limit<=0 不限 */
char* hs_candidates_page(hs_handle* sess, int offset, int limit); /* 候选页，offset 从 0 开始 */

/* 用户词典 */
int   hs_user_dict_save(hs_handle* eng, const char* path); /* 1=成功 0=失败 */

void  hs_str_free(char* s);

#ifdef __cplusplus
}
#endif
#endif /* HESHUN_H */