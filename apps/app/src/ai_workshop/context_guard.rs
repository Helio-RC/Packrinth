// === AI-WORKSHOP START ===
// 上下文窗口溢出保护：动态截断 + 摘要压缩判定（流 E 实现摘要本身）。
use crate::ai_workshop::providers::provider_trait::{AiMessage, AiMessageRole};

/// 判定消息列表总字符数是否超过窗口上限 `max_chars`。
/// 超过则调用方应触发摘要/压缩（摘要本身调用 LLM 在流 E，本函数仅提供判定）。
pub fn summarize_needed(messages: &[AiMessage], max_chars: usize) -> bool {
    messages
        .iter()
        .map(|message| message.content.len())
        .sum::<usize>()
        > max_chars
}

/// 将 `messages` 裁剪至约 `max_chars` 字符以内。
///
/// 规则：
/// - 保留首条消息 + 尾部 `max_chars / 2` 字符（尾窗）。
/// - 中间窗口优先保留 `role == "tool"` 的消息（不截断），剩余预算再填充其他消息。
/// - 返回被丢弃的消息数。
pub fn enforce_window(
    messages: &mut Vec<AiMessage>,
    max_chars: usize,
) -> usize {
    let original_len = messages.len();
    let total: usize =
        messages.iter().map(|message| message.content.len()).sum();
    if total <= max_chars || messages.is_empty() {
        return 0;
    }

    let mut kept: Vec<AiMessage> = Vec::new();

    // 首条始终保留。
    let first = messages.remove(0);
    let head_len = first.content.len();
    kept.push(first);

    // 收集尾部窗口（最后 max_chars / 2 字符）；pop 顺序为逆序，最后再 reverse。
    let tail_quota = max_chars / 2;
    let mut tail: Vec<AiMessage> = Vec::new();
    let mut tail_len = 0usize;
    while let Some(last) = messages.pop() {
        if tail_len + last.content.len() <= tail_quota {
            tail_len += last.content.len();
            tail.push(last);
        } else {
            messages.push(last);
            break;
        }
    }

    // 中间窗口预算。
    let mut budget =
        max_chars.saturating_sub(head_len).saturating_sub(tail_len);

    // 第一遍：优先保留 tool 消息（不截断）。
    let mut middle: Vec<AiMessage> = Vec::new();
    for message in messages.iter() {
        if matches!(message.role, AiMessageRole::Tool) {
            middle.push(message.clone());
            budget = budget.saturating_sub(message.content.len());
        }
    }
    // 第二遍：剩余预算填充其他消息（按原顺序）。
    for message in messages.iter() {
        if !matches!(message.role, AiMessageRole::Tool)
            && message.content.len() <= budget
        {
            middle.push(message.clone());
            budget -= message.content.len();
        }
    }

    tail.reverse();
    kept.extend(middle);
    kept.extend(tail);
    *messages = kept;

    original_len - messages.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(role: AiMessageRole, content: &str) -> AiMessage {
        AiMessage {
            role,
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    #[test]
    fn below_threshold_returns_zero() {
        let mut messages = vec![
            msg(AiMessageRole::System, "sys"),
            msg(AiMessageRole::User, "hello"),
        ];
        let dropped = enforce_window(&mut messages, 100);
        assert_eq!(dropped, 0);
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn keeps_head_and_tail() {
        let mut messages = vec![
            msg(AiMessageRole::System, "head"),
            msg(AiMessageRole::User, "m1"),
            msg(AiMessageRole::User, "m2"),
            msg(AiMessageRole::Assistant, "middle-long-text"),
            msg(AiMessageRole::User, "tail"),
        ];
        // max_chars=10：仅首条(4) + 尾窗(5 字符上限, 2+13>5 截断, 保留 tail=4)
        let dropped = enforce_window(&mut messages, 10);
        assert!(dropped > 0);
        // 首条保留
        assert_eq!(messages[0].content, "head");
        // 尾部保留
        assert_eq!(messages.last().unwrap().content, "tail");
        // 中间长文本被丢弃
        assert!(!messages.iter().any(|m| m.content == "middle-long-text"));
    }

    #[test]
    fn prioritizes_tool_messages() {
        let mut messages = vec![
            msg(AiMessageRole::System, "head"),
            msg(AiMessageRole::Assistant, "aa"),
            msg(AiMessageRole::Tool, "tool-result"),
            msg(AiMessageRole::User, "bb"),
        ];
        let dropped = enforce_window(&mut messages, 12);
        assert!(dropped > 0);
        // tool 消息保留
        assert!(
            messages
                .iter()
                .any(|m| matches!(m.role, AiMessageRole::Tool))
        );
        // 中间 assistant 被丢弃（旧 user 落入尾部窗口保留）
        assert!(!messages.iter().any(|m| m.content == "aa"));
        assert!(messages.iter().any(|m| m.content == "bb"));
    }

    #[test]
    fn dropped_count_matches() {
        let mut messages = vec![
            msg(AiMessageRole::System, "head"),
            msg(AiMessageRole::Assistant, "drop-me"),
            msg(AiMessageRole::User, "tail"),
        ];
        let dropped = enforce_window(&mut messages, 8);
        assert_eq!(dropped, 1);
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn summarize_needed_threshold() {
        let messages = vec![
            msg(AiMessageRole::User, "abcd"),
            msg(AiMessageRole::Assistant, "efgh"),
        ];
        assert!(!summarize_needed(&messages, 8)); // 恰好等于上限
        assert!(summarize_needed(&messages, 7)); // 超过上限
        assert!(!summarize_needed(&messages, 100));
    }

    #[test]
    fn empty_messages_noop() {
        let mut messages: Vec<AiMessage> = Vec::new();
        assert_eq!(enforce_window(&mut messages, 10), 0);
        assert!(!summarize_needed(&messages, 10));
    }
}
// === AI-WORKSHOP END ===
