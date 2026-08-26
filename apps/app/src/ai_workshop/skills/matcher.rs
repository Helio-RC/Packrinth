// === AI-WORKSHOP START ===
// 关键词匹配与优先级排序（计划 §4.5 D.2）：
//   大小写不敏感全词匹配（query 分词后与 keywords 精确相等比较）；
//   仅匹配 enabled 技能；排序：priority 降序 → 命中数降序 → 名称稳定（字典序）排序。
use super::loader::Skill;

/// 在给定技能列表中匹配 enabled 技能，返回最多 `max_results` 个按规则排序的技能。
pub fn match_skills(skills: &[Skill], query: &str, max_results: usize) -> Vec<Skill> {
	let mut matched: Vec<(Skill, usize)> = Vec::new();
	for skill in skills {
		if !skill.enabled {
			continue;
		}
		let hits = count_hits(&skill.keywords, query);
		if hits > 0 {
			matched.push((skill.clone(), hits));
		}
	}
	matched.sort_by(|(a, a_hits), (b, b_hits)| {
		b.priority
			.cmp(&a.priority)
			.then(b_hits.cmp(a_hits))
			.then(a.name.cmp(&b.name))
	});
	matched
		.into_iter()
		.take(max_results)
		.map(|(skill, _)| skill)
		.collect()
}

/// 统计命中的关键词数：query 分词后与每个关键词大小写不敏感全词比较。
fn count_hits(keywords: &[String], query: &str) -> usize {
	let tokens: Vec<String> = query
		.split_whitespace()
		.map(|token| token.to_lowercase())
		.collect();
	if tokens.is_empty() {
		return 0;
	}
	keywords
		.iter()
		.filter(|keyword| {
			let keyword = keyword.to_lowercase();
			tokens.iter().any(|token| token == &keyword)
		})
		.count()
}

#[cfg(test)]
mod tests {
	use super::*;

	fn skill(name: &str, priority: u8, keywords: &[&str], enabled: bool) -> Skill {
		Skill {
			name: name.to_string(),
			description: String::new(),
			keywords: keywords.iter().map(|s| s.to_string()).collect(),
			priority,
			version: "1.0".to_string(),
			author: "user".to_string(),
			enabled,
			guide_md: String::new(),
		}
	}

	#[test]
	fn matches_case_insensitive_and_whole_word() {
		let skills = vec![
			skill("ftb", 50, &["ftb"], true),
			skill("ftbrecipes", 50, &["ftbrecipes"], true),
		];
		let res = match_skills(&skills, "FTB", 10);
		assert_eq!(res.len(), 1);
		assert_eq!(res[0].name, "ftb");
	}

	#[test]
	fn sorts_by_priority_then_hits_then_name() {
		let skills = vec![
			skill("a", 10, &["x"], true),
			skill("b", 90, &["x"], true),
			skill("c", 90, &["x", "y"], true),
			skill("d", 90, &["x"], true),
		];
		let res = match_skills(&skills, "x y", 10);
		let names: Vec<&str> = res.iter().map(|s| s.name.as_str()).collect();
		assert_eq!(names, vec!["c", "b", "d", "a"]);
	}

	#[test]
	fn ignores_disabled_skills() {
		let skills = vec![
			skill("on", 50, &["x"], true),
			skill("off", 50, &["x"], false),
		];
		let res = match_skills(&skills, "x", 10);
		assert_eq!(res.len(), 1);
		assert_eq!(res[0].name, "on");
	}

	#[test]
	fn respects_max_results() {
		let skills = vec![
			skill("a", 50, &["x"], true),
			skill("b", 50, &["x"], true),
			skill("c", 50, &["x"], true),
		];
		let res = match_skills(&skills, "x", 2);
		assert_eq!(res.len(), 2);
	}
}
// === AI-WORKSHOP END ===