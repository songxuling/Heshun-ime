//! 提交历史：为多段编辑和用户词典事务提供最小可撤销记录。

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitRecord {
    pub text: String,
    pub code: String,
    pub learned: bool,
}

#[derive(Debug, Default, Clone)]
pub struct CommitHistory {
    records: Vec<CommitRecord>,
    limit: usize,
}

impl CommitHistory {
    pub fn new(limit: usize) -> Self { Self { records: Vec::new(), limit: limit.max(1) } }
    pub fn push(&mut self, record: CommitRecord) {
        self.records.push(record);
        if self.records.len() > self.limit { self.records.remove(0); }
    }
    pub fn last(&self) -> Option<&CommitRecord> { self.records.last() }
    pub fn pop(&mut self) -> Option<CommitRecord> { self.records.pop() }
    pub fn len(&self) -> usize { self.records.len() }
    pub fn is_empty(&self) -> bool { self.records.is_empty() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_is_bounded_and_reversible() {
        let mut history = CommitHistory::new(2);
        for text in ["一", "二", "三"] {
            history.push(CommitRecord { text: text.into(), code: "a".into(), learned: true });
        }
        assert_eq!(history.len(), 2);
        assert_eq!(history.last().unwrap().text, "三");
        assert_eq!(history.pop().unwrap().text, "三");
    }
}
