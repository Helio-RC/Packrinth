# toolchain

L2 可执行工具链（编译时固化；由原子工具组合的复合流程）。

## 组成

- `toolchain_trait.rs`：`ExecutableToolchain{ name, description, execute(instance_id, params, ctx) }` + `ToolchainRegistry`（register/list/get）。
- `builtin/`：`export_mods`（mods 打包 zip）、`kubejs_gen`（KubeJS 脚本写入）、`ct_gen`（CraftTweaker 脚本）、`ftb_recipe`（配方 JSON → KubeJS/CT 代码）、`mod_config`（生成配置骨架）。

执行入口：`execute_toolchain_command`（受实例写锁 + 300s 超时约束），列表 `list_toolchains`。

## 测试

各内置链纯函数单测（script_dir 映射、配方渲染、骨架内容）+ 元数据测试。
