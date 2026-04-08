use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn unique_output_path() -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("compiler-part4-{now}.s"))
}

fn compile_part4(output_path: &Path) -> String {
    let input = repo_root().join("tests/lab6/part4.sy");
    let status = Command::new(env!("CARGO_BIN_EXE_compiler"))
        .arg(&input)
        .arg(output_path)
        .status()
        .expect("failed to run compiler binary");

    assert!(status.success(), "compiler exited with status {status}");
    fs::read_to_string(output_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", output_path.display()))
}

fn run_rars(asm_path: &Path) -> Output {
    Command::new("timeout")
        .arg("5s")
        .arg("java")
        .arg("-jar")
        .arg(repo_root().join("tests/lab6/rars.jar"))
        .arg(asm_path)
        .arg("a0")
        .output()
        .expect("failed to run RARS")
}

fn parse_a0(stdout: &str) -> Option<String> {
    stdout.lines().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.starts_with("a0") {
            return None;
        }

        trimmed.split_whitespace().nth(1).map(str::to_string)
    })
}

fn section_between<'a>(asm: &'a str, start_label: &str, end_label: &str) -> &'a str {
    let start = asm
        .find(start_label)
        .unwrap_or_else(|| panic!("missing start label {start_label}"));
    let rest = &asm[start..];
    let end = rest
        .find(end_label)
        .unwrap_or_else(|| panic!("missing end label {end_label}"));
    &rest[..end]
}

#[test]
fn part4_generated_program_terminates_with_target_result() {
    let output_path = unique_output_path();
    let _ = compile_part4(&output_path);
    let generated = run_rars(&output_path);
    let target = run_rars(&repo_root().join("tests/lab6/target_part4.s"));
    let _ = fs::remove_file(&output_path);

    let generated_stdout = String::from_utf8_lossy(&generated.stdout);
    let target_stdout = String::from_utf8_lossy(&target.stdout);

    assert_ne!(
        generated.status.code(),
        Some(124),
        "generated part4 program timed out\nstdout:\n{}\nstderr:\n{}",
        generated_stdout,
        String::from_utf8_lossy(&generated.stderr)
    );
    assert!(
        generated_stdout.contains("Program terminated by calling exit"),
        "generated part4 program did not terminate normally:\nstdout:\n{}\nstderr:\n{}",
        generated_stdout,
        String::from_utf8_lossy(&generated.stderr)
    );
    assert!(
        target_stdout.contains("Program terminated by calling exit"),
        "target part4 program did not terminate normally:\nstdout:\n{}\nstderr:\n{}",
        target_stdout,
        String::from_utf8_lossy(&target.stderr)
    );

    let generated_a0 = parse_a0(&generated_stdout).expect("generated output missing a0 line");
    let target_a0 = parse_a0(&target_stdout).expect("target output missing a0 line");
    assert_eq!(generated_a0, target_a0, "generated runtime result diverges from target");
    assert_eq!(generated_a0, "0x0000000e", "generated runtime result should be 14");
}

#[test]
fn part4_generated_program_does_not_timeout() {
    let output_path = unique_output_path();
    let _ = compile_part4(&output_path);
    let output = run_rars(&output_path);
    let _ = fs::remove_file(&output_path);

    assert_ne!(
        output.status.code(),
        Some(124),
        "generated part4 program timed out\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn part4_truthy_branches_target_loop_body_and_if_true() {
    let output_path = unique_output_path();
    let asm = compile_part4(&output_path);
    let _ = fs::remove_file(&output_path);

    let while_cond = section_between(&asm, "whileCond:\n", "whileBody:\n");
    assert!(
        while_cond.contains(", whileBody"),
        "while condition truthy branch should target whileBody:\n{while_cond}"
    );
    assert!(
        while_cond.contains("j whileNext"),
        "while condition false branch should jump to whileNext:\n{while_cond}"
    );

    let while_body = section_between(&asm, "whileBody:\n", "whileNext:\n");
    assert!(
        while_body.contains(", if_true"),
        "if condition truthy branch should target if_true:\n{while_body}"
    );
    assert!(
        while_body.contains("j if_false"),
        "if condition false branch should jump to if_false:\n{while_body}"
    );
}
