// examples/skills_demo.rs

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use hello_agents::core::traits::tool::Tool;
use hello_agents::skills::skill_loader::{Skill, SkillLoader};
use hello_agents::tools::builtin::skill::SkillTool;
use tempfile::TempDir;

/// 列出指定目录下的文件（不递归）
fn list_files(dir: &std::path::Path) -> Vec<PathBuf> {
    if !dir.is_dir() {
        return vec![];
    }
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect()
}

// ==================== 示例1：技能加载器基础 ====================
async fn demo_skill_loader() -> anyhow::Result<()> {
    println!("{}", "=".repeat(60));
    println!("示例 1: 技能加载器基础");
    println!("{}", "=".repeat(60));

    let tmp = TempDir::new()?;
    let skills_dir = tmp.path().join("skills");
    std::fs::create_dir_all(&skills_dir)?;

    // 创建 pdf 技能
    let pdf_dir = skills_dir.join("pdf");
    std::fs::create_dir(&pdf_dir)?;
    std::fs::write(
        pdf_dir.join("SKILL.md"),
        r#"---
name: pdf
description: Process PDF files and extract text content
---

# PDF Processing Skill

This skill helps you process PDF files.

## Usage
Use `pdftotext` command to extract text from PDF files.

## Example
```bash
pdftotext input.pdf output.txt
```"#,
    )?;

    let mut loader = SkillLoader::new(&skills_dir);

    println!("\n技能描述（元数据）:");
    let descriptions = loader.get_descriptions();
    println!("{}", descriptions);

    println!("\n加载完整技能:");
    if let Some(skill) = loader.get_skill("pdf") {
        println!("  名称: {}", skill.name);
        println!("  描述: {}", skill.description);
        println!("  内容长度: {} 字符", skill.body.len());
        let preview = &skill.body[..std::cmp::min(100, skill.body.len())];
        println!("  内容预览: {}...", preview);
    } else {
        println!("  技能未找到");
    }

    println!("\n✅ 技能加载器测试完成");
    Ok(())
}

// ==================== 示例2：技能工具使用 ====================
async fn demo_skill_tool() -> anyhow::Result<()> {
    println!("\n{}", "=".repeat(60));
    println!("示例 2: 技能工具使用");
    println!("{}", "=".repeat(60));

    let tmp = TempDir::new()?;
    let skills_dir = tmp.path().join("skills");
    std::fs::create_dir_all(&skills_dir)?;

    // 创建 code-review 技能
    let review_dir = skills_dir.join("code-review");
    std::fs::create_dir(&review_dir)?;
    std::fs::write(
        review_dir.join("SKILL.md"),
        r#"---
name: code-review
description: Perform comprehensive code reviews
---

# Code Review Skill

## Checklist
1. Security vulnerabilities
2. Performance issues
3. Code style consistency
4. Error handling
5. Test coverage

## Best Practices
- Use static analysis tools
- Check for common patterns
- Review documentation
"#,
    )?;

    let loader = Arc::new(Mutex::new(SkillLoader::new(&skills_dir)));
    let skill_tool = SkillTool::new(loader);

    println!("\n调用技能工具:");
    let response = skill_tool
        .execute(serde_json::json!({"skill": "code-review"}))
        .await?;

    println!("  状态: {:?}", response.status);
    println!("  内容长度: {} 字符", response.text.len());
    let preview = &response.text[..std::cmp::min(200, response.text.len())];
    println!("  内容预览:\n{}...", preview);

    println!("\n调用不存在的技能:");
    let response = skill_tool
        .execute(serde_json::json!({"skill": "nonexistent"}))
        .await?;
    println!("  状态: {:?}", response.status);
    if let Some(err) = &response.error_info {
        println!("  错误码: {}", err.code);
    }

    println!("\n✅ 技能工具测试完成");
    Ok(())
}

// ==================== 示例3：零配置激活 ====================
async fn demo_zero_config_activation() -> anyhow::Result<()> {
    println!("\n{}", "=".repeat(60));
    println!("示例 3: 零配置自动激活");
    println!("{}", "=".repeat(60));

    let skills_dir = std::env::current_dir()?.join("skills");
    if skills_dir.exists() {
        println!("\n检测到 skills 目录: {}", skills_dir.display());
        println!("  若启用 skills_auto_register，Skill 工具将被自动注册。");
    } else {
        println!("\n⚠️ skills 目录不存在: {}", skills_dir.display());
        println!("   创建 skills 目录并添加 SKILL.md 文件即可自动激活");
    }

    println!("\n✅ 零配置激活测试完成");
    Ok(())
}

// ==================== 示例4：带参数的技能 ====================
async fn demo_skill_with_arguments() -> anyhow::Result<()> {
    println!("\n{}", "=".repeat(60));
    println!("示例 4: 带参数的技能");
    println!("{}", "=".repeat(60));

    let tmp = TempDir::new()?;
    let skills_dir = tmp.path().join("skills");
    std::fs::create_dir_all(&skills_dir)?;

    // 创建带占位符的技能
    let template_dir = skills_dir.join("template");
    std::fs::create_dir(&template_dir)?;
    std::fs::write(
        template_dir.join("SKILL.md"),
        r#"---
name: template
description: A template skill with arguments
---

# Template Skill

This skill accepts custom arguments.

## Arguments
$ARGUMENTS

## Usage
Process the arguments above and generate output.
"#,
    )?;

    let loader = Arc::new(Mutex::new(SkillLoader::new(&skills_dir)));
    let skill_tool = SkillTool::new(loader);

    println!("\n调用带参数的技能:");
    let response = skill_tool
        .execute(serde_json::json!({
            "skill": "template",
            "args": "Target: Python code\nFocus: Performance optimization"
        }))
        .await?;

    println!("  状态: {:?}", response.status);
    let preview = &response.text[..std::cmp::min(300, response.text.len())];
    println!("  内容预览:\n{}...", preview);

    assert!(response.text.contains("Target: Python code"));
    assert!(response.text.contains("Performance optimization"));

    println!("\n✅ 带参数技能测试完成");
    Ok(())
}

// ==================== 示例5：技能资源文件 ====================
async fn demo_skill_resources() -> anyhow::Result<()> {
    println!("\n{}", "=".repeat(60));
    println!("示例 5: 技能资源文件");
    println!("{}", "=".repeat(60));

    let tmp = TempDir::new()?;
    let skills_dir = tmp.path().join("skills");
    std::fs::create_dir_all(&skills_dir)?;

    // 创建带资源的技能
    let mcp_dir = skills_dir.join("mcp-builder");
    std::fs::create_dir(&mcp_dir)?;
    std::fs::write(
        mcp_dir.join("SKILL.md"),
        r#"---
name: mcp-builder
description: Build MCP servers
---

# MCP Server Builder

Build Model Context Protocol servers.

## Resources
Check the scripts/ and examples/ folders for templates.
"#,
    )?;

    // 创建资源文件夹
    let scripts_dir = mcp_dir.join("scripts");
    let examples_dir = mcp_dir.join("examples");
    let references_dir = mcp_dir.join("references");
    std::fs::create_dir_all(&scripts_dir)?;
    std::fs::create_dir_all(&examples_dir)?;
    std::fs::create_dir_all(&references_dir)?;

    // 创建资源文件
    std::fs::write(scripts_dir.join("template.py"), "# MCP template")?;
    std::fs::write(examples_dir.join("weather.py"), "# Weather example")?;
    std::fs::write(references_dir.join("spec.md"), "# MCP spec")?;

    // 加载技能
    let mut loader = SkillLoader::new(&skills_dir);
    if let Some(skill) = loader.get_skill("mcp-builder") {
        // 手动扫描资源目录
        let scripts = list_files(&skill.dir.join("scripts"));
        let examples = list_files(&skill.dir.join("examples"));
        let references = list_files(&skill.dir.join("references"));

        println!("\n技能资源:");
        println!("  脚本: {:?}", scripts);
        println!("  示例: {:?}", examples);
        println!("  参考: {:?}", references);

        assert_eq!(scripts.len(), 1);
        assert_eq!(examples.len(), 1);
        assert_eq!(references.len(), 1);
    } else {
        println!("\n技能未找到");
    }

    println!("\n✅ 技能资源测试完成");
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    demo_skill_loader().await?;
    demo_skill_tool().await?;
    demo_zero_config_activation().await?;
    demo_skill_with_arguments().await?;
    demo_skill_resources().await?;

    println!("\n{}", "=".repeat(60));
    println!("✅ 所有示例运行完成！");
    println!("{}", "=".repeat(60));
    Ok(())
}