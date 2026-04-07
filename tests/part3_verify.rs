use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_file(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
}

fn unique_output_path() -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("compiler-part3-{now}.s"))
}

fn compile_part3(output_path: &Path) -> String {
    let input = repo_root().join("tests/lab6/part3.sy");
    let status = Command::new(env!("CARGO_BIN_EXE_compiler"))
        .arg(&input)
        .arg(output_path)
        .status()
        .expect("failed to run compiler binary");

    assert!(status.success(), "compiler exited with status {status}");
    read_file(output_path)
}

fn stack_frame_size(asm: &str) -> usize {
    asm.lines()
        .find_map(|line| {
            let trimmed = line.trim();
            let prefix = "addi sp, sp, -";
            trimmed.strip_prefix(prefix)?.trim().parse::<usize>().ok()
        })
        .expect("missing stack frame prologue: addi sp, sp, -N")
}

fn load_store_count(asm: &str) -> usize {
    asm.lines()
        .map(|line| {
            let trimmed = line.trim();
            usize::from(trimmed.starts_with("lw ") || trimmed.starts_with("sw "))
        })
        .sum()
}

#[test]
fn part3_stack_frame_stays_within_target_budget() {
    let output_path = unique_output_path();
    let generated = compile_part3(&output_path);
    let target = read_file(&repo_root().join("tests/lab6/target_part3.s"));

    let generated_frame = stack_frame_size(&generated);
    let target_frame = stack_frame_size(&target);

    let _ = fs::remove_file(&output_path);

    assert!(
        generated_frame <= target_frame,
        "part3 stack frame regressed: generated {generated_frame}, target baseline {target_frame}"
    );
}

#[test]
fn part3_load_store_count_stays_close_to_target_baseline() {
    let output_path = unique_output_path();
    let generated = compile_part3(&output_path);
    let target = read_file(&repo_root().join("tests/lab6/target_part3.s"));

    let generated_count = load_store_count(&generated);
    let target_count = load_store_count(&target);
    let budget = target_count + 20;

    let _ = fs::remove_file(&output_path);

    assert!(
        generated_count <= budget,
        "part3 load/store traffic regressed: generated {generated_count}, budget {budget}, target baseline {target_count}"
    );
}
