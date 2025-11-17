// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use anyhow::{Context, Result};
use futures::{stream, StreamExt};
use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::api::{Api, DynamicObject, ListParams, LogParams, WatchParams};
use kube::config::{KubeConfigOptions, Kubeconfig};
use kube::core::{ApiResource, GroupVersionKind};
use kube::{Client, Config};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::Duration;
use tauri::Emitter;
use tauri::State;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

fn guess_plural(group: &str, kind: &str) -> &'static str {
    match (group, kind) {
        // core
        ("", "Pod") => "pods",
        ("", "Service") => "services",
        ("", "Node") => "nodes",
        ("", "Namespace") => "namespaces",
        ("", "ConfigMap") => "configmaps",
        ("", "Secret") => "secrets",
        ("", "PersistentVolume") => "persistentvolumes",
        ("", "PersistentVolumeClaim") => "persistentvolumeclaims",
        // apps
        ("apps", "Deployment") => "deployments",
        ("apps", "StatefulSet") => "statefulsets",
        ("apps", "DaemonSet") => "daemonsets",
        ("apps", "ReplicaSet") => "replicasets",
        // batch
        ("batch", "Job") => "jobs",
        ("batch", "CronJob") => "cronjobs",
        // networking
        ("networking.k8s.io", "Ingress") => "ingresses",
        // discovery
        ("discovery.k8s.io", "EndpointSlice") => "endpointslices",
        // storage
        ("storage.k8s.io", "StorageClass") => "storageclasses",
        // fallback heuristic: lowercase + 's'
        (_g, k) if !k.is_empty() => {
            // Note: returning &'static str; heuristic not used here
            // because we can't allocate; leave empty to let server error bubble up.
            ""
        }
        _ => "",
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CrdSummary {
    pub name: String,
    pub group: String,
    pub versions: Vec<String>,
    pub scope: String,
    pub plural: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceRef {
    pub group: String,
    pub version: String,
    pub kind: String,
    pub namespace: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContainerStatus {
    pub name: String,
    pub ready: bool,
    pub state: String, // "running", "waiting", "terminated", "init"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>, // reason when terminated
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceSummary {
    pub reference: ResourceRef,
    pub labels: serde_json::Value,
    pub annotations: serde_json::Value,
    pub creation_timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_statuses: Option<Vec<ContainerStatus>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceGroup {
    pub gvk: ResourceRefGvk,
    pub namespaced: bool,
    pub items: Vec<ResourceSummary>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceRefGvk {
    pub group: String,
    pub version: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct K8sOverview {
    pub crds: Vec<CrdSummary>,
    pub resources: Vec<ResourceGroup>,
    pub errors: Vec<String>,
}

#[derive(Default)]
struct K8sSettings {
    // Selected kube context; None means default/infer
    selected_context: Option<String>,
    // Active watcher cancel tokens (support multiple concurrent watchers)
    watcher_cancels: Vec<CancellationToken>,
}

// (no type alias; use State<'_, K8sSettings> directly in handlers)

async fn load_client_with_context(context: Option<String>) -> Result<Client> {
    let config = if let Some(ctx) = context {
        Config::from_kubeconfig(&KubeConfigOptions {
            context: Some(ctx),
            ..Default::default()
        })
        .await
        .context("failed to load kubeconfig for selected context")?
    } else {
        // Infer from environment (in-cluster or local kubeconfig)
        Config::infer()
            .await
            .context("failed to infer kube config (in-cluster or kubeconfig)")?
    };
    Ok(Client::try_from(config)?)
}

fn summarize_dynamic(obj: &DynamicObject, gvk: &GroupVersionKind) -> ResourceSummary {
    let meta = obj.metadata.clone();
    let labels = match meta.labels {
        Some(m) => serde_json::to_value(m).unwrap_or(serde_json::Value::Null),
        None => serde_json::Value::Null,
    };
    let annotations = match meta.annotations {
        Some(m) => serde_json::to_value(m).unwrap_or(serde_json::Value::Null),
        None => serde_json::Value::Null,
    };
    
    // Extract container statuses for Pods
    let container_statuses = if gvk.kind == "Pod" && gvk.group.is_empty() {
        if let Some(status) = obj.data.get("status") {
            let mut statuses = Vec::new();
            
            // Extract init container statuses
            if let Some(init_containers) = status.get("initContainerStatuses").and_then(|v| v.as_array()) {
                for container in init_containers {
                    if let Some(name) = container.get("name").and_then(|v| v.as_str()) {
                        let ready = container.get("ready").and_then(|v| v.as_bool()).unwrap_or(false);
                        let (state, reason) = if container.get("state").is_some() {
                            if container.get("state").and_then(|s| s.get("waiting")).is_some() {
                                ("init", None)
                            } else if container.get("state").and_then(|s| s.get("running")).is_some() {
                                ("running", None)
                            } else if let Some(terminated) = container.get("state").and_then(|s| s.get("terminated")) {
                                let reason = terminated.get("reason").and_then(|v| v.as_str()).map(|s| s.to_string());
                                ("terminated", reason)
                            } else {
                                ("unknown", None)
                            }
                        } else {
                            ("unknown", None)
                        };
                        statuses.push(ContainerStatus {
                            name: name.to_string(),
                            ready,
                            state: state.to_string(),
                            reason,
                        });
                    }
                }
            }
            
            // Extract regular container statuses
            if let Some(containers) = status.get("containerStatuses").and_then(|v| v.as_array()) {
                for container in containers {
                    if let Some(name) = container.get("name").and_then(|v| v.as_str()) {
                        let ready = container.get("ready").and_then(|v| v.as_bool()).unwrap_or(false);
                        let (state, reason) = if container.get("state").is_some() {
                            if container.get("state").and_then(|s| s.get("waiting")).is_some() {
                                ("waiting", None)
                            } else if container.get("state").and_then(|s| s.get("running")).is_some() {
                                ("running", None)
                            } else if let Some(terminated) = container.get("state").and_then(|s| s.get("terminated")) {
                                let reason = terminated.get("reason").and_then(|v| v.as_str()).map(|s| s.to_string());
                                ("terminated", reason)
                            } else {
                                ("unknown", None)
                            }
                        } else {
                            ("unknown", None)
                        };
                        statuses.push(ContainerStatus {
                            name: name.to_string(),
                            ready,
                            state: state.to_string(),
                            reason,
                        });
                    }
                }
            }
            
            if !statuses.is_empty() {
                Some(statuses)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };
    
    ResourceSummary {
        reference: ResourceRef {
            group: gvk.group.clone(),
            version: gvk.version.clone(),
            kind: gvk.kind.clone(),
            namespace: meta.namespace,
            name: meta.name.unwrap_or_default(),
        },
        labels,
        annotations,
        creation_timestamp: meta.creation_timestamp.map(|t| t.0.to_rfc3339()),
        container_statuses,
    }
}

#[tauri::command]
async fn list_k8s_overview(settings: State<'_, Mutex<K8sSettings>>) -> Result<K8sOverview, String> {
    let mut errors: Vec<String> = Vec::new();

    let current_ctx = { settings.lock().unwrap().selected_context.clone() };
    let client = match timeout(
        Duration::from_secs(2),
        load_client_with_context(current_ctx),
    )
    .await
    {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => {
            errors.push(format!("kube client error: {e:#}"));
            return Ok(K8sOverview {
                crds: vec![],
                resources: vec![],
                errors,
            });
        }
        Err(_) => {
            errors.push("kube client timeout".into());
            return Ok(K8sOverview {
                crds: vec![],
                resources: vec![],
                errors,
            });
        }
    };

    // List CRDs with timeout
    let crds_api: Api<CustomResourceDefinition> = Api::all(client.clone());
    let crd_items: Vec<CustomResourceDefinition> = match timeout(
        Duration::from_secs(3),
        crds_api.list(&ListParams::default()),
    )
    .await
    {
        Ok(Ok(list)) => list.items,
        Ok(Err(e)) => {
            errors.push(format!("list CRDs failed: {e:#}"));
            Vec::new()
        }
        Err(_) => {
            errors.push("list CRDs timeout".into());
            Vec::new()
        }
    };

    let mut crds: Vec<CrdSummary> = Vec::with_capacity(crd_items.len());
    let mut dynamic_targets: Vec<(
        GroupVersionKind,
        String, /*plural*/
        bool,   /*namespaced*/
    )> = Vec::new();
    for crd in crd_items {
        let name = crd.metadata.name.clone().unwrap_or_default();
        let spec = crd.spec;
        let group = spec.group.clone();
        let versions = spec
            .versions
            .iter()
            .map(|v| v.name.clone())
            .collect::<Vec<_>>();
        let scope = spec.scope.clone();
        let plural = spec.names.plural.clone();
        crds.push(CrdSummary {
            name,
            group: group.clone(),
            versions: versions.clone(),
            scope: scope.clone(),
            plural: plural.clone(),
        });
        // pick storage or first version for listing
        if let Some(v) = spec
            .versions
            .iter()
            .find(|v| v.storage)
            .or(spec.versions.first())
        {
            let gvk = GroupVersionKind::gvk(&group, &v.name, &spec.names.kind);
            let namespaced = scope.to_lowercase() == "namespaced";
            dynamic_targets.push((gvk, plural, namespaced));
        }
    }

    // Also include well-known built-in resource kinds (core and standard groups)
    let builtin_kinds: Vec<(&str, &str, &str, &str, bool)> = vec![
        // group, version, kind, plural, namespaced
        ("", "v1", "Pod", "pods", true),
        ("", "v1", "Service", "services", true),
        ("", "v1", "ConfigMap", "configmaps", true),
        ("", "v1", "Secret", "secrets", true),
        ("", "v1", "PersistentVolume", "persistentvolumes", false),
        (
            "",
            "v1",
            "PersistentVolumeClaim",
            "persistentvolumeclaims",
            true,
        ),
        ("", "v1", "Namespace", "namespaces", false),
        ("", "v1", "Node", "nodes", false),
        ("apps", "v1", "Deployment", "deployments", true),
        ("apps", "v1", "StatefulSet", "statefulsets", true),
        ("apps", "v1", "DaemonSet", "daemonsets", true),
        ("apps", "v1", "ReplicaSet", "replicasets", true),
        ("batch", "v1", "Job", "jobs", true),
        ("batch", "v1", "CronJob", "cronjobs", true),
        ("networking.k8s.io", "v1", "Ingress", "ingresses", true),
        (
            "discovery.k8s.io",
            "v1",
            "EndpointSlice",
            "endpointslices",
            true,
        ),
        (
            "storage.k8s.io",
            "v1",
            "StorageClass",
            "storageclasses",
            false,
        ),
    ];
    for (group, version, kind, plural, namespaced) in builtin_kinds {
        let gvk = GroupVersionKind::gvk(group, version, kind);
        dynamic_targets.push((gvk, plural.to_string(), namespaced));
    }

    // For each CRD kind, list instances with limited concurrency and timeouts
    let concurrency = 8usize;
    let groups_stream = stream::iter(dynamic_targets.into_iter())
        .map(|(gvk, plural, namespaced)| {
            let client = client.clone();
            async move {
                let mut ar = ApiResource::from_gvk(&gvk);
                ar.plural = plural.clone();
                let api: Api<DynamicObject> = Api::all_with(client, &ar);
                let list_res = timeout(
                    Duration::from_secs(4),
                    api.list(&ListParams::default().limit(250)),
                )
                .await;
                let mut errors = Vec::new();
                let mut items = Vec::new();
                match list_res {
                    Ok(Ok(list)) => {
                        for obj in list.items.iter() {
                            items.push(summarize_dynamic(obj, &gvk));
                        }
                    }
                    Ok(Err(e)) => {
                        errors.push(format!(
                            "list {}.{} {} failed: {e:#}",
                            gvk.kind,
                            gvk.version,
                            gvk.group.clone()
                        ));
                    }
                    Err(_) => {
                        errors.push(format!(
                            "list {}.{} {} timeout",
                            gvk.kind,
                            gvk.version,
                            gvk.group.clone()
                        ));
                    }
                }
                ResourceGroup {
                    gvk: ResourceRefGvk {
                        group: gvk.group.clone(),
                        version: gvk.version.clone(),
                        kind: gvk.kind.clone(),
                    },
                    namespaced,
                    items,
                    errors,
                }
            }
        })
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await;

    Ok(K8sOverview {
        crds,
        resources: groups_stream,
        errors,
    })
}

#[derive(Debug, Clone, Deserialize)]
struct DescribeArgs {
    group: String,
    version: String,
    kind: String,
    namespace: Option<String>,
    name: String,
    timeout_secs: Option<u64>,
}

#[tauri::command]
async fn describe_resource(args: DescribeArgs) -> Result<serde_json::Value, String> {
    let client = timeout(Duration::from_secs(2), load_client_with_context(None))
        .await
        .map_err(|_| "kube client timeout".to_string())?
        .map_err(|e| format!("{e:#}"))?;
    let gvk = GroupVersionKind::gvk(&args.group, &args.version, &args.kind);
    let mut ar = ApiResource::from_gvk(&gvk);
    if ar.plural.is_empty() {
        ar.plural = guess_plural(&args.group, &args.kind).to_string();
    }
    let api: Api<DynamicObject> = if let Some(ns) = &args.namespace {
        Api::namespaced_with(client, ns, &ar)
    } else {
        // cluster-scoped
        Api::all_with(client, &ar)
    };
    let dur = Duration::from_secs(args.timeout_secs.unwrap_or(3));
    let obj = timeout(dur, api.get(&args.name))
        .await
        .map_err(|_| "describe timeout".to_string())?
        .map_err(|e| format!("{e:#}"))?;
    Ok(serde_json::to_value(&obj).unwrap_or(serde_json::json!({ "error": "serialize failed" })))
}

#[derive(Debug, Clone, Deserialize)]
struct PodLogArgs {
    namespace: String,
    name: String,
    container: Option<String>,
    tail_lines: Option<i64>,
    previous: Option<bool>,
    timeout_secs: Option<u64>,
}

#[tauri::command]
async fn pod_logs(args: PodLogArgs) -> Result<String, String> {
    let client = timeout(Duration::from_secs(2), load_client_with_context(None))
        .await
        .map_err(|_| "kube client timeout".to_string())?
        .map_err(|e| format!("{e:#}"))?;
    let api: Api<Pod> = Api::namespaced(client, &args.namespace);
    let mut lp = LogParams::default();
    lp.container = args.container.clone();
    lp.tail_lines = args.tail_lines;
    lp.previous = args.previous.unwrap_or(false);
    let dur = Duration::from_secs(args.timeout_secs.unwrap_or(5));
    let logs = timeout(dur, api.logs(&args.name, &lp))
        .await
        .map_err(|_| "logs timeout".to_string())?
        .map_err(|e| format!("{e:#}"))?;
    Ok(logs)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Mutex::new(K8sSettings::default()))
        .invoke_handler(tauri::generate_handler![
            greet,
            list_k8s_overview,
            describe_resource,
            pod_logs,
            list_contexts,
            set_context,
            start_watch,
            stop_all_watches,
            copy_text
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn copy_text(text: String) -> Result<(), String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("clipboard error: {e}"))?;
    cb.set_text(text)
        .map_err(|e| format!("clipboard set failed: {e}"))
}
#[tauri::command]
fn list_contexts() -> Result<serde_json::Value, String> {
    // Try read default kubeconfig and extract contexts
    let cfg = Kubeconfig::read().map_err(|e| format!("read kubeconfig failed: {e:#}"))?;
    let contexts: Vec<String> = cfg.contexts.iter().map(|c| c.name.clone()).collect();
    let current = cfg.current_context.unwrap_or_default();
    Ok(serde_json::json!({
        "contexts": contexts,
        "current": current
    }))
}

#[derive(Debug, Clone, Deserialize)]
struct SetContextArgs {
    name: Option<String>,
}

#[tauri::command]
fn set_context(
    settings: State<'_, Mutex<K8sSettings>>,
    args: SetContextArgs,
) -> Result<(), String> {
    let mut s = settings
        .lock()
        .map_err(|_| "settings lock poisoned".to_string())?;
    s.selected_context = args.name;
    // stop all watchers on context change
    for tok in s.watcher_cancels.drain(..) {
        tok.cancel();
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
struct StartWatchArgs {
    group: String,
    version: String,
    kind: String,
    namespace: Option<String>,
}

#[tauri::command]
async fn start_watch(
    app: tauri::AppHandle,
    settings: State<'_, Mutex<K8sSettings>>,
    args: StartWatchArgs,
) -> Result<(), String> {
    let token = CancellationToken::new();
    settings
        .lock()
        .map_err(|_| "settings lock poisoned".to_string())?
        .watcher_cancels
        .push(token.clone());
    let current_ctx = {
        settings
            .lock()
            .map_err(|_| "settings lock poisoned".to_string())?
            .selected_context
            .clone()
    };
    let app2 = app.clone();
    let group = args.group.clone();
    let version = args.version.clone();
    let kind = args.kind.clone();
    let ns = args.namespace.clone();
    tokio::spawn(async move {
        let client = match load_client_with_context(current_ctx).await {
            Ok(c) => c,
            Err(_) => return,
        };
        let gvk = GroupVersionKind::gvk(&group, &version, &kind);
        let mut ar = ApiResource::from_gvk(&gvk);
        if ar.plural.is_empty() {
            ar.plural = guess_plural(&group, &kind).to_string();
        }
        let api: Api<DynamicObject> = match &ns {
            Some(n) => Api::namespaced_with(client, n, &ar),
            None => Api::all_with(client, &ar),
        };
        use futures::StreamExt;
        let mut rv: Option<String> = None;
        'outer: loop {
            if token.is_cancelled() {
                break;
            }
            if rv.is_none() {
                // initial list for rv and sync
                match api.list(&ListParams::default()).await {
                    Ok(list) => {
                        rv = list.metadata.resource_version.clone();
                        for obj in list.items {
                            let _ = app2.emit(
                                "k8s:resource_event",
                                serde_json::json!({
                                    "op":"ADDED",
                                    "gvk": { "group": group, "version": version, "kind": kind },
                                    "item": obj
                                }),
                            );
                        }
                    }
                    Err(_) => {
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        continue;
                    }
                }
            }
            let params = WatchParams::default().timeout(30);
            let mut stream = match api.watch(&params, &rv.clone().unwrap_or_default()).await {
                Ok(s) => s.boxed(),
                Err(_) => {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    rv = None;
                    continue;
                }
            };
            while let Some(ev) = stream.next().await {
                if token.is_cancelled() {
                    break 'outer;
                }
                match ev {
                    Ok(kube::api::WatchEvent::Added(obj)) => {
                        let _ = app2.emit("k8s:resource_event", serde_json::json!({
                            "op":"ADDED","gvk": { "group": group, "version": version, "kind": kind }, "item": obj
                        }));
                    }
                    Ok(kube::api::WatchEvent::Modified(obj)) => {
                        let _ = app2.emit("k8s:resource_event", serde_json::json!({
                            "op":"MODIFIED","gvk": { "group": group, "version": version, "kind": kind }, "item": obj
                        }));
                    }
                    Ok(kube::api::WatchEvent::Deleted(obj)) => {
                        let _ = app2.emit("k8s:resource_event", serde_json::json!({
                            "op":"DELETED","gvk": { "group": group, "version": version, "kind": kind }, "item": obj
                        }));
                    }
                    Ok(kube::api::WatchEvent::Bookmark(bm)) => {
                        rv = Some(bm.metadata.resource_version);
                    }
                    Ok(kube::api::WatchEvent::Error(_)) | Err(_) => {
                        rv = None;
                        break;
                    }
                }
            }
        }
    });
    Ok(())
}

#[tauri::command]
fn stop_all_watches(settings: State<'_, Mutex<K8sSettings>>) -> Result<(), String> {
    let mut s = settings
        .lock()
        .map_err(|_| "settings lock poisoned".to_string())?;
    for tok in s.watcher_cancels.drain(..) {
        tok.cancel();
    }
    Ok(())
}
