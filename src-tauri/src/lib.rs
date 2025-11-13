// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use std::process::Command;
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn get_k8s_resources(kind: &str, namespace: Option<String>) -> Result<String, String> {
    // cluster-scoped kinds: do not pass -A or -n
    let is_cluster_scoped = matches!(
        kind.to_lowercase().as_str(),
        "pv" | "persistentvolume"
            | "sc" | "storageclass"
            | "crd" | "customresourcedefinition"
            | "node" | "namespace" | "ns"
            | "clusterrole" | "clusterrolebinding"
    );

    let mut cmd = Command::new("kubectl");
    cmd.arg("get").arg(kind);

    if !is_cluster_scoped {
        match namespace.as_deref() {
            Some(ns) if !ns.is_empty() && ns.to_lowercase() != "all" => {
                cmd.arg("-n").arg(ns);
            }
            _ => {
                cmd.arg("-A");
            }
        }
    }

    cmd.arg("-o").arg("json");

    let output = cmd.output().map_err(|e| format!("Failed to spawn kubectl: {}", e))?;

    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|e| format!("Invalid UTF-8 from kubectl: {}", e))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(format!("kubectl error: {}", stderr))
    }
}

#[tauri::command]
fn get_k8s_yaml(kind: &str, name: &str, namespace: Option<String>) -> Result<String, String> {
    // cluster-scoped kinds: do not pass -n
    let is_cluster_scoped = matches!(
        kind.to_lowercase().as_str(),
        "pv" | "persistentvolume"
            | "sc" | "storageclass"
            | "crd" | "customresourcedefinition"
            | "node" | "namespace" | "ns"
            | "clusterrole" | "clusterrolebinding"
    );

    let mut cmd = Command::new("kubectl");
    cmd.arg("get").arg(kind).arg(name);
    if !is_cluster_scoped {
        if let Some(ns) = namespace {
            if !ns.is_empty() && ns.to_lowercase() != "all" {
                cmd.arg("-n").arg(ns);
            }
        }
    }
    cmd.arg("-o").arg("yaml");
    let output = cmd.output().map_err(|e| format!("Failed to spawn kubectl: {}", e))?;
    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|e| format!("Invalid UTF-8 from kubectl: {}", e))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(format!("kubectl error: {}", stderr))
    }
}

#[tauri::command]
fn get_pod_logs(pod: &str, namespace: String, tail: Option<u32>) -> Result<String, String> {
    if namespace.is_empty() || namespace.to_lowercase() == "all" {
        return Err("namespace is required for pod logs".to_string());
    }
    let mut cmd = Command::new("kubectl");
    cmd.arg("logs")
        .arg(pod)
        .arg("-n")
        .arg(namespace)
        .arg("--all-containers=true");
    if let Some(t) = tail {
        cmd.arg("--tail").arg(t.to_string());
    } else {
        cmd.arg("--tail").arg("500");
    }
    let output = cmd.output().map_err(|e| format!("Failed to spawn kubectl: {}", e))?;
    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|e| format!("Invalid UTF-8 from kubectl: {}", e))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(format!("kubectl error: {}", stderr))
    }
}

#[tauri::command]
fn get_k8s_object(kind: &str, name: &str, namespace: Option<String>, format: &str) -> Result<String, String> {
    let fmt = match format.to_lowercase().as_str() {
        "json" | "yaml" => format.to_lowercase(),
        _ => return Err("unsupported format; use json or yaml".to_string()),
    };
    let is_cluster_scoped = matches!(
        kind.to_lowercase().as_str(),
        "pv" | "persistentvolume"
            | "sc" | "storageclass"
            | "crd" | "customresourcedefinition"
            | "node" | "namespace" | "ns"
            | "clusterrole" | "clusterrolebinding"
    );
    let mut cmd = Command::new("kubectl");
    cmd.arg("get").arg(kind).arg(name);
    if !is_cluster_scoped {
        if let Some(ns) = namespace {
            if !ns.is_empty() && ns.to_lowercase() != "all" {
                cmd.arg("-n").arg(ns);
            }
        }
    }
    cmd.arg("-o").arg(fmt);
    let output = cmd.output().map_err(|e| format!("Failed to spawn kubectl: {}", e))?;
    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|e| format!("Invalid UTF-8 from kubectl: {}", e))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(format!("kubectl error: {}", stderr))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            get_k8s_resources,
            get_k8s_yaml,
            get_pod_logs,
            get_k8s_object
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
