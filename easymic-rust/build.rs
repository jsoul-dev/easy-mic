use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=../src/Resources/app.manifest");
    println!("cargo:rerun-if-changed=../src/Resources/appicon.ico");

    let manifest_path = Path::new("../src/Resources/app.manifest")
        .canonicalize()
        .unwrap_or_else(|_| Path::new("..\\src\\Resources\\app.manifest").to_path_buf());

    // Try finding rc.exe from PATH or Windows Kits
    let mut rc_path = None;
    if let Ok(output) = Command::new("where").arg("rc.exe").output() {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = s.lines().next() {
                rc_path = Some(line.trim().to_string());
            }
        }
    }

    if rc_path.is_none() {
        let kits_root = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
        if kits_root.exists() {
            if let Ok(entries) = std::fs::read_dir(kits_root) {
                for entry in entries.flatten() {
                    let candidate = entry.path().join("x64").join("rc.exe");
                    if candidate.exists() {
                        rc_path = Some(candidate.to_string_lossy().to_string());
                        break;
                    }
                }
            }
        }
    }

    let out_dir = env::var("OUT_DIR").unwrap_or_else(|_| ".".to_string());
    let mut compiled_res = false;

    if let Some(rc_exe) = rc_path {
        let icon_path = Path::new("../src/Resources/appicon.ico")
            .canonicalize()
            .unwrap_or_else(|_| Path::new("..\\src\\Resources\\appicon.ico").to_path_buf());

        let rc_content = format!(
            "1 24 \"{}\"\n1 ICON \"{}\"\n",
            manifest_path.display().to_string().replace('\\', "\\\\"),
            icon_path.display().to_string().replace('\\', "\\\\")
        );
        let rc_file = Path::new(&out_dir).join("app.rc");
        let res_file = Path::new(&out_dir).join("app.res");

        if std::fs::write(&rc_file, rc_content).is_ok() {
            let status = Command::new(rc_exe)
                .arg("/fo")
                .arg(&res_file)
                .arg(&rc_file)
                .status();
            if let Ok(s) = status {
                if s.success() {
                    println!("cargo:rustc-link-arg={}", res_file.display());
                    compiled_res = true;
                }
            }
        }
    }

    if !compiled_res {
        // Fallback to MSVC link.exe manifest embedding
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg=/MANIFESTINPUT:{}",
            manifest_path.display()
        );
    }
}
