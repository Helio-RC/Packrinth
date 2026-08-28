# skills

L3 技能系统（Markdown 知识注入；热加载；用户可编辑）。

## 组成

- `loader.rs`：扫描 `<data_dir>/ai-workshop/skills/`，解析 `skill.toml`（校验 priority 0~100、keywords 1~20、name Unicode 字母数字空格连字符；失败跳过整个技能并记录）；`import_skill` 来源豁免前缀限制（见 goal.md §3.3）；`failed_skills()` 供前端展示。
- `matcher.rs`：大小写不敏感全词匹配 + priority 排序（先 priority 降序，再命中数，再名称）。
- `injector.rs`：组装技能注入段落。
- `sanitizer.rs`：三层净化（pulldown-cmark 拒 HTML → ammonia 清洗 → 链接协议白名单）。

## 测试

loader 覆盖校验拒绝/Unicode 接受/保持启用状态/导入逃逸；sanitizer/matcher 有独立用例。
