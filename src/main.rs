// ================================================================
// 项目: 音频质量分析器 (AudioQuality-rs)
// 文件: src/main.rs
// 作者: AudioQuality-rs 开发团队
// 版本: 4.0.0
// 描述: 程序主入口点，负责命令行解析、用户交互和分析流程协调
//
// 功能概述:
// - 支持直接模式和交互模式两种操作方式
// - 集成FFmpeg音频指标提取、质量评分和报告生成
// - 提供用户友好的进度显示和结果摘要
// - 实现高性能并行处理和错误容错机制
// ================================================================

mod analyzer;

// ================================================================
// 依赖导入 (Dependencies Import)
// ================================================================

// --- 内部模块导入 (Internal Modules) ---
use crate::analyzer::{
    ffmpeg,                    // FFmpeg交互模块，负责音频指标提取
    metrics::FileMetrics,      // 音频文件指标数据结构
    scoring::QualityScorer,    // 质量评分算法模块
    report::ReportGenerator,   // 报告生成模块
};

// --- 外部依赖导入 (External Dependencies) ---
use anyhow::{anyhow, Context, Result};  // 错误处理库，提供丰富的错误上下文
use chrono::Local;                      // 时间处理库，用于显示分析开始/结束时间
use clap::Parser;                       // 命令行参数解析库
use indicatif::{ProgressBar, ProgressStyle}; // 进度条显示库
use rayon::prelude::*;                  // 并行处理库，用于多线程音频分析
use std::env;                           // 环境变量访问
use std::fs;                            // 文件系统操作
use std::io::{self, Write};             // 输入输出操作
use std::path::{Path, PathBuf};         // 路径处理
use walkdir::WalkDir;                   // 目录递归遍历
use which::which;                       // 系统PATH中可执行文件查找

// ================================================================
// 常量定义 (Constants Definition)
// ================================================================

/// 支持的音频文件扩展名列表
/// 包含常见的无损和有损音频格式
const SUPPORTED_EXTENSIONS: [&str; 10] = [
    "wav", "mp3", "m4a", "flac", "aac", "ogg", "opus", "wma", "aiff", "alac",
];

// ================================================================
// 命令行接口定义 (Command Line Interface Definition)
// ================================================================

/// 程序命令行接口结构体
///
/// 使用 clap 库定义命令行参数解析规则。支持两种运行模式：
/// 1. 直接模式：提供路径参数，直接开始分析
/// 2. 交互模式：不提供参数，进入菜单驱动的交互界面
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "一个基于 FFmpeg 的纯 Rust 音频质量分析工具",
    long_about = "该工具可以递归扫描指定目录下的音频文件，并行提取多项技术指标，使用先进的评分算法进行质量评估，并生成详细的 CSV 和 JSON 格式报告。如果未提供路径参数，则会进入交互模式。"
)]
struct Cli {
    /// 要分析的音频文件夹路径（可选）
    ///
    /// 如果提供此参数，程序将直接分析指定文件夹中的所有支持格式的音频文件。
    /// 如果不提供，程序将进入交互模式，允许用户通过菜单选择操作。
    #[arg(value_name = "PATH", help = "要递归扫描和处理的音频文件夹路径")]
    path: Option<PathBuf>,
}

// ================================================================
// 交互模式功能实现 (Interactive Mode Implementation)
// ================================================================

/// 显示交互式主菜单
///
/// 在控制台输出用户可选择的操作选项，包括：
/// - 选项1：开始音频文件分析
/// - 选项2：退出程序
///
/// 使用 flush() 确保提示信息立即显示，提供良好的用户体验
fn show_menu() {
    println!("\n--- 🎵 音频质量分析器交互模式 ---");
    println!("1. 📊 分析音频文件");
    println!("2. 🚪 退出程序");
    print!("请选择一个操作 (1-2): ");
    io::stdout().flush().unwrap();
}

/// 交互模式主循环控制器
///
/// 管理用户交互会话的完整生命周期：
/// 1. 显示菜单选项
/// 2. 读取用户输入
/// 3. 根据选择执行相应操作
/// 4. 处理错误和异常情况
/// 5. 循环直到用户选择退出
///
/// # 返回值
/// - `Ok(())`: 正常退出
/// - `Err`: 发生不可恢复的错误
fn interactive_mode() -> Result<()> {
    loop {
        show_menu();

        // 读取用户输入
        let mut choice = String::new();
        io::stdin().read_line(&mut choice)?;

        // 根据用户选择执行相应操作
        match choice.trim() {
            "1" => {
                // 选项1：开始音频分析流程
                println!("\n🔍 准备开始音频质量分析...");
                match get_path_from_user_interaction() {
                    Ok(path) => {
                        // 成功获取有效路径，开始分析
                        if let Err(e) = run_analysis(&path) {
                            eprintln!("\n❌ 分析过程中发生错误: {e}");
                            println!("💡 建议检查文件路径和权限后重试");
                        }
                    }
                    Err(e) => {
                        // 路径获取失败
                        eprintln!("\n❌ 无法获取有效路径: {e}");
                        println!("💡 请确保输入的是有效的文件夹路径");
                    }
                }
            }
            "2" => {
                // 选项2：退出程序
                println!("\n👋 感谢使用音频质量分析器，再见！");
                break;
            }
            _ => {
                // 无效输入处理
                eprintln!("\n❌ 无效的选择，请输入 '1' 或 '2'");
            }
        }
    }
    Ok(())
}

/// 增强的用户路径输入交互系统
///
/// 提供用户友好的路径输入体验，包括：
/// - 清晰的输入提示和使用示例
/// - 实时路径验证和详细错误提示
/// - 支持相对路径和绝对路径
/// - 自动路径规范化处理
/// - 循环输入直到获得有效路径
///
/// # 返回值
/// - `Ok(PathBuf)`: 验证通过并规范化的有效路径
/// - `Err`: 发生不可恢复的I/O错误
fn get_path_from_user_interaction() -> Result<PathBuf> {
    println!("\n📁 音频文件夹路径输入");
    println!("💡 支持格式: 相对路径或绝对路径");
    println!("📝 示例: ./music 或 /Users/username/Music 或 C:\\Music");
    println!("🔄 输入错误时可重新输入，按 Ctrl+C 退出");

    loop {
        print!("\n🎯 请输入音频文件夹路径: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let path_str = input.trim();

        // 检查空输入
        if path_str.is_empty() {
            eprintln!("❌ 路径不能为空，请重新输入");
            continue;
        }

        let path = PathBuf::from(path_str);

        // 验证路径存在性和类型
        if path.is_dir() {
            match path.canonicalize() {
                Ok(canonical_path) => {
                    println!("✅ 路径验证通过: {}", canonical_path.display());
                    return Ok(canonical_path);
                }
                Err(e) => {
                    eprintln!("❌ 路径规范化失败: {e}");
                    eprintln!("💡 请检查路径权限");
                }
            }
        } else if path.exists() {
            eprintln!("❌ \"{}\" 不是一个文件夹，请提供文件夹路径", path.display());
        } else {
            eprintln!("❌ 路径不存在: \"{}\"", path.display());
            eprintln!("💡 请检查路径拼写和权限");
        }
    }
}

// --- 核心分析逻辑 ---

/// 查找 FFmpeg 可执行文件。
///
/// 查找顺序:
/// 1. 首先在系统的 PATH 环境变量中查找 `ffmpeg`。
/// 2. 如果找不到，则回退到在项目 `resources` 目录中查找。
/// 3. 如果都找不到，返回一个错误。
fn find_ffmpeg_path() -> Result<PathBuf> {
    // 优先：在系统 PATH 中查找
    match which("ffmpeg") {
        Ok(path) => {
            println!("成功在系统 PATH 中找到 FFmpeg: {}", path.display());
            Ok(path)
        }
        Err(_) => {
            // 次选：在本地 resources 目录中查找
            println!("未在系统 PATH 中找到 FFmpeg，正在尝试备用路径...");
            let current_exe_path = env::current_exe()?;
            let project_root = current_exe_path
                .ancestors()
                .nth(3)
                .unwrap_or_else(|| Path::new(""));
            let ffmpeg_path = project_root.join("resources/ffmpeg");

            if ffmpeg_path.exists() {
                println!("成功在 resources 目录中找到 FFmpeg: {}", ffmpeg_path.display());
                Ok(ffmpeg_path)
            } else {
                Err(anyhow!(
                    "错误: 在系统 PATH 和备用目录 ({}) 中都找不到 ffmpeg 可执行文件。\n请采取以下任一措施后重试:\n1. 安装 FFmpeg 并确保其位于您的系统 PATH 中。\n2. 将 FFmpeg 可执行文件放置在上述备用目录中。",
                    ffmpeg_path.display()
                ))
            }
        }
    }
}


/// 对指定路径下的音频文件执行完整的分析流程。
///
/// # 参数
/// - `base_folder_path`: 要扫描和分析的根目录的路径。
fn run_analysis(base_folder_path: &Path) -> Result<()> {
    println!("\n--- ✨ 开始执行分析流程 ---");
    println!("分析开始时间: {}", Local::now().format("%Y-%m-%d %H:%M:%S"));

    // --- 环境检查：定位 FFmpeg ---
    let ffmpeg_path = find_ffmpeg_path()?;
    
    println!("正在扫描文件夹: {}", base_folder_path.display());

    // --- 文件扫描 ---
    let audio_files: Vec<PathBuf> = WalkDir::new(base_folder_path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|path| {
            path.extension()
                .and_then(|s| s.to_str())
                .map(|ext| SUPPORTED_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
                .unwrap_or(false)
        })
        .collect();

    if audio_files.is_empty() {
        println!("在指定路径下没有找到支持的音频文件。");
        return Ok(())
    }

    let total_files = audio_files.len();
    println!(
        "扫描完成，找到 {total_files} 个音频文件待处理。开始并行分析..."
    );

    // --- 并行处理 ---
    let bar = ProgressBar::new(total_files as u64);
    bar.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) - {msg}")
            .unwrap()
            .progress_chars("#>- "),
    );

    let results: Vec<FileMetrics> = audio_files
        .into_par_iter()
        .map(|path| {
            bar.set_message(
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            );
            let result = ffmpeg::process_file(&path, &ffmpeg_path);
            bar.inc(1);
            result
        })
        .filter_map(|res| match res {
            Ok(m) => Some(m),
            Err(e) => {
                bar.println(format!("处理失败: {e}"));
                None
            }
        })
        .collect();

    bar.finish_with_message("数据提取完成。");

    // --- 质量评分分析 ---
    println!("正在进行质量评分分析...");
    let scorer = QualityScorer::new();
    let quality_analyses = scorer.analyze_files(&results);

    // --- 生成报告 ---
    let report_generator = ReportGenerator::new();

    // 保存CSV报告
    let csv_output_path = base_folder_path.join("audio_quality_report.csv");
    report_generator.generate_csv_report(&quality_analyses, &csv_output_path)?;

    // 显示分析摘要
    report_generator.display_summary(&quality_analyses);

    // --- 保存原始JSON数据 ---
    let json_output_path = base_folder_path.join("analysis_data.json");
    println!("\n正在保存原始数据到: {}", json_output_path.display());

    fs::write(
        &json_output_path,
        serde_json::to_string_pretty(&results)?,
    )
    .context("无法写入 analysis_data.json 文件")?;
    println!("原始数据保存成功！");

    // --- 任务结束 ---
    println!("\n分析结束时间: {}", Local::now().format("%Y-%m-%d %H:%M:%S"));
    println!("--- ✅ 分析流程顺利完成 ---");

    Ok(())
}

// --- 程序入口 ---

/// 程序的主函数，根据命令行参数决定进入直接模式还是交互模式。
fn main() -> Result<()> {
    let cli = Cli::parse();

    println!("欢迎使用音频质量分析器 (Rust 重构版)");

    match cli.path {
        // 模式一：用户通过命令行参数提供了路径
        Some(path) => {
            if path.is_dir() {
                let absolute_path = path.canonicalize()?;
                run_analysis(&absolute_path)
            } else {
                Err(anyhow!(
                    "错误: 命令行提供的路径 \"{}\" 不是一个有效的文件夹或不存在。",
                    path.display()
                ))
            }
        }
        // 模式二：没有提供命令行参数，进入交互模式
        None => interactive_mode(),
    }
}

// --- 单元测试 ---
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_extensions_are_lowercase() {
        for &ext in SUPPORTED_EXTENSIONS.iter() {
            assert_eq!(
                ext,
                ext.to_lowercase(),
                "支持的扩展名 '{ext}' 应该全部为小写。"
            );
        }
    }
}