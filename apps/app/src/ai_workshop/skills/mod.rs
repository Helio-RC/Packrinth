// === AI-WORKSHOP START ===
// 技能系统（流 D.1/D.2）：模块入口。
// 保持既有 pub API（Skill / SkillLoader）兼容，并暴露匹配与注入函数。
pub mod injector;
pub mod loader;
pub mod matcher;
pub mod sanitizer;

// 下述 re-export 构成模块的公共 API 面；部分供后续任务（Task 16 引擎接入）使用。
pub use loader::SkillLoader;
// === AI-WORKSHOP END ===
