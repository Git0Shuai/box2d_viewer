use std::{io::Write, path::PathBuf, process};

use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    task: Task,
}

#[derive(Subcommand)]
enum Task {
    Dist,
}

fn main() {
    let cli = Cli::parse();
    match cli.task {
        Task::Dist => {
            let path = std::env::current_dir().unwrap();
            cxx_export(&path);

            println!("cargo build --package box2d_viewer --release");
            let build_result = process::Command::new("cmd")
                .arg("/C")
                .arg("cargo build --package box2d_viewer --release")
                .status()
                .unwrap();
            if !build_result.success() {
                println!("cargo build FAILED.");
                return;
            }

            std::fs::create_dir_all(path.clone()).unwrap();
            let target_dir = path.join("target");
            std::fs::copy(
                target_dir.join("release/box2d_viewer.exe"),
                path.join("box2d_viewer.exe"),
            )
            .unwrap();

            println!(
                "executable write to: {}",
                path.join("box2d_viewer.exe").to_str().unwrap()
            );
        }
    }
}

fn cxx_export(path: &PathBuf) {
    println!("working directory: {:?}", path);

    println!("cargo build --package box2d_reporter --release");
    let build_result = process::Command::new("cmd")
        .arg("/C")
        .arg("cargo build --package box2d_reporter --release")
        .status()
        .unwrap();
    if !build_result.success() {
        println!("cargo build FAILED.");
        return;
    }

    let export_dir = path.join("cxx_export");
    if export_dir.exists() {
        let prompt = std::format!(
            "delete exist directory: {} ? [Y for delete] ",
            export_dir.to_str().unwrap()
        )
        .into_bytes();
        std::io::stdout().write_all(&prompt).unwrap();
        std::io::stdout().flush().unwrap();
        let mut confirm = String::new();
        std::io::stdin().read_line(&mut confirm).unwrap();
        if confirm.trim_end() == "Y" {
            println!("delte exist directory.");
            std::fs::remove_dir_all(export_dir.clone()).unwrap();
            std::fs::create_dir(export_dir.clone()).unwrap();
        } else {
            println!("export directory already exist.");
            return;
        }
    } else {
        println!("create export directory.");
        std::fs::create_dir_all(export_dir.clone()).unwrap();
    }

    let target_dir = path.join("target");
    let cxxbridge_dir = target_dir.join("cxxbridge");

    println!("copy cpp headers.");
    let include_dir = export_dir.join("include");
    std::fs::create_dir_all(include_dir.clone()).unwrap();
    std::fs::copy(
        cxxbridge_dir.join("rust/cxx.h"),
        include_dir.clone().join("cxx.h"),
    )
    .unwrap();
    std::fs::copy(
        cxxbridge_dir.join("box2d_reporter/src/lib.rs.h"),
        include_dir.clone().join("box2d_reporter.h"),
    )
    .unwrap();

    println!("copy cpp source.");
    let src_dir = export_dir.join("src");
    std::fs::create_dir_all(src_dir.clone()).unwrap();
    std::fs::copy(
        cxxbridge_dir.join("box2d_reporter/src/lib.rs.cc"),
        src_dir.clone().join("lib.rs.cc"),
    )
    .unwrap();
    std::fs::copy(
        path.join("crates/box2d_reporter/src/cxx.cc"),
        src_dir.clone().join("cxx.cc"),
    )
    .unwrap();

    println!("copy library.");
    let lib_dir = export_dir.join("lib");
    std::fs::create_dir_all(lib_dir.clone()).unwrap();
    std::fs::copy(
        target_dir.join("release/box2d_reporter.lib"),
        lib_dir.clone().join("box2d_reporter.lib"),
    )
    .unwrap();

    println!("export-cxx success.");
}
