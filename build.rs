use std::{
    env,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn command_output(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn main() {
    let revision = command_output(&["rev-parse", "--short", "HEAD"])
        .filter(|revision| !revision.is_empty())
        .unwrap_or_else(|| "no-git-revision".to_owned());
    let dirty = command_output(&["status", "--porcelain"]).is_some_and(|status| !status.is_empty());
    let built_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let profile = env::var("PROFILE").unwrap_or_else(|_| "unknown-profile".to_owned());
    let dirty_suffix = if dirty { "+dirty" } else { "" };
    println!(
        "cargo:rustc-env=ASSETPACK_BUILDER_BUILD_ID={revision}{dirty_suffix} / {profile} / unix-{built_at}"
    );
}
