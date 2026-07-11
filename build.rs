//! Build script — Windows-only: embed the app icon into the `.exe`.
//!
//! On Windows the taskbar / window / Explorer icon comes from an icon *resource*
//! compiled into the executable; there's no equivalent of macOS's `.app` bundle
//! (which gets its icon from `tty7.icns` via `.github/scripts/bundle.sh`). So we
//! compile `assets/favicon.ico` (a multi-res 16–256px ICO) into the binary here.
//!
//! On every other platform this is a no-op.

fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/favicon.ico");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/favicon.ico");
        if let Err(e) = res.compile() {
            // Don't fail the build just because the resource compiler is missing;
            // the app still runs, it just falls back to the default Windows icon.
            println!("cargo:warning=failed to embed Windows icon: {e}");
        }

        stage_conpty();
    }
}

#[cfg(windows)]
fn stage_conpty() {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    const CONPTY_URL: &str = "https://github.com/microsoft/terminal/releases/download/v1.24.10621.0/Microsoft.Windows.Console.ConPTY.1.24.260303001.nupkg";

    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let Some((conpty_rel, open_console_rel)) = conpty_package_paths(&target_arch) else {
        println!("cargo:warning=unsupported Windows arch for ConPTY sidecar: {target_arch}");
        return;
    };

    let out_dir = match std::env::var_os("OUT_DIR").map(PathBuf::from) {
        Some(path) => path,
        None => {
            println!("cargo:warning=OUT_DIR is not set; cannot stage ConPTY sidecars");
            return;
        }
    };
    let Some(target_dir) = out_dir
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
    else {
        println!(
            "cargo:warning=failed to resolve target dir from OUT_DIR={}",
            out_dir.display()
        );
        return;
    };

    let conpty_target = target_dir.join("conpty.dll");
    let open_console_target = target_dir.join("OpenConsole.exe");
    if conpty_target.exists() && open_console_target.exists() {
        return;
    }

    let package_path = out_dir.join("conpty.nupkg.zip");
    let extract_dir = out_dir.join("conpty");

    if !download_conpty_package(CONPTY_URL, &package_path) {
        if !stage_local_conpty(&conpty_target, &open_console_target) {
            println!("cargo:warning=failed to stage ConPTY sidecars");
        }
        return;
    }

    let extract_script = format!(
        "$ProgressPreference = 'SilentlyContinue'; Expand-Archive -LiteralPath {} -DestinationPath {} -Force",
        ps_quote(&package_path.display().to_string()),
        ps_quote(&extract_dir.display().to_string()),
    );
    if !run_powershell(&extract_script, "extract ConPTY package") {
        if !stage_local_conpty(&conpty_target, &open_console_target) {
            println!("cargo:warning=failed to stage ConPTY sidecars");
        }
        return;
    }

    let conpty_source = extract_dir.join(conpty_rel);
    let open_console_source = extract_dir.join(open_console_rel);

    if !copy_sidecars(
        &conpty_source,
        &open_console_source,
        &conpty_target,
        &open_console_target,
    ) && !stage_local_conpty(&conpty_target, &open_console_target)
    {
        println!("cargo:warning=failed to stage ConPTY sidecars");
    }

    fn run_powershell(script: &str, label: &str) -> bool {
        match Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output()
        {
            Ok(output) if output.status.success() => true,
            Ok(output) => {
                println!(
                    "cargo:warning=failed to {label}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                false
            }
            Err(e) => {
                println!("cargo:warning=failed to start PowerShell to {label}: {e}");
                false
            }
        }
    }

    fn download_conpty_package(url: &str, package_path: &Path) -> bool {
        download_with_curl(url, package_path) || download_with_powershell(url, package_path)
    }

    fn download_with_curl(url: &str, package_path: &Path) -> bool {
        match Command::new("curl.exe")
            .args([
                "--location",
                "--fail",
                "--silent",
                "--show-error",
                "--retry",
                "2",
                "--connect-timeout",
                "20",
                "--output",
                &package_path.display().to_string(),
                url,
            ])
            .output()
        {
            Ok(output) if output.status.success() => true,
            Ok(output) => {
                println!(
                    "cargo:warning=failed to download ConPTY package with curl: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                false
            }
            Err(e) => {
                println!("cargo:warning=failed to start curl to download ConPTY package: {e}");
                false
            }
        }
    }

    fn download_with_powershell(url: &str, package_path: &Path) -> bool {
        let download_script = format!(
            "$ProgressPreference = 'SilentlyContinue'; [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -Uri {} -OutFile {}",
            ps_quote(url),
            ps_quote(&package_path.display().to_string()),
        );
        run_powershell(&download_script, "download ConPTY package")
    }
}

#[cfg(windows)]
fn conpty_package_paths(target_arch: &str) -> Option<(&'static str, &'static str)> {
    match target_arch {
        "x86_64" => Some((
            "runtimes/win-x64/native/conpty.dll",
            "build/native/runtimes/x64/OpenConsole.exe",
        )),
        "aarch64" => Some((
            "runtimes/win-arm64/native/conpty.dll",
            "build/native/runtimes/arm64/OpenConsole.exe",
        )),
        _ => None,
    }
}

#[cfg(windows)]
fn copy_sidecars(
    conpty_source: &std::path::Path,
    open_console_source: &std::path::Path,
    conpty_target: &std::path::Path,
    open_console_target: &std::path::Path,
) -> bool {
    if let Err(e) = std::fs::copy(conpty_source, conpty_target) {
        println!(
            "cargo:warning=failed to stage conpty.dll from {}: {e}",
            conpty_source.display()
        );
        return false;
    }
    if let Err(e) = std::fs::copy(open_console_source, open_console_target) {
        println!(
            "cargo:warning=failed to stage OpenConsole.exe from {}: {e}",
            open_console_source.display()
        );
        return false;
    }
    true
}

#[cfg(windows)]
fn ps_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(windows)]
fn stage_local_conpty(
    conpty_target: &std::path::Path,
    open_console_target: &std::path::Path,
) -> bool {
    for dir in local_conpty_dirs() {
        let conpty_source = dir.join("conpty.dll");
        let open_console_source = dir.join("OpenConsole.exe");
        if conpty_source.is_file()
            && open_console_source.is_file()
            && copy_sidecars(
                &conpty_source,
                &open_console_source,
                conpty_target,
                open_console_target,
            )
        {
            println!(
                "cargo:warning=using local ConPTY sidecars from {}",
                dir.display()
            );
            return true;
        }
    }
    false
}

#[cfg(windows)]
fn local_conpty_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = std::env::var_os("TTY7_CONPTY_DIR").map(std::path::PathBuf::from) {
        dirs.push(dir);
    }
    if let Some(local_appdata) = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from) {
        collect_vscode_conpty_dirs(&local_appdata.join("Programs"), &mut dirs);
    }
    dirs
}

#[cfg(windows)]
fn collect_vscode_conpty_dirs(root: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for app_name in ["Microsoft VS Code", "Microsoft VS Code Insiders"] {
        let app_root = root.join(app_name);
        let Ok(entries) = std::fs::read_dir(&app_root) else {
            continue;
        };
        for entry in entries.flatten() {
            let dir = entry.path().join("resources/app/node_modules/node-pty/build/Release/conpty");
            if dir.join("conpty.dll").is_file() && dir.join("OpenConsole.exe").is_file() {
                out.push(dir);
            }
        }
    }
}
