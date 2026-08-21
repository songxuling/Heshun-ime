//! 处理器骨架（Processor chain）。
//!
//! 对标 Rime 的 engine/processors 流水线。每个 processor 有机会：
//! - 拦截按键并返回结果（消费该键）
//! - 修改按键后传给下一个 processor
//! - 不做处理（放行）
//!
//! 链顺序：ascii_composer → recognizer → key_binder → speller → punctuator

use crate::engine::FeedResult;

/// 处理器上下文，暴露 session 状态供 processor 读取。
pub struct ProcessCtx {
    /// 是否需要将缓冲内容提交（如中英切换时 commit_code）
    pub flush_requested: bool,
    /// 是否处于西文模式（ascii_composer 控制）
    pub ascii_mode: bool,
    /// 临时放行标志：反查/标点模式激活后，下一个按键按原字符上屏而非走 speller
    pub bypass_speller: bool,
    /// 翻页偏移（key_binder 控制候选页）
    pub page_offset: usize,
}

impl Default for ProcessCtx {
    fn default() -> Self {
        ProcessCtx { flush_requested: false, ascii_mode: false, bypass_speller: false, page_offset: 0 }
    }
}

/// 处理器结果。
pub enum ProcessOutcome {
    /// 处理器消费了按键，返回 feed 结果
    Handled(FeedResult),
    /// 处理器修改了按键，传给下一个处理器
    Modified(char),
}

/// 处理器 trait。
pub trait Processor {
    fn name(&self) -> &str;
    fn process(&self, key: char, pending: &str, ctx: &mut ProcessCtx) -> Option<ProcessOutcome>;
}

/// 处理器链。
pub struct ProcessorChain {
    processors: Vec<Box<dyn Processor>>,
}

impl ProcessorChain {
    pub fn new() -> Self {
        ProcessorChain { processors: Vec::new() }
    }

    pub fn add(&mut self, p: Box<dyn Processor>) {
        self.processors.push(p);
    }

    pub fn is_empty(&self) -> bool {
        self.processors.is_empty()
    }

    /// 按键通过整条链。返回 (FeedResult, Option<modified_char>)。
    /// 如果链中某个处理器 Handled 了，直接返回该结果；
    /// 按键可能在链中被修改（Modified），最终退出链的字符用于 speller。
    pub fn process(&self, key: char, pending: &str, ctx: &mut ProcessCtx) -> Option<ProcessOutcome> {
        let mut current_key = key;
        for p in &self.processors {
            match p.process(current_key, pending, ctx) {
                Some(ProcessOutcome::Handled(r)) => return Some(ProcessOutcome::Handled(r)),
                Some(ProcessOutcome::Modified(c)) => current_key = c,
                None => {} // 放行
            }
        }
        // 链走完，返回最终（可能被修改的）按键
        None
    }

    /// 退出链后的最终按键（用于 speller）
    pub fn final_key(&self, key: char, pending: &str, ctx: &mut ProcessCtx) -> char {
        let mut current_key = key;
        for p in &self.processors {
            match p.process(current_key, pending, ctx) {
                Some(ProcessOutcome::Handled(_)) => return '\0', // 已处理，返回 sentinel
                Some(ProcessOutcome::Modified(c)) => {
                    ctx.bypass_speller = true;
                    current_key = c;
                }
                None => {}
            }
        }
        current_key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Passthrough;
    impl Processor for Passthrough {
        fn name(&self) -> &str { "passthrough" }
        fn process(&self, _key: char, _pending: &str, _ctx: &mut ProcessCtx) -> Option<ProcessOutcome> {
            None
        }
    }

    #[test]
    fn empty_chain_passthrough() {
        let chain = ProcessorChain::new();
        let mut ctx = ProcessCtx::default();
        let r = chain.final_key('a', "", &mut ctx);
        assert_eq!(r, 'a');
    }

    #[test]
    fn chain_with_passthrough() {
        let mut chain = ProcessorChain::new();
        chain.add(Box::new(Passthrough));
        let mut ctx = ProcessCtx::default();
        assert_eq!(chain.final_key('x', "", &mut ctx), 'x');
    }
}