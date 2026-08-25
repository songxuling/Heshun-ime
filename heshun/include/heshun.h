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

/* Owned runtime ABI v1. All views remain valid until hs_runtime_result_free. */
typedef struct hs_text_view { const unsigned char* ptr; unsigned int len; } hs_text_view;
typedef struct hs_candidate_view {
    unsigned int source;
    unsigned int ordinal;
    hs_text_view word;
    hs_text_view annotation;
    hs_text_view label;
} hs_candidate_view;
typedef struct hs_runtime_event_t {
    unsigned int opcode; /* 0=text,1=backspace,2=delete,3=escape,4=space,5=enter,
                            6=select,7=move,8=page,9=toggle-ascii,10=toggle-full-shape,12=reset */
    long long value;
    unsigned int source;
    unsigned int ordinal;
} hs_runtime_event_t;
typedef struct hs_runtime_result {
    unsigned int disposition;
    unsigned int composition;
    hs_text_view committed;
    hs_text_view pending;
    const hs_candidate_view* candidates;
    unsigned int candidate_count;
    unsigned int page_index;
    unsigned int page_size;
    unsigned int total_candidates;
    unsigned int selected_source;
    unsigned int selected_ordinal;
    unsigned char has_previous;
    unsigned char has_next;
    unsigned char ascii_mode;
    unsigned char full_shape;
    unsigned char composing;
    unsigned int error_code;
} hs_runtime_result;

unsigned int hs_runtime_abi_version(void);
hs_handle* hs_runtime_new_schema(const char* schema_path);
void hs_runtime_free(hs_handle* runtime);
hs_handle* hs_runtime_event(hs_handle* runtime, const hs_runtime_event_t* event);
const hs_runtime_result* hs_runtime_result_view(const hs_handle* result);
void hs_runtime_result_free(hs_handle* result);
int hs_runtime_user_dict_save(hs_handle* runtime, const char* path);

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