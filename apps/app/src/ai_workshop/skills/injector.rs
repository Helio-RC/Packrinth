// === AI-WORKSHOP START ===
use std::fmt::Write as _;
// System Prompt 技能注入：将技能名 + 描述 + guide_md（前 2000 字符）拼接为 Markdown 块。
// 本模块仅提供纯函数；实际接入推理引擎由 Task 16 完成。
use super::loader::Skill;

/// 将技能列表拼接为系统提示注入段（Markdown 块格式）。
pub fn build_skill_prompt(skills: &[Skill]) -> String {
    let mut out = String::new();
    for skill in skills {
        let guide: String = skill.guide_md.chars().take(2000).collect();
        let _ = write!(
            out,
            "\n\n## 技能: {}\n\n描述: {}\n\n指南:\n{}\n",
            skill.name, skill.description, guide
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(name: &str, description: &str, guide: &str) -> Skill {
        Skill {
            name: name.to_string(),
            description: description.to_string(),
            keywords: vec![],
            priority: 0,
            version: "1.0".to_string(),
            author: "user".to_string(),
            enabled: true,
            guide_md: guide.to_string(),
        }
    }

    #[test]
    fn includes_name_description_and_truncated_guide() {
        let guide = "g".repeat(3000);
        let out = build_skill_prompt(&[skill("helper", "a helper", &guide)]);
        assert!(out.contains("## 技能: helper"));
        assert!(out.contains("a helper"));
        // guide_md 前 2000 字符被注入，整体不包含完整 3000 字符。
        assert!(out.len() < 2500);
    }
}
// === AI-WORKSHOP END ===
