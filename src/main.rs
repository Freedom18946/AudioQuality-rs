mod analyzer;

use crate::analyzer::{
    cache::{self, AnalysisCache, FileFingerprint},
    ffmpeg,
    metrics::FileMetrics,
    report::ReportGenerator,
    safe_io,
    scoring::{QualityScorer, ScoringProfile},
};
use anyhow::{anyhow, Context, Result};
use chrono::Local;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;
use walkdir::WalkDir;
use which::which;

const SUPPORTED_EXTENSIONS: [&str; 10] = [
    "wav", "mp3", "m4a", "flac", "aac", "ogg", "opus", "wma", "aiff", "alac",
];

#[derive(Parser, Debug, Clone)]
#[command(
    author,
    version,
    about = "一个基于 FFmpeg 的纯 Rust 音频质量分析工具",
    long_about = "递归扫描目录中的音频文件，提取技术指标并输出 CSV/JSON 报告。默认启用安全模式（原子写入、符号链接防护、超时与并发限制）。"
)]
struct Cli {
    #[arg(value_name = "PATH", help = "要递归扫描和处理的音频文件夹路径")]
    path: Option<PathBuf>,

    #[arg(
        long,
        default_value_t = 90,
        help = "每个 FFmpeg/FFprobe 子进程超时（秒）"
    )]
    ffmpeg_timeout_seconds: u64,

    #[arg(
        long,
        help = "允许同时运行的 FFmpeg/FFprobe 子进程数（默认: CPU 核心数）"
    )]
    max_ffmpeg_processes: Option<usize>,

    #[arg(long, help = "禁用安全模式（不推荐）")]
    unsafe_mode: bool,

    #[arg(long, help = "禁用增量缓存（默认开启）")]
    no_cache: bool,

    #[arg(long, help = "额外生成 JSONL 报告")]
    jsonl: bool,

    #[arg(long, help = "额外生成 SARIF 报告")]
    sarif: bool,

    #[arg(
        long,
        default_value = "pop",
        help = "评分档案: pop(默认, 适合A-pop/J-pop/K-pop), broadcast, archive"
    )]
    profile: String,
}

#[derive(Debug, Clone)]
struct AppConfig {
    command_timeout: Duration,
    max_ffmpeg_processes: usize,
    safe_mode: bool,
    cache_enabled: bool,
    emit_jsonl: bool,
    emit_sarif: bool,
    scoring_profile: ScoringProfile,
}

#[derive(Debug)]
struct ProcessedRecord {
    metrics: FileMetrics,
    fingerprint: FileFingerprint,
}

fn show_menu() -> Result<()> {
    println!("\n--- 音频质量分析器交互模式 ---");
    println!("1. 分析音频文件");
    println!("2. 退出程序");
    print!("请选择一个操作 (1-2): ");
    io::stdout().flush()?;
    Ok(())
}

fn interactive_mode(config: &AppConfig) -> Result<()> {
    loop {
        show_menu()?;

        let mut choice = String::new();
        io::stdin().read_line(&mut choice)?;

        match choice.trim() {
            "1" => {
                println!("\n准备开始音频质量分析...");
                match get_path_from_user_interaction() {
                    Ok(path) => {
                        if let Err(e) = run_analysis(&path, config) {
                            eprintln!("\n分析过程中发生错误: {e}");
                        }
                    }
                    Err(e) => {
                        eprintln!("\n无法获取有效路径: {e}");
                    }
                }
            }
            "2" => {
                println!("\n感谢使用，再见。");
                break;
            }
            _ => eprintln!("\n无效选择，请输入 1 或 2"),
        }
    }
    Ok(())
}

fn get_path_from_user_interaction() -> Result<PathBuf> {
    println!("\n请输入音频文件夹路径（支持相对路径或绝对路径）");

    loop {
        print!("\n路径: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let path_str = input.trim();

        if path_str.is_empty() {
            eprintln!("路径不能为空，请重试。");
            continue;
        }

        let path = PathBuf::from(path_str);
        if path.is_dir() {
            return path.canonicalize().context("路径规范化失败，请检查权限");
        }

        if path.exists() {
            eprintln!("输入路径不是文件夹: {}", path.display());
        } else {
            eprintln!("路径不存在: {}", path.display());
        }
    }
}

fn find_ffmpeg_path() -> Result<PathBuf> {
    if let Ok(path) = which("ffmpeg") {
        println!("成功在 PATH 中找到 ffmpeg: {}", path.display());
        return Ok(path);
    }

    let mut candidates = Vec::new();
    if let Ok(cwd) = env::current_dir() {
        candidates.push(cwd.join("resources/ffmpeg"));
    }

    if let Ok(current_exe_path) = env::current_exe() {
        if let Some(project_root) = current_exe_path.ancestors().nth(3) {
            candidates.push(project_root.join("resources/ffmpeg"));
        }
    }

    for candidate in candidates {
        if candidate.is_file() {
            println!(
                "未在 PATH 找到 ffmpeg，使用备用路径: {}",
                candidate.display()
            );
            return Ok(candidate);
        }
    }

    Err(anyhow!(
        "在 PATH 与 resources 目录中均未找到 ffmpeg，可执行文件缺失。"
    ))
}

fn find_ffprobe_path(ffmpeg_path: &Path) -> Option<PathBuf> {
    if let Ok(path) = which("ffprobe") {
        println!("成功在 PATH 中找到 ffprobe: {}", path.display());
        return Some(path);
    }

    let sibling = ffmpeg_path
        .parent()
        .map(|parent| parent.join("ffprobe"))
        .filter(|path| path.is_file());
    if let Some(path) = sibling {
        println!(
            "未在 PATH 找到 ffprobe，使用同目录备用路径: {}",
            path.display()
        );
        return Some(path);
    }

    println!("未找到 ffprobe，将跳过采样率/码率/声道等元数据分析。");
    None
}

fn sanitize_for_terminal(input: &str) -> String {
    input
        .chars()
        .filter(|ch| {
            let c = *ch as u32;
            c == 0x09 || c == 0x20 || (0x21..=0x7e).contains(&c) || c >= 0xa0
        })
        .collect()
}

fn run_analysis(base_folder_path: &Path, config: &AppConfig) -> Result<()> {
    println!("\n--- 开始执行分析流程 ---");
    println!("分析开始时间: {}", Local::now().format("%Y-%m-%d %H:%M:%S"));
    println!(
        "安全模式: {} | 缓存: {} | 命令超时: {}s | 最大并发进程: {} | 评分档案: {}",
        if config.safe_mode { "开启" } else { "关闭" },
        if config.cache_enabled {
            "开启"
        } else {
            "关闭"
        },
        config.command_timeout.as_secs(),
        config.max_ffmpeg_processes,
        config.scoring_profile.as_str()
    );

    let ffmpeg_path = find_ffmpeg_path()?;
    let ffprobe_path = find_ffprobe_path(&ffmpeg_path);

    println!("正在扫描文件夹: {}", base_folder_path.display());

    let audio_files: Vec<PathBuf> = WalkDir::new(base_folder_path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|path| {
            path.extension()
                .and_then(|s| s.to_str())
                .map(|ext| SUPPORTED_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
                .unwrap_or(false)
        })
        .collect();

    if audio_files.is_empty() {
        println!("在指定路径下没有找到支持的音频文件。");
        return Ok(());
    }

    let total_files = audio_files.len();
    println!("扫描完成，找到 {total_files} 个音频文件。开始分析...");

    let cache_path = base_folder_path.join(".audio_quality_cache.json");
    let mut cache_data = if config.cache_enabled {
        AnalysisCache::load(&cache_path).with_context(|| {
            format!("加载增量缓存失败，请检查缓存文件: {}", cache_path.display())
        })?
    } else {
        AnalysisCache::default()
    };
    let cache_snapshot = cache_data.clone();

    let processing_config = ffmpeg::ProcessingConfig {
        ffmpeg_path,
        ffprobe_path,
        command_timeout: config.command_timeout,
        process_limiter: ffmpeg::ProcessLimiter::new(config.max_ffmpeg_processes),
    };

    let bar = ProgressBar::new(total_files as u64);
    let style = ProgressStyle::with_template(
        "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) - {msg}",
    )
    .unwrap_or_else(|_| ProgressStyle::default_bar());
    bar.set_style(style.progress_chars("#>- "));

    let processed_records: Vec<ProcessedRecord> = audio_files
        .into_par_iter()
        .filter_map(|path| {
            let filename = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            bar.set_message(sanitize_for_terminal(&filename));

            let result = process_one_file(
                &path,
                &processing_config,
                &cache_snapshot,
                config.cache_enabled,
            );
            bar.inc(1);

            match result {
                Ok(record) => Some(record),
                Err(e) => {
                    bar.println(format!("处理失败 [{}]: {e}", path.display()));
                    None
                }
            }
        })
        .collect();
    bar.finish_with_message("数据提取完成。");

    let mut results: Vec<FileMetrics> = Vec::with_capacity(processed_records.len());
    let mut cache_hits = 0usize;
    for record in processed_records {
        if record.metrics.cache_hit {
            cache_hits += 1;
        }
        if config.cache_enabled {
            cache_data.upsert(
                &PathBuf::from(&record.metrics.file_path),
                record.fingerprint,
                record.metrics.clone(),
            );
        }
        results.push(record.metrics);
    }
    println!("缓存命中: {cache_hits}/{}", results.len());

    if config.cache_enabled {
        cache_data
            .save(&cache_path, config.safe_mode)
            .with_context(|| format!("保存缓存失败: {}", cache_path.display()))?;
        println!("缓存已更新: {}", cache_path.display());
    }

    println!("正在进行质量评分分析...");
    let scorer = QualityScorer::with_profile(config.scoring_profile);
    let quality_analyses = scorer.analyze_files(&results);

    let report_generator = ReportGenerator::new(config.safe_mode);

    let csv_output_path = base_folder_path.join("audio_quality_report.csv");
    report_generator.generate_csv_report(&quality_analyses, &csv_output_path)?;

    report_generator.display_summary(&quality_analyses);

    let json_output_path = base_folder_path.join("analysis_data.json");
    println!("\n正在保存原始数据到: {}", json_output_path.display());
    let json_content = serde_json::to_string_pretty(&results)?;
    safe_io::atomic_write_string(&json_output_path, &json_content, config.safe_mode)
        .context("无法写入 analysis_data.json 文件")?;
    println!("原始数据保存成功。");

    if config.emit_jsonl {
        let jsonl_path = base_folder_path.join("audio_quality_report.jsonl");
        report_generator.generate_jsonl_report(&quality_analyses, &jsonl_path)?;
    }

    if config.emit_sarif {
        let sarif_path = base_folder_path.join("audio_quality_report.sarif.json");
        report_generator.generate_sarif_report(&quality_analyses, &sarif_path)?;
    }

    println!(
        "\n分析结束时间: {}",
        Local::now().format("%Y-%m-%d %H:%M:%S")
    );
    println!("--- 分析流程完成 ---");
    Ok(())
}

fn process_one_file(
    path: &Path,
    processing_config: &ffmpeg::ProcessingConfig,
    cache_snapshot: &AnalysisCache,
    cache_enabled: bool,
) -> Result<ProcessedRecord> {
    let fingerprint = cache::fingerprint_file(path)?;

    if cache_enabled {
        if let Some(mut metrics) = cache_snapshot.lookup(path, &fingerprint) {
            metrics.processing_time_ms = 0;
            return Ok(ProcessedRecord {
                metrics,
                fingerprint,
            });
        }
    }

    let mut metrics = ffmpeg::process_file(path, processing_config)?;
    metrics.content_sha256 = Some(fingerprint.content_sha256.clone());

    Ok(ProcessedRecord {
        metrics,
        fingerprint,
    })
}

fn build_app_config(cli: &Cli) -> Result<AppConfig> {
    let default_parallel = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let scoring_profile =
        ScoringProfile::from_str(&cli.profile).map_err(|e| anyhow!("profile 参数错误: {e}"))?;

    Ok(AppConfig {
        command_timeout: Duration::from_secs(cli.ffmpeg_timeout_seconds.max(1)),
        max_ffmpeg_processes: cli.max_ffmpeg_processes.unwrap_or(default_parallel).max(1),
        safe_mode: !cli.unsafe_mode,
        cache_enabled: !cli.no_cache,
        emit_jsonl: cli.jsonl,
        emit_sarif: cli.sarif,
        scoring_profile,
    })
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = build_app_config(&cli)?;

    println!("欢迎使用音频质量分析器 (Rust 版)");

    match cli.path {
        Some(path) => {
            if path.is_dir() {
                let absolute_path = path.canonicalize()?;
                run_analysis(&absolute_path, &config)
            } else {
                Err(anyhow!(
                    "命令行提供的路径不是有效文件夹: {}",
                    path.display()
                ))
            }
        }
        None => interactive_mode(&config),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_extensions_are_lowercase() {
        for &ext in &SUPPORTED_EXTENSIONS {
            assert_eq!(ext, ext.to_lowercase());
        }
    }

    #[test]
    fn test_build_app_config_defaults() {
        let cli = Cli::parse_from(["AudioQuality-rs"]);
        let config = build_app_config(&cli).expect("build config");
        assert!(config.safe_mode);
        assert!(config.cache_enabled);
        assert!(config.command_timeout.as_secs() >= 1);
        assert_eq!(config.scoring_profile, ScoringProfile::Pop);
    }
}
