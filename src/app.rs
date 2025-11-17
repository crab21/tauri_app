use leptos::ev::{KeyboardEvent, MouseEvent};
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[component]
fn DescribeContent(
    detail: ReadSignal<Option<serde_json::Value>>,
    set_detail: WriteSignal<Option<serde_json::Value>>,
) -> impl IntoView {
    let (search_query, set_search_query) = signal::<String>(String::new());
    let search_input_ref = NodeRef::<leptos::html::Input>::new();
    
    let yaml_text = move || match detail.get() {
        Some(v) => {
            let json_pretty = serde_json::to_value(v).unwrap_or(serde_json::json!({}));
            match serde_yaml::to_string(&json_pretty) {
                Ok(s) => s,
                Err(_) => serde_json::to_string_pretty(&json_pretty).unwrap_or_default(),
            }
        }
        None => String::new(),
    };

    // Highlight text with search query - returns HTML string
    let highlighted_text_html = move || {
        let text = yaml_text();
        let query = search_query.get();
        if query.is_empty() {
            return text;
        }
        
        let query_lower = query.to_lowercase();
        let text_lower = text.to_lowercase();
        let mut result = String::new();
        let mut last_end = 0;
        let mut search_start = 0;
        
        // Find all matches and build HTML
        while let Some(pos) = text_lower[search_start..].find(&query_lower) {
            let actual_pos = search_start + pos;
            let end_pos = actual_pos + query.len();
            
            // Add text before match (escape HTML)
            if actual_pos > last_end {
                let before = &text[last_end..actual_pos];
                result.push_str(&before.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;"));
            }
            
            // Add highlighted match
            let matched = &text[actual_pos..end_pos];
            let escaped = matched.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
            result.push_str(&format!("<mark style=\"background-color:#ffeb3b;color:#000;padding:0 2px;\">{}</mark>", escaped));
            
            last_end = end_pos;
            search_start = actual_pos + 1;
        }
        
        // Add remaining text
        if last_end < text.len() {
            let remaining = &text[last_end..];
            result.push_str(&remaining.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;"));
        }
        
        if result.is_empty() {
            text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
        } else {
            result
        }
    };

    let do_copy = move |_| {
        let content = yaml_text();
        // Use backend to place text into clipboard (reliable across platforms)
        let payload =
            serde_wasm_bindgen::to_value(&serde_json::json!({ "text": content })).unwrap();
        spawn_local(async move {
            let _ = invoke("copy_text", payload).await;
        });
    };

    let handle_keydown = move |e: KeyboardEvent| {
        // Check for Command+F (Mac) or Ctrl+F (Windows/Linux)
        let meta = e.ctrl_key() || e.meta_key();
        if meta && e.key_code() == 70 {
            e.prevent_default();
            if let Some(input) = search_input_ref.get() {
                let _ = input.focus();
                let _ = input.select();
            }
        }
    };

    view! {
        <div style="display:flex;flex-direction:column;height:100%;min-height:0;" on:keydown=handle_keydown>
            <div style="display:flex;align-items:center;justify-content:space-between;padding:10px 12px;border-bottom:1px solid #eee;flex-shrink:0;">
                <h3 style="margin:0;font-size:14px;color:#333">"Describe"</h3>
                <div style="display:flex;gap:8px;">
                    <button on:click=do_copy>"Copy"</button>
                    <button on:click=move |_| set_detail.set(None)>"Close"</button>
                </div>
            </div>
            <div style="padding:8px 12px;border-bottom:1px solid #eee;background:#f5f5f5;flex-shrink:0;">
                <input
                    node_ref=search_input_ref
                    type="text"
                    placeholder="Search... (Cmd+F / Ctrl+F)"
                    value=move || search_query.get()
                    on:input=move |e| set_search_query.set(event_target_value(&e))
                    style="width:100%;padding:6px 10px;border:1px solid #ddd;border-radius:4px;font-size:13px;"
                />
            </div>
            <div style="padding:0;flex:1;overflow-y:auto;overflow-x:auto;background:#0b1021;min-height:0;">
                <pre style="margin:0;padding:12px;white-space:pre;overflow:visible;color:#d6e1ff;text-align:left;font-family:ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, \"Liberation Mono\", \"Courier New\", monospace;" inner_html=highlighted_text_html></pre>
            </div>
        </div>
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"])]
    async fn listen(event: &str, handler: &js_sys::Function) -> JsValue;
}

#[derive(Clone, Debug)]
struct MenuItem {
    group: &'static str,
    kind: &'static str,
    title: &'static str,
}

#[derive(Clone, Debug)]
struct MenuGroup {
    group_title: &'static str,
    items: Vec<MenuItem>,
}

fn static_menu() -> Vec<MenuGroup> {
    vec![
        MenuGroup {
            group_title: "Workloads",
            items: vec![
                MenuItem {
                    group: "",
                    kind: "Pod",
                    title: "Pods",
                },
                MenuItem {
                    group: "apps",
                    kind: "Deployment",
                    title: "Deployments",
                },
                MenuItem {
                    group: "apps",
                    kind: "StatefulSet",
                    title: "StatefulSets",
                },
                MenuItem {
                    group: "apps",
                    kind: "DaemonSet",
                    title: "DaemonSets",
                },
                MenuItem {
                    group: "apps",
                    kind: "ReplicaSet",
                    title: "ReplicaSets",
                },
                MenuItem {
                    group: "batch",
                    kind: "Job",
                    title: "Jobs",
                },
                MenuItem {
                    group: "batch",
                    kind: "CronJob",
                    title: "CronJobs",
                },
            ],
        },
        MenuGroup {
            group_title: "Networking",
            items: vec![
                MenuItem {
                    group: "",
                    kind: "Service",
                    title: "Services",
                },
                MenuItem {
                    group: "networking.k8s.io",
                    kind: "Ingress",
                    title: "Ingresses",
                },
                MenuItem {
                    group: "discovery.k8s.io",
                    kind: "EndpointSlice",
                    title: "EndpointSlices",
                },
            ],
        },
        MenuGroup {
            group_title: "Storage",
            items: vec![
                MenuItem {
                    group: "",
                    kind: "PersistentVolume",
                    title: "PVs",
                },
                MenuItem {
                    group: "",
                    kind: "PersistentVolumeClaim",
                    title: "PVCs",
                },
                MenuItem {
                    group: "storage.k8s.io",
                    kind: "StorageClass",
                    title: "StorageClasses",
                },
            ],
        },
        MenuGroup {
            group_title: "Cluster",
            items: vec![
                MenuItem {
                    group: "",
                    kind: "Node",
                    title: "Nodes",
                },
                MenuItem {
                    group: "",
                    kind: "Namespace",
                    title: "Namespaces",
                },
                MenuItem {
                    group: "",
                    kind: "ConfigMap",
                    title: "ConfigMaps",
                },
                MenuItem {
                    group: "",
                    kind: "Secret",
                    title: "Secrets",
                },
            ],
        },
    ]
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CrdSummary {
    name: String,
    group: String,
    versions: Vec<String>,
    scope: String,
    plural: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ResourceRefGvk {
    group: String,
    version: String,
    kind: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ResourceRef {
    group: String,
    version: String,
    kind: String,
    namespace: Option<String>,
    name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ContainerStatus {
    name: String,
    ready: bool,
    state: String,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ServicePort {
    port: i64,
    target_port: Option<String>,
    protocol: Option<String>,
    name: Option<String>,
    node_port: Option<i64>, // NodePort for NodePort type services
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct EndpointSlicePort {
    port: Option<i64>,
    protocol: Option<String>,
    name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct EndpointSliceEndpoint {
    addresses: Vec<String>,
    ready: Option<bool>,
    serving: Option<bool>,
    terminating: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ResourceSummary {
    reference: ResourceRef,
    labels: serde_json::Value,
    annotations: serde_json::Value,
    creation_timestamp: Option<String>,
    #[serde(default)]
    container_statuses: Option<Vec<ContainerStatus>>,
    #[serde(default)]
    service_ports: Option<Vec<ServicePort>>,
    #[serde(default)]
    service_type: Option<String>,
    // PV fields
    #[serde(default)]
    pv_capacity: Option<String>,
    #[serde(default)]
    pv_access_modes: Option<Vec<String>>,
    #[serde(default)]
    pv_reclaim_policy: Option<String>,
    #[serde(default)]
    pv_status: Option<String>,
    #[serde(default)]
    pv_claim: Option<String>,
    #[serde(default)]
    pv_storage_class: Option<String>,
    #[serde(default)]
    pv_volume_attributes_class: Option<String>,
    #[serde(default)]
    pv_reason: Option<String>,
    // PVC fields
    #[serde(default)]
    pvc_status: Option<String>,
    #[serde(default)]
    pvc_volume: Option<String>,
    #[serde(default)]
    pvc_capacity: Option<String>,
    #[serde(default)]
    pvc_access_modes: Option<Vec<String>>,
    #[serde(default)]
    pvc_storage_class: Option<String>,
    #[serde(default)]
    pvc_volume_attributes_class: Option<String>,
    // EndpointSlice fields
    #[serde(default)]
    endpointslice_ports: Option<Vec<EndpointSlicePort>>,
    #[serde(default)]
    endpointslice_endpoints: Option<Vec<EndpointSliceEndpoint>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ResourceGroup {
    gvk: ResourceRefGvk,
    namespaced: bool,
    items: Vec<ResourceSummary>,
    errors: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct K8sOverview {
    crds: Vec<CrdSummary>,
    resources: Vec<ResourceGroup>,
    errors: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct EmptyArgs {}

#[component]
pub fn App() -> impl IntoView {
    let (loading, set_loading) = signal(false);
    let (overview, set_overview) = signal::<Option<K8sOverview>>(None);
    let (selected_key, set_selected_key) = signal::<Option<String>>(None);
    let (selected_ns, set_selected_ns) = signal::<Option<String>>(None);
    let (contexts, set_contexts) = signal::<Vec<String>>(Vec::new());
    let (current_ctx, set_current_ctx) = signal::<Option<String>>(None);
    let (detail, set_detail) = signal::<Option<serde_json::Value>>(None);
    let (logs, set_logs) = signal::<Option<String>>(None);
    let (logs_container, set_logs_container) = signal::<Option<String>>(None);
    // store last request state to prevent overlap
    // request state to avoid overlapping loads and drop stale results
    let (is_fetching, set_is_fetching) = signal(false);
    let (last_req_id, set_last_req_id) = signal(0i32);

    let load_overview = move || {
        if is_fetching.get() {
            return;
        }
        set_is_fetching.set(true);
        set_loading.set(true);
        set_detail.set(None);
        set_logs.set(None);
        let my_id = last_req_id.get_untracked() + 1;
        set_last_req_id.set(my_id);
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&EmptyArgs {}).unwrap();
            let result = invoke("list_k8s_overview", args).await;
            // If call failed, invoke returns JsValue::undefined/null; we try to deserialize
            let parsed: Result<K8sOverview, _> = serde_wasm_bindgen::from_value(result.clone());
            // ignore stale response
            if my_id != last_req_id.get_untracked() {
                set_loading.set(false);
                set_is_fetching.set(false);
                return;
            }
            match parsed {
                Ok(v) => {
                    // preserve current selection; only set default if none
                    let current_sel = selected_key.get_untracked();
                    if current_sel.is_none() {
                        // default select Nodes if present, else first group
                        let node_key = v
                            .resources
                            .iter()
                            .find(|g| g.gvk.kind == "Node" && g.gvk.group.is_empty())
                            .map(|g| format!("{}.{}/{}", g.gvk.kind, g.gvk.version, g.gvk.group));
                        let fallback = v
                            .resources
                            .get(0)
                            .map(|g| format!("{}.{}/{}", g.gvk.kind, g.gvk.version, g.gvk.group));
                        set_selected_key.set(node_key.or(fallback));
                    }
                    set_overview.set(Some(v));
                }
                Err(_) => {
                    // keep previous overview; just stop loading
                }
            }
            set_loading.set(false);
            set_is_fetching.set(false);
        });
    };

    // auto-load once
    let (started, set_started) = signal(false);
    Effect::new(move |_| {
        if !started.get() {
            set_started.set(true);
            load_overview();
            // load kube contexts
            spawn_local(async move {
                let val = invoke("list_contexts", JsValue::NULL).await;
                let parsed: Result<serde_json::Value, _> = serde_wasm_bindgen::from_value(val);
                if let Ok(v) = parsed {
                    let list = v
                        .get("contexts")
                        .and_then(|x| x.as_array())
                        .cloned()
                        .unwrap_or_default();
                    let mut items = Vec::new();
                    for c in list {
                        if let Some(s) = c.as_str() {
                            items.push(s.to_string());
                        }
                    }
                    set_contexts.set(items);
                    let cur = v
                        .get("current")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string());
                    set_current_ctx.set(cur);
                }
            });
            // subscribe to backend watch events and do incremental updates
            spawn_local(async move {
                let handler = Closure::wrap(Box::new(move |payload: JsValue| {
                    let parsed_root: Result<serde_json::Value, _> =
                        serde_wasm_bindgen::from_value(payload);
                    if parsed_root.is_err() {
                        return;
                    }
                    let root = parsed_root.unwrap();
                    let json = root.get("payload").cloned().unwrap_or(root);
                    let op = json.get("op").and_then(|x| x.as_str()).unwrap_or("");
                    let gvk = json.get("gvk");
                    let item = json.get("item");
                    if gvk.is_none() || item.is_none() {
                        return;
                    }
                    let gvk = gvk.unwrap();
                    let item = item.unwrap();
                    let group = gvk
                        .get("group")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let version = gvk
                        .get("version")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let kind = gvk
                        .get("kind")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let meta = item
                        .get("metadata")
                        .cloned()
                        .unwrap_or(serde_json::json!({}));
                    let name = meta
                        .get("name")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let namespace = meta
                        .get("namespace")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string());
                    // build ResourceSummary from json
                    let labels = meta
                        .get("labels")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);
                    let annotations = meta
                        .get("annotations")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);
                    let creation_timestamp = meta
                        .get("creationTimestamp")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string());
                    
                    // Extract container statuses for Pods
                    let container_statuses = if kind == "Pod" && group.is_empty() {
                        if let Some(status) = item.get("status") {
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
                    
                    // Extract service ports and type for Services
                    let (service_ports, service_type) = if kind == "Service" && group.is_empty() {
                        let mut ports = Vec::new();
                        let mut svc_type: Option<String> = None;
                        
                        if let Some(spec) = item.get("spec") {
                            // Extract service type
                            if let Some(typ) = spec.get("type").and_then(|v| v.as_str()) {
                                svc_type = Some(typ.to_string());
                            }
                            
                            // Extract ports
                            if let Some(ports_array) = spec.get("ports").and_then(|v| v.as_array()) {
                                for port_obj in ports_array {
                                    let port = port_obj.get("port").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let target_port = port_obj.get("targetPort").and_then(|v| {
                                        if let Some(s) = v.as_str() {
                                            Some(s.to_string())
                                        } else if let Some(n) = v.as_i64() {
                                            Some(n.to_string())
                                        } else {
                                            None
                                        }
                                    });
                                    let protocol = port_obj.get("protocol").and_then(|v| v.as_str()).map(|s| s.to_string());
                                    let name = port_obj.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
                                    let node_port = port_obj.get("nodePort").and_then(|v| v.as_i64());
                                    ports.push(ServicePort {
                                        port,
                                        target_port,
                                        protocol,
                                        name,
                                        node_port,
                                    });
                                }
                            }
                        }
                        
                        if !ports.is_empty() {
                            (Some(ports), svc_type)
                        } else {
                            (None, svc_type)
                        }
                    } else {
                        (None, None)
                    };
                    
                    // Extract PV fields
                    let (pv_capacity, pv_access_modes, pv_reclaim_policy, pv_status, pv_claim, pv_storage_class, pv_volume_attributes_class, pv_reason) = if kind == "PersistentVolume" && group.is_empty() {
                        let mut capacity: Option<String> = None;
                        let mut access_modes: Option<Vec<String>> = None;
                        let mut reclaim_policy: Option<String> = None;
                        let mut status: Option<String> = None;
                        let mut claim: Option<String> = None;
                        let mut storage_class: Option<String> = None;
                        let mut volume_attributes_class: Option<String> = None;
                        let mut reason: Option<String> = None;
                        
                        if let Some(spec) = item.get("spec") {
                            // Capacity
                            if let Some(cap) = spec.get("capacity").and_then(|c| c.get("storage")).and_then(|v| v.as_str()) {
                                capacity = Some(cap.to_string());
                            }
                            // Access modes
                            if let Some(modes) = spec.get("accessModes").and_then(|v| v.as_array()) {
                                access_modes = Some(modes.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect());
                            }
                            // Reclaim policy
                            if let Some(rp) = spec.get("persistentVolumeReclaimPolicy").and_then(|v| v.as_str()) {
                                reclaim_policy = Some(rp.to_string());
                            }
                            // Storage class
                            if let Some(sc) = spec.get("storageClassName").and_then(|v| v.as_str()) {
                                storage_class = Some(sc.to_string());
                            }
                            // Volume attributes class
                            if let Some(vac) = spec.get("volumeAttributesClassName").and_then(|v| v.as_str()) {
                                volume_attributes_class = Some(vac.to_string());
                            }
                        }
                        
                        if let Some(status_obj) = item.get("status") {
                            // Status phase
                            if let Some(phase) = status_obj.get("phase").and_then(|v| v.as_str()) {
                                status = Some(phase.to_string());
                            }
                            // Claim reference
                            if let Some(claim_ref) = status_obj.get("claimRef") {
                                if let Some(ns) = claim_ref.get("namespace").and_then(|v| v.as_str()) {
                                    if let Some(name) = claim_ref.get("name").and_then(|v| v.as_str()) {
                                        claim = Some(format!("{}/{}", ns, name));
                                    }
                                }
                            }
                            // Reason
                            if let Some(r) = status_obj.get("reason").and_then(|v| v.as_str()) {
                                reason = Some(r.to_string());
                            }
                        }
                        
                        (capacity, access_modes, reclaim_policy, status, claim, storage_class, volume_attributes_class, reason)
                    } else {
                        (None, None, None, None, None, None, None, None)
                    };
                    
                    // Extract PVC fields
                    let (pvc_status, pvc_volume, pvc_capacity, pvc_access_modes, pvc_storage_class, pvc_volume_attributes_class) = if kind == "PersistentVolumeClaim" && group.is_empty() {
                        let mut status: Option<String> = None;
                        let mut volume: Option<String> = None;
                        let mut capacity: Option<String> = None;
                        let mut access_modes: Option<Vec<String>> = None;
                        let mut storage_class: Option<String> = None;
                        let mut volume_attributes_class: Option<String> = None;
                        
                        if let Some(spec) = item.get("spec") {
                            // Access modes
                            if let Some(modes) = spec.get("accessModes").and_then(|v| v.as_array()) {
                                access_modes = Some(modes.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect());
                            }
                            // Storage class
                            if let Some(sc) = spec.get("storageClassName").and_then(|v| v.as_str()) {
                                storage_class = Some(sc.to_string());
                            }
                            // Volume attributes class
                            if let Some(vac) = spec.get("volumeAttributesClassName").and_then(|v| v.as_str()) {
                                volume_attributes_class = Some(vac.to_string());
                            }
                            // Volume name
                            if let Some(vol) = spec.get("volumeName").and_then(|v| v.as_str()) {
                                volume = Some(vol.to_string());
                            }
                        }
                        
                        if let Some(status_obj) = item.get("status") {
                            // Status phase
                            if let Some(phase) = status_obj.get("phase").and_then(|v| v.as_str()) {
                                status = Some(phase.to_string());
                            }
                            // Capacity
                            if let Some(cap) = status_obj.get("capacity").and_then(|c| c.get("storage")).and_then(|v| v.as_str()) {
                                capacity = Some(cap.to_string());
                            }
                        }
                        
                        (status, volume, capacity, access_modes, storage_class, volume_attributes_class)
                    } else {
                        (None, None, None, None, None, None)
                    };
                    
                    // Extract EndpointSlice ports and endpoints
                    let (endpointslice_ports, endpointslice_endpoints) = if kind == "EndpointSlice" && group == "discovery.k8s.io" {
                        let mut ports = Vec::new();
                        let mut endpoints = Vec::new();
                        
                        // Extract ports
                        if let Some(ports_array) = item.get("ports").and_then(|v| v.as_array()) {
                            for port_obj in ports_array {
                                let port = port_obj.get("port").and_then(|v| v.as_i64());
                                let protocol = port_obj.get("protocol").and_then(|v| v.as_str()).map(|s| s.to_string());
                                let name = port_obj.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
                                ports.push(EndpointSlicePort {
                                    port,
                                    protocol,
                                    name,
                                });
                            }
                        }
                        
                        // Extract endpoints
                        if let Some(endpoints_array) = item.get("endpoints").and_then(|v| v.as_array()) {
                            for endpoint_obj in endpoints_array {
                                let mut addresses = Vec::new();
                                if let Some(addresses_array) = endpoint_obj.get("addresses").and_then(|v| v.as_array()) {
                                    for addr in addresses_array {
                                        if let Some(addr_str) = addr.as_str() {
                                            addresses.push(addr_str.to_string());
                                        }
                                    }
                                }
                                let ready = endpoint_obj.get("conditions").and_then(|c| c.get("ready")).and_then(|v| v.as_bool());
                                let serving = endpoint_obj.get("conditions").and_then(|c| c.get("serving")).and_then(|v| v.as_bool());
                                let terminating = endpoint_obj.get("conditions").and_then(|c| c.get("terminating")).and_then(|v| v.as_bool());
                                endpoints.push(EndpointSliceEndpoint {
                                    addresses,
                                    ready,
                                    serving,
                                    terminating,
                                });
                            }
                        }
                        
                        if !ports.is_empty() || !endpoints.is_empty() {
                            (if !ports.is_empty() { Some(ports) } else { None }, if !endpoints.is_empty() { Some(endpoints) } else { None })
                        } else {
                            (None, None)
                        }
                    } else {
                        (None, None)
                    };
                    
                    let rs = ResourceSummary {
                        reference: ResourceRef {
                            group: group.clone(),
                            version: version.clone(),
                            kind: kind.clone(),
                            namespace: namespace.clone(),
                            name: name.clone(),
                        },
                        labels,
                        annotations,
                        creation_timestamp,
                        container_statuses,
                        service_ports,
                        service_type,
                        pv_capacity,
                        pv_access_modes,
                        pv_reclaim_policy,
                        pv_status,
                        pv_claim,
                        pv_storage_class,
                        pv_volume_attributes_class,
                        pv_reason,
                        pvc_status,
                        pvc_volume,
                        pvc_capacity,
                        pvc_access_modes,
                        pvc_storage_class,
                        pvc_volume_attributes_class,
                        endpointslice_ports,
                        endpointslice_endpoints,
                    };
                    // mutate overview incrementally
                    if let Some(mut ov) = overview.get_untracked() {
                        // find matching group - prefer exact version match, else match by (group,kind)
                        let mut idx: Option<usize> = None;
                        for (i, grp) in ov.resources.iter().enumerate() {
                            if grp.gvk.group == group
                                && grp.gvk.kind == kind
                                && grp.gvk.version == version
                            {
                                idx = Some(i);
                                break;
                            }
                        }
                        if idx.is_none() {
                            for (i, grp) in ov.resources.iter().enumerate() {
                                if grp.gvk.group == group && grp.gvk.kind == kind {
                                    idx = Some(i);
                                    break;
                                }
                            }
                        }
                        if let Some(i) = idx {
                            let mut grp = ov.resources[i].clone();
                            match op {
                                "ADDED" | "MODIFIED" => {
                                    // replace if same (ns,name) exists; else push
                                    let mut replaced = false;
                                    for it in grp.items.iter_mut() {
                                        if it.reference.name == name
                                            && it.reference.namespace == namespace
                                        {
                                            *it = rs.clone();
                                            replaced = true;
                                            break;
                                        }
                                    }
                                    if !replaced {
                                        grp.items.push(rs);
                                    }
                                }
                                "DELETED" => {
                                    grp.items.retain(|it| {
                                        !(it.reference.name == name
                                            && it.reference.namespace == namespace)
                                    });
                                }
                                _ => {}
                            }
                            ov.resources[i] = grp;
                            set_overview.set(Some(ov));
                        }
                    }
                }) as Box<dyn FnMut(JsValue)>);
                // listen returns an unlisten function; we won't store it for brevity
                let _ = listen("k8s:resource_event", handler.as_ref().unchecked_ref()).await;
                handler.forget();
            });
        }
    });

    // Derived state for right panel (reacts to overview, selected_key, selected_ns)
    let (fr_has_errors, set_fr_has_errors) = signal(false);
    let (fr_errors_str, set_fr_errors_str) = signal(String::new());
    let (fr_items, set_fr_items) = signal::<Vec<ResourceSummary>>(Vec::new());
    let (fr_found, set_fr_found) = signal(false);
    Effect::new(move |_| {
        // establish reactive dependencies
        let ov = overview.get();
        let key = selected_key.get();
        let ns_filter = selected_ns.get();
        let mut has_errors = false;
        let mut errors_str = String::new();
        let mut items: Vec<ResourceSummary> = Vec::new();
        let mut found = false;
        if let (Some(ov), Some(k)) = (ov, key) {
            if let Some(grp) = ov
                .resources
                .iter()
                .find(|g| format!("{}.{}/{}", g.gvk.kind, g.gvk.version, g.gvk.group) == k)
            {
                has_errors = !grp.errors.is_empty();
                errors_str = grp.errors.join(" | ");
                items = grp.items.clone();
                if let Some(ns) = ns_filter.clone() {
                    items = items
                        .into_iter()
                        .filter(|it| it.reference.namespace.as_deref() == Some(ns.as_str()))
                        .collect();
                }
                found = true;
            } else {
                let parts: Vec<&str> = k.split('/').collect();
                if parts.len() == 2 {
                    let left = parts[0];
                    let right = parts[1];
                    let wanted_kind = left.split('.').next().unwrap_or("");
                    let wanted_group = right;
                    if let Some(grp) = ov
                        .resources
                        .iter()
                        .find(|g| g.gvk.kind == wanted_kind && g.gvk.group == wanted_group)
                    {
                        has_errors = !grp.errors.is_empty();
                        errors_str = grp.errors.join(" | ");
                        items = grp.items.clone();
                        if let Some(ns) = ns_filter {
                            items = items
                                .into_iter()
                                .filter(|it| it.reference.namespace.as_deref() == Some(ns.as_str()))
                                .collect();
                        }
                        found = true;
                    }
                }
            }
        }
        set_fr_has_errors.set(has_errors);
        set_fr_errors_str.set(errors_str);
        set_fr_items.set(items);
        set_fr_found.set(found);
    });
    let do_describe = move |r: ResourceRef| {
        set_detail.set(None);
        spawn_local(async move {
            #[derive(Serialize)]
            struct DescribeInvoke {
                args: ResourceRef,
            }
            let payload = DescribeInvoke { args: r };
            let val = invoke(
                "describe_resource",
                serde_wasm_bindgen::to_value(&payload).unwrap(),
            )
            .await;
            let parsed: Result<serde_json::Value, _> = serde_wasm_bindgen::from_value(val.clone());
            match parsed {
                Ok(v) => set_detail.set(Some(v)),
                Err(e) => {
                    set_detail.set(Some(serde_json::json!({
                        "error": "describe failed",
                        "message": e.to_string()
                    })));
                }
            };
        });
    };

    let do_logs = move |ns: String, name: String, container: Option<String>| {
        set_logs.set(None);
        set_logs_container.set(container.clone());
        spawn_local(async move {
            #[derive(Serialize)]
            struct LogArgs {
                namespace: String,
                name: String,
                container: Option<String>,
                tail_lines: Option<i64>,
                previous: Option<bool>,
                timeout_secs: Option<u64>,
            }
            #[derive(Serialize)]
            struct LogInvoke {
                args: LogArgs,
            }
            let payload = LogInvoke {
                args: LogArgs {
                    namespace: ns.clone(),
                    name: name.clone(),
                    container: container.clone(),
                    tail_lines: Some(200),
                    previous: Some(false),
                    timeout_secs: Some(5),
                },
            };
            let val = invoke("pod_logs", serde_wasm_bindgen::to_value(&payload).unwrap()).await;
            set_logs.set(val.as_string());
        });
    };

    view! {
        <main class="container">
            <div style="display:flex;gap:16px;align-items:flex-start;">
                <div style="width:280px;flex:0 0 280px;border-right:1px solid #ddd;padding-right:12px;">
                    <div style="display:flex;justify-content:space-between;align-items:center;">
                        <h2 style="margin:0;">"Resource Types"</h2>
                        <button on:click=move |_: MouseEvent| load_overview() disabled=move || loading.get()>"Refresh"</button>
                    </div>
                    <Show when=move || loading.get()>
                        <div style="margin-top:8px;color:#555">"Loading..."</div>
                    </Show>
                    <div style="margin-top:8px;display:flex;gap:8px;align-items:center;">
                        <span style="color:#555">"Context"</span>
                        <select on:change=move |ev| {
                            let v = event_target_value(&ev);
                            let payload = serde_wasm_bindgen::to_value(&serde_json::json!({ "args": { "name": if v.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(v.clone()) }}})).unwrap();
                            let vclone = v.clone();
                            spawn_local(async move {
                                let _ = invoke("set_context", payload).await;
                                // set current, clear namespace filter, and reload overview
                                set_current_ctx.set(if vclone.is_empty() { None } else { Some(vclone.clone()) });
                                set_selected_ns.set(None);
                                load_overview();
                            });
                        }>
                            { move || {
                                let ctxs = contexts.get();
                                let cur = current_ctx.get();
                                view! {
                                    <>
                                        <option value="">{ cur.clone().unwrap_or_else(|| "default".into()) }</option>
                                        { ctxs.into_iter().map(|c| {
                                            let val_attr = c.clone();
                                            let val_text = val_attr.clone();
                                            view! { <option value={val_attr}>{ val_text }</option> }
                                        }).collect_view() }
                                    </>
                                }
                            } }
                        </select>
                    </div>
                    <Show when=move || overview.get().is_some()>
                        { move || {
                            let ov = overview.get().unwrap();
                            let mut ns_list: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
                            for rg in &ov.resources {
                                for it in &rg.items {
                                    if let Some(ns) = &it.reference.namespace {
                                        ns_list.insert(ns.clone());
                                    }
                                }
                            }
                            let mut nss: Vec<String> = ns_list.into_iter().collect();
                            nss.insert(0, "All".into());
                            view! {
                                <div style="margin-top:8px;display:flex;gap:8px;align-items:center;">
                                    <span style="color:#555">"Namespace"</span>
                                    <select on:change=move |ev| {
                                        let v = event_target_value(&ev);
                                        if v == "All" { set_selected_ns.set(None); }
                                        else { set_selected_ns.set(Some(v)); }
                                    }>
                                        { nss.into_iter().map(|ns| {
                                            let value = ns.clone();
                                            let display = ns.clone();
                                            let is_sel = move || {
                                                match selected_ns.get() {
                                                    None => value == "All",
                                                    Some(ref s) => value == *s,
                                                }
                                            };
                                            view! { <option selected=move || is_sel() value=display.clone()>{ display.clone() }</option> }
                                        }).collect_view() }
                                    </select>
                                </div>
                            }
                        }}
                    </Show>
                    { let groups = static_menu();
                      view! {
                        <ul style="list-style:none;padding-left:0;margin-top:12px;">
                            { groups.into_iter().map(|grp| {
                                let heading = grp.group_title;
                                let items = grp.items.clone();
                                view! {
                                    <li style="margin-bottom:10px;">
                                        <div style="font-weight:600;color:#333;margin:6px 0;">{ heading }</div>
                                        <ul style="list-style:none;padding-left:8px;">
                                            { items.into_iter().map(|item| {
                                                let group = item.group;
                                                let kind = item.kind;
                                                let item_key_for_click = format!("{}.*/{}", kind, group); // pattern key
                                                let is_active = move || {
                                                    if let Some(sel) = selected_key.get() {
                                                        // match by group+kind ignoring version
                                                        let parts: Vec<&str> = sel.split('/').collect();
                                                        if parts.len() == 2 {
                                                            let left = parts[0]; // Kind.Version
                                                            let right = parts[1]; // Group
                                                            let kind_part = left.split('.').next().unwrap_or("");
                                                            return kind_part == kind && right == group;
                                                        }
                                                    }
                                                    false
                                                };
                                                let title = item.title;
                                                view! {
                                                    <li style=move || {
                                                        if is_active() { "padding:6px 8px;background:#eef;border-radius:6px;cursor:pointer;margin-bottom:4px;" }
                                                        else { "padding:6px 8px;border-radius:6px;cursor:pointer;margin-bottom:4px;" }
                                                    } on:click={
                                                        let g = group.to_string();
                                                        let k = kind.to_string();
                                                        // start backend watch for this selection
                                                        let ns_sel = selected_ns.get_untracked();
                                                        move |_| {
                                                            // build selection from overview if available; otherwise fallback pattern
                                                            if let Some(ov) = overview.get() {
                                                                if let Some(grp) = ov.resources.iter().find(|rg| rg.gvk.group == g && rg.gvk.kind == k) {
                                                                    // capture owned copies for 'static async move
                                                                    let sel_group = grp.gvk.group.clone();
                                                                    let sel_version = grp.gvk.version.clone();
                                                                    let sel_kind = grp.gvk.kind.clone();
                                                                    let ns_owned = ns_sel.clone();
                                                                    let sel = format!("{}.{}/{}", sel_kind, sel_version, sel_group);
                                                                    set_selected_key.set(Some(sel));
                                                                    // stop previous and start cascade watchers (owned payloads)
                                                                    spawn_local(async move {
                                                                        let _ = invoke("stop_all_watches", JsValue::NULL).await;
                                                                        // primary
                                                                        let payload_primary = serde_json::json!({ "args": {
                                                                            "group": sel_group.clone(),
                                                                            "version": sel_version.clone(),
                                                                            "kind": sel_kind.clone(),
                                                                            "namespace": ns_owned.clone(),
                                                                        }});
                                                                        let payload_primary = serde_wasm_bindgen::to_value(&payload_primary).unwrap();
                                                                        let _ = invoke("start_watch", payload_primary).await;
                                                                        // cascade map
                                                                        let mut extra: Vec<(&str,&str,&str)> = Vec::new();
                                                                        match (sel_group.as_str(), sel_kind.as_str()) {
                                                                            ("apps","Deployment") => { extra.push(("apps","v1","ReplicaSet")); extra.push(("", "v1","Pod")); }
                                                                            ("apps","StatefulSet") => { extra.push(("", "v1","Pod")); }
                                                                            ("apps","DaemonSet") => { extra.push(("", "v1","Pod")); }
                                                                            ("batch","Job") => { extra.push(("", "v1","Pod")); }
                                                                            ("batch","CronJob") => { extra.push(("batch","v1","Job")); extra.push(("", "v1","Pod")); }
                                                                            ("","Service") => { extra.push(("discovery.k8s.io","v1","EndpointSlice")); }
                                                                            _ => {}
                                                                        }
                                                                        for (eg, ev, ek) in extra {
                                                                            let payload_extra = serde_json::json!({ "args": {
                                                                                "group": eg.to_string(),
                                                                                "version": ev.to_string(),
                                                                                "kind": ek.to_string(),
                                                                                "namespace": ns_owned.clone(),
                                                                            }});
                                                                            let payload_extra = serde_wasm_bindgen::to_value(&payload_extra).unwrap();
                                                                            let _ = invoke("start_watch", payload_extra).await;
                                                                        }
                                                                    });
                                                                    return;
                                                                }
                                                            }
                                                            set_selected_key.set(Some(item_key_for_click.clone()));
                                                            // fallback: start watch ignoring version
                                                            let g_owned = g.clone();
                                                            let k_owned = k.clone();
                                                            let ns_owned = ns_sel.clone();
                                                            spawn_local(async move {
                                                                let _ = invoke("stop_all_watches", JsValue::NULL).await;
                                                                let payload2 = serde_json::json!({ "args": {
                                                                    "group": g_owned.clone(),
                                                                    "version": "",
                                                                    "kind": k_owned.clone(),
                                                                    "namespace": ns_owned.clone(),
                                                                }});
                                                                let payload2 = serde_wasm_bindgen::to_value(&payload2).unwrap();
                                                                let _ = invoke("start_watch", payload2).await;
                                                                // cascade with known versions
                                                                let mut extra: Vec<(&str,&str,&str)> = Vec::new();
                                                                match (g_owned.as_str(), k_owned.as_str()) {
                                                                    ("apps","Deployment") => { extra.push(("apps","v1","ReplicaSet")); extra.push(("", "v1","Pod")); }
                                                                    ("apps","StatefulSet") => { extra.push(("", "v1","Pod")); }
                                                                    ("apps","DaemonSet") => { extra.push(("", "v1","Pod")); }
                                                                    ("batch","Job") => { extra.push(("", "v1","Pod")); }
                                                                    ("batch","CronJob") => { extra.push(("batch","v1","Job")); extra.push(("", "v1","Pod")); }
                                                                    ("","Service") => { extra.push(("discovery.k8s.io","v1","EndpointSlice")); }
                                                                    _ => {}
                                                                }
                                                                for (eg, ev, ek) in extra {
                                                                    let payload_extra = serde_json::json!({ "args": {
                                                                        "group": eg.to_string(),
                                                                        "version": ev.to_string(),
                                                                        "kind": ek.to_string(),
                                                                        "namespace": ns_owned.clone(),
                                                                    }});
                                                                    let payload_extra = serde_wasm_bindgen::to_value(&payload_extra).unwrap();
                                                                    let _ = invoke("start_watch", payload_extra).await;
                                                                }
                                                            });
                                                        }
                                                    }>
                                                        <span>{ title }</span>
                                                    </li>
                                                }
                                            }).collect_view() }
                                        </ul>
                                    </li>
                                }
                            }).collect_view() }
                        </ul>
                      }
                    }
                </div>
                <div style="flex:1;min-height:400px;">
                    <h2 style="margin-top:0;">"Resource List"</h2>
                    <Show when=move || overview.get().is_some()>
                        { move || {
                            view! {
                                <div>
                                    <Show when=move || fr_has_errors.get()>
                                        <div style="color:#b00;margin-bottom:8px">{ fr_errors_str.get() }</div>
                                    </Show>
                                    <div>
                                        <table style="width:100%;border-collapse:collapse;">
                                            <thead>
                                                { move || {
                                                    let items = fr_items.get();
                                                    let is_service = items.first().map(|item| {
                                                        item.reference.kind == "Service" && item.reference.group.is_empty()
                                                    }).unwrap_or(false);
                                                    let is_pv = items.first().map(|item| {
                                                        item.reference.kind == "PersistentVolume" && item.reference.group.is_empty()
                                                    }).unwrap_or(false);
                                                    let is_pvc = items.first().map(|item| {
                                                        item.reference.kind == "PersistentVolumeClaim" && item.reference.group.is_empty()
                                                    }).unwrap_or(false);
                                                    let is_endpointslice = items.first().map(|item| {
                                                        item.reference.kind == "EndpointSlice" && item.reference.group == "discovery.k8s.io"
                                                    }).unwrap_or(false);
                                                    view! {
                                                        <tr>
                                                            <Show when=move || !is_pv>
                                                                <th style="text-align:left;border-bottom:1px solid #ddd;padding:8px 12px;vertical-align:middle;">"Namespace"</th>
                                                            </Show>
                                                            <th style="text-align:left;border-bottom:1px solid #ddd;padding:8px 12px;vertical-align:middle;">"Name"</th>
                                                            <Show when=move || is_pv>
                                                                <th style="text-align:left;border-bottom:1px solid #ddd;padding:8px 12px;vertical-align:middle;">"Capacity"</th>
                                                                <th style="text-align:left;border-bottom:1px solid #ddd;padding:8px 12px;vertical-align:middle;">"Access Modes"</th>
                                                                <th style="text-align:left;border-bottom:1px solid #ddd;padding:8px 12px;vertical-align:middle;">"Reclaim Policy"</th>
                                                                <th style="text-align:left;border-bottom:1px solid #ddd;padding:8px 12px;vertical-align:middle;">"Status"</th>
                                                                <th style="text-align:left;border-bottom:1px solid #ddd;padding:8px 12px;vertical-align:middle;">"Claim"</th>
                                                                <th style="text-align:left;border-bottom:1px solid #ddd;padding:8px 12px;vertical-align:middle;">"StorageClass"</th>
                                                                <th style="text-align:left;border-bottom:1px solid #ddd;padding:8px 12px;vertical-align:middle;">"VolumeAttributesClass"</th>
                                                                <th style="text-align:left;border-bottom:1px solid #ddd;padding:8px 12px;vertical-align:middle;">"Reason"</th>
                                                            </Show>
                                                            <Show when=move || is_pvc>
                                                                <th style="text-align:left;border-bottom:1px solid #ddd;padding:8px 12px;vertical-align:middle;">"Status"</th>
                                                                <th style="text-align:left;border-bottom:1px solid #ddd;padding:8px 12px;vertical-align:middle;">"Volume"</th>
                                                                <th style="text-align:left;border-bottom:1px solid #ddd;padding:8px 12px;vertical-align:middle;">"Capacity"</th>
                                                                <th style="text-align:left;border-bottom:1px solid #ddd;padding:8px 12px;vertical-align:middle;">"Access Modes"</th>
                                                                <th style="text-align:left;border-bottom:1px solid #ddd;padding:8px 12px;vertical-align:middle;">"StorageClass"</th>
                                                                <th style="text-align:left;border-bottom:1px solid #ddd;padding:8px 12px;vertical-align:middle;">"VolumeAttributesClass"</th>
                                                            </Show>
                                                            <Show when=move || is_service>
                                                                <th style="text-align:left;border-bottom:1px solid #ddd;padding:8px 12px;vertical-align:middle;">"Port"</th>
                                                                <th style="text-align:left;border-bottom:1px solid #ddd;padding:8px 12px;vertical-align:middle;">"Type"</th>
                                                            </Show>
                                                            <Show when=move || is_endpointslice>
                                                                <th style="text-align:left;border-bottom:1px solid #ddd;padding:8px 12px;vertical-align:middle;">"Ports"</th>
                                                                <th style="text-align:left;border-bottom:1px solid #ddd;padding:8px 12px;vertical-align:middle;">"Endpoints"</th>
                                                            </Show>
                                                            <Show when=move || !is_pv && !is_pvc>
                                                                <th style="text-align:left;border-bottom:1px solid #ddd;padding:8px 12px;vertical-align:middle;">"Created"</th>
                                                            </Show>
                                                            <th style="text-align:left;border-bottom:1px solid #ddd;padding:8px 12px;vertical-align:middle;">"Actions"</th>
                                                        </tr>
                                                    }
                                                }}
                                            </thead>
                                            <tbody>
                                                { fr_items.get().into_iter().map(|item| {
                                                    let is_pod = item.reference.kind == "Pod" && item.reference.group.is_empty();
                                                    let ns = item.reference.namespace.clone().unwrap_or_else(|| "-".into());
                                                    let created = item.creation_timestamp.clone().unwrap_or_else(|| "-".into());
                                                    let name = item.reference.name.clone();
                                                    // mark default StorageClass
                                                    let is_default_sc = {
                                                        if item.reference.kind == "StorageClass" && item.reference.group == "storage.k8s.io" {
                                                            let ann = &item.annotations;
                                                            let key_stable = "storageclass.kubernetes.io/is-default-class";
                                                            let key_beta = "storageclass.beta.kubernetes.io/is-default-class";
                                                            let val_stable = ann.get(key_stable).and_then(|v| v.as_str()).unwrap_or("");
                                                            let val_beta = ann.get(key_beta).and_then(|v| v.as_str()).unwrap_or("");
                                                            val_stable.eq_ignore_ascii_case("true") || val_beta.eq_ignore_ascii_case("true")
                                                        } else { false }
                                                    };
                                                    let r = item.reference.clone();
                                                    let container_statuses = item.container_statuses.clone();
                                                    let pod_namespace = item.reference.namespace.clone().unwrap_or_default();
                                                    let pod_name = item.reference.name.clone();
                                                    let is_service = item.reference.kind == "Service" && item.reference.group.is_empty();
                                                    let service_ports = item.service_ports.clone();
                                                    let service_type = item.service_type.clone();
                                                    let is_pv = item.reference.kind == "PersistentVolume" && item.reference.group.is_empty();
                                                    let is_pvc = item.reference.kind == "PersistentVolumeClaim" && item.reference.group.is_empty();
                                                    let is_endpointslice = item.reference.kind == "EndpointSlice" && item.reference.group == "discovery.k8s.io";
                                                    let endpointslice_ports = item.endpointslice_ports.clone();
                                                    let endpointslice_endpoints = item.endpointslice_endpoints.clone();
                                                    // Pre-build service port views outside the closure
                                                    let service_port_views: Vec<_> = if is_service {
                                                        let svc_type_clone = service_type.clone();
                                                        service_ports.as_ref().map(|ports| {
                                                            ports.iter().map(|p| {
                                                                let port_display = if let Some(name) = &p.name {
                                                                    format!("{}:{}", name, p.port)
                                                                } else {
                                                                    p.port.to_string()
                                                                };
                                                                let target_port_text = p.target_port.as_ref().map(|tp| format!("→{}", tp));
                                                                // If NodePort type and has nodePort, show it
                                                                let node_port_text = if svc_type_clone.as_deref() == Some("NodePort") {
                                                                    p.node_port.map(|np| format!("(NodePort:{})", np))
                                                                } else {
                                                                    None
                                                                };
                                                                let protocol_display = p.protocol.as_ref().cloned().unwrap_or_else(|| "TCP".to_string());
                                                                view! {
                                                                    <div style="font-size:11px;line-height:1.4;">
                                                                        <span style="font-weight:500;">{ port_display }</span>
                                                                        { if let Some(tp_text) = target_port_text {
                                                                            view! { <span style="color:#666;margin-left:4px;">{ tp_text }</span> }
                                                                        } else {
                                                                            view! { <span style="color:#666;margin-left:4px;">{ String::new() }</span> }
                                                                        }}
                                                                        { if let Some(np_text) = node_port_text {
                                                                            view! { <span style="color:#0066cc;margin-left:4px;font-weight:500;">{ np_text }</span> }
                                                                        } else {
                                                                            view! { <span style="color:#666;margin-left:4px;">{ String::new() }</span> }
                                                                        }}
                                                                        <span style="color:#999;margin-left:4px;font-size:10px;">{ protocol_display }</span>
                                                                    </div>
                                                                }
                                                            }).collect()
                                                        }).unwrap_or_default()
                                                    } else {
                                                        Vec::new()
                                                    };
                                                    // Pre-build service port and type views outside the closure
                                                    let service_port_cell = if is_service {
                                                        service_port_views.clone()
                                                    } else {
                                                        Vec::new()
                                                    };
                                                    let service_type_cell = if is_service {
                                                        service_type.clone().unwrap_or_else(|| "-".to_string())
                                                    } else {
                                                        String::new()
                                                    };
                                                    // Pre-build service type view as Vec<View> to match other branches
                                                    let service_type_views: Vec<_> = if is_service {
                                                        vec![view! { <div>{ service_type_cell.clone() }</div> }]
                                                    } else {
                                                        Vec::new()
                                                    };
                                                    // Pre-build EndpointSlice port views
                                                    let endpointslice_port_views: Vec<_> = if is_endpointslice {
                                                        endpointslice_ports.as_ref().map(|ports| {
                                                            ports.iter().map(|p| {
                                                                let port_display = if let Some(port_num) = p.port {
                                                                    if let Some(name) = &p.name {
                                                                        format!("{}:{}", name, port_num)
                                                                    } else {
                                                                        port_num.to_string()
                                                                    }
                                                                } else if let Some(name) = &p.name {
                                                                    name.clone()
                                                                } else {
                                                                    "-".to_string()
                                                                };
                                                                let protocol_display = p.protocol.as_ref().cloned().unwrap_or_else(|| "TCP".to_string());
                                                                view! {
                                                                    <div style="font-size:11px;line-height:1.4;">
                                                                        <span style="font-weight:500;">{ port_display }</span>
                                                                        <span style="color:#999;margin-left:4px;font-size:10px;">{ protocol_display }</span>
                                                                    </div>
                                                                }
                                                            }).collect()
                                                        }).unwrap_or_default()
                                                    } else {
                                                        Vec::new()
                                                    };
                                                    // Pre-build EndpointSlice endpoint views
                                                    let endpointslice_endpoint_views: Vec<_> = if is_endpointslice {
                                                        endpointslice_endpoints.as_ref().map(|endpoints| {
                                                            endpoints.iter().map(|ep| {
                                                                let addresses_display = if ep.addresses.is_empty() {
                                                                    "-".to_string()
                                                                } else {
                                                                    ep.addresses.join(", ")
                                                                };
                                                                let status_parts = vec![
                                                                    ep.ready.map(|r| if r { "Ready" } else { "NotReady" }),
                                                                    ep.serving.map(|s| if s { "Serving" } else { "NotServing" }),
                                                                    ep.terminating.map(|t| if t { "Terminating" } else { "NotTerminating" }),
                                                                ].into_iter().flatten().collect::<Vec<_>>();
                                                                let status_display = if status_parts.is_empty() {
                                                                    String::new()
                                                                } else {
                                                                    format!(" ({})", status_parts.join(", "))
                                                                };
                                                                view! {
                                                                    <div style="font-size:11px;line-height:1.4;">
                                                                        <span style="font-weight:500;">{ addresses_display }</span>
                                                                        { if !status_display.is_empty() {
                                                                            view! { <span style="color:#666;margin-left:4px;">{ status_display }</span> }
                                                                        } else {
                                                                            view! { <span style="color:#666;margin-left:4px;">{ String::new() }</span> }
                                                                        }}
                                                                    </div>
                                                                }
                                                            }).collect()
                                                        }).unwrap_or_default()
                                                    } else {
                                                        Vec::new()
                                                    };
                                                    // Pre-build container status views outside the closure
                                                    let container_status_views: Vec<_> = if is_pod {
                                                        container_statuses.as_ref().map(|statuses| {
                                                            statuses.iter().map(|cs| {
                                                                let container_name = cs.name.clone();
                                                                let container_state = cs.state.clone();
                                                                let container_ready = cs.ready;
                                                                let container_reason = cs.reason.clone();
                                                                // Determine color based on container name and state
                                                                let (bg_color, text_color) = if container_name == "main" && container_state == "running" {
                                                                    ("#ffd700", "#333") // yellow for main container running
                                                                } else if container_state.is_empty() || container_state == "unknown" {
                                                                    ("#b3d9ff", "#333") // light blue for no state/not init yet
                                                                } else if container_state == "init" || container_state == "waiting" {
                                                                    ("#ffd700", "#333") // yellow
                                                                } else if container_ready && container_state == "running" {
                                                                    ("#28a745", "#fff") // green
                                                                } else if container_state == "terminated" {
                                                                    // If terminated, check reason: Completed = green, otherwise red
                                                                    if let Some(ref reason) = container_reason {
                                                                        if reason == "Completed" {
                                                                            ("#28a745", "#fff") // green
                                                                        } else {
                                                                            ("#dc3545", "#fff") // red
                                                                        }
                                                                    } else {
                                                                        ("#dc3545", "#fff") // red (no reason)
                                                                    }
                                                                } else {
                                                                    ("#dc3545", "#fff") // red
                                                                };
                                                                // Display state with reason if terminated
                                                                let display_text = if container_state == "terminated" {
                                                                    if let Some(reason) = container_reason {
                                                                        format!("{}: {} ({})", container_name, container_state, reason)
                                                                    } else {
                                                                        format!("{}: {}", container_name, container_state)
                                                                    }
                                                                } else {
                                                                    format!("{}: {}", container_name, container_state)
                                                                };
                                                                // Prepare click handler for container logs
                                                                let ns_for_logs = pod_namespace.clone();
                                                                let pod_name_for_logs = pod_name.clone();
                                                                let container_name_for_logs = container_name.clone();
                                                                view! {
                                                                    <span 
                                                                        style=format!("display:inline-block;padding:2px 6px;border-radius:4px;background:{};color:{};font-size:10px;white-space:nowrap;cursor:pointer;", bg_color, text_color)
                                                                        on:click=move |_| {
                                                                            do_logs(ns_for_logs.clone(), pod_name_for_logs.clone(), Some(container_name_for_logs.clone()));
                                                                        }
                                                                        title="Click to view logs"
                                                                    >
                                                                        { display_text }
                                                                    </span>
                                                                }
                                                            }).collect::<Vec<_>>()
                                                        }).unwrap_or_default()
                                                    } else {
                                                        Vec::new()
                                                    };
                                                    // Pre-build PV fields
                                                    let pv_capacity = item.pv_capacity.clone().unwrap_or_else(|| "-".to_string());
                                                    let pv_access_modes = item.pv_access_modes.clone().unwrap_or_default();
                                                    let pv_access_modes_display = if pv_access_modes.is_empty() {
                                                        "-".to_string()
                                                    } else {
                                                        pv_access_modes.join(",")
                                                    };
                                                    let pv_reclaim_policy = item.pv_reclaim_policy.clone().unwrap_or_else(|| "-".to_string());
                                                    let pv_status = item.pv_status.clone().unwrap_or_else(|| "-".to_string());
                                                    let pv_claim = item.pv_claim.clone().unwrap_or_else(|| "-".to_string());
                                                    let pv_storage_class = item.pv_storage_class.clone().unwrap_or_else(|| "-".to_string());
                                                    let pv_volume_attributes_class = item.pv_volume_attributes_class.clone().unwrap_or_else(|| "-".to_string());
                                                    let pv_reason = item.pv_reason.clone().unwrap_or_else(|| "-".to_string());
                                                    
                                                    // Pre-build PVC fields
                                                    let pvc_status = item.pvc_status.clone().unwrap_or_else(|| "-".to_string());
                                                    let pvc_volume = item.pvc_volume.clone().unwrap_or_else(|| "-".to_string());
                                                    let pvc_capacity = item.pvc_capacity.clone().unwrap_or_else(|| "-".to_string());
                                                    let pvc_access_modes = item.pvc_access_modes.clone().unwrap_or_default();
                                                    let pvc_access_modes_display = if pvc_access_modes.is_empty() {
                                                        "-".to_string()
                                                    } else {
                                                        pvc_access_modes.join(",")
                                                    };
                                                    let pvc_storage_class = item.pvc_storage_class.clone().unwrap_or_else(|| "-".to_string());
                                                    let pvc_volume_attributes_class = item.pvc_volume_attributes_class.clone().unwrap_or_else(|| "-".to_string());
                                                    
                                                    // Build conditional cells outside view! macro
                                                    let namespace_cell_view = if !is_pv {
                                                        Some(view! {
                                                            <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;">{ ns.clone() }</td>
                                                        }.into_view())
                                                    } else {
                                                        None
                                                    };
                                                    
                                                    let pv_cells_view = if is_pv {
                                                        Some(view! {
                                                            <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;">{ pv_capacity.clone() }</td>
                                                            <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;max-width:150px;word-break:break-word;">{ pv_access_modes_display.clone() }</td>
                                                            <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;">{ pv_reclaim_policy.clone() }</td>
                                                            <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;">{ pv_status.clone() }</td>
                                                            <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;max-width:200px;word-break:break-word;">{ pv_claim.clone() }</td>
                                                            <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;">{ pv_storage_class.clone() }</td>
                                                            <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;">{ pv_volume_attributes_class.clone() }</td>
                                                            <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;">{ pv_reason.clone() }</td>
                                                        }.into_view())
                                                    } else {
                                                        None
                                                    };
                                                    
                                                    let pvc_cells_view = if is_pvc {
                                                        Some(view! {
                                                            <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;">{ pvc_status.clone() }</td>
                                                            <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;max-width:200px;word-break:break-word;">{ pvc_volume.clone() }</td>
                                                            <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;">{ pvc_capacity.clone() }</td>
                                                            <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;max-width:150px;word-break:break-word;">{ pvc_access_modes_display.clone() }</td>
                                                            <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;">{ pvc_storage_class.clone() }</td>
                                                            <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;">{ pvc_volume_attributes_class.clone() }</td>
                                                        }.into_view())
                                                    } else {
                                                        None
                                                    };
                                                    
                                                    let created_cell_view = if !is_pv && !is_pvc {
                                                        Some(view! {
                                                            <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;">{ created.clone() }</td>
                                                        }.into_view())
                                                    } else {
                                                        None
                                                    };
                                                    
                                                    // Build service cells view
                                                    let service_cells_view = if is_service {
                                                        Some(view! {
                                                            <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;max-width:300px;">
                                                                <div style="display:flex;flex-direction:column;gap:2px;max-height:150px;overflow-y:auto;">
                                                                    { service_port_cell.clone() }
                                                                </div>
                                                            </td>
                                                            <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;max-width:300px;">
                                                                <div style="display:flex;flex-direction:column;gap:2px;max-height:150px;overflow-y:auto;">
                                                                    { service_type_views.clone() }
                                                                </div>
                                                            </td>
                                                        }.into_view())
                                                    } else {
                                                        None
                                                    };
                                                    
                                                    // Build EndpointSlice cells view
                                                    let endpointslice_cells_view = if is_endpointslice {
                                                        Some(view! {
                                                            <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;max-width:300px;">
                                                                <div style="display:flex;flex-direction:column;gap:2px;max-height:150px;overflow-y:auto;">
                                                                    { endpointslice_port_views.clone() }
                                                                </div>
                                                            </td>
                                                            <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;max-width:300px;">
                                                                <div style="display:flex;flex-direction:column;gap:2px;max-height:150px;overflow-y:auto;">
                                                                    { endpointslice_endpoint_views.clone() }
                                                                </div>
                                                            </td>
                                                        }.into_view())
                                                    } else {
                                                        None
                                                    };
                                                    
                                                    // Build empty views with matching types - use same string values as Some branches
                                                    let empty_str = "-".to_string();
                                                    let empty_namespace_view = view! {
                                                        <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;display:none;">{ empty_str.clone() }</td>
                                                    }.into_view();
                                                    let empty_pv_view = view! {
                                                        <td style="display:none;">{ empty_str.clone() }</td>
                                                        <td style="display:none;">{ empty_str.clone() }</td>
                                                        <td style="display:none;">{ empty_str.clone() }</td>
                                                        <td style="display:none;">{ empty_str.clone() }</td>
                                                        <td style="display:none;">{ empty_str.clone() }</td>
                                                        <td style="display:none;">{ empty_str.clone() }</td>
                                                        <td style="display:none;">{ empty_str.clone() }</td>
                                                        <td style="display:none;">{ empty_str.clone() }</td>
                                                    }.into_view();
                                                    let empty_pvc_view = view! {
                                                        <td style="display:none;">{ empty_str.clone() }</td>
                                                        <td style="display:none;">{ empty_str.clone() }</td>
                                                        <td style="display:none;">{ empty_str.clone() }</td>
                                                        <td style="display:none;">{ empty_str.clone() }</td>
                                                        <td style="display:none;">{ empty_str.clone() }</td>
                                                        <td style="display:none;">{ empty_str.clone() }</td>
                                                    }.into_view();
                                                    let empty_created_view = view! {
                                                        <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;display:none;">{ empty_str.clone() }</td>
                                                    }.into_view();
                                                    let empty_service_view = view! {
                                                        <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;max-width:300px;display:none;">
                                                            <div style="display:flex;flex-direction:column;gap:2px;max-height:150px;overflow-y:auto;">
                                                                { Vec::<leptos::prelude::View<_>>::new() }
                                                            </div>
                                                        </td>
                                                        <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;max-width:300px;display:none;">
                                                            <div style="display:flex;flex-direction:column;gap:2px;max-height:150px;overflow-y:auto;">
                                                                { Vec::<leptos::prelude::View<_>>::new() }
                                                            </div>
                                                        </td>
                                                    }.into_view();
                                                    let empty_endpointslice_view = view! {
                                                        <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;max-width:300px;display:none;">
                                                            <div style="display:flex;flex-direction:column;gap:2px;max-height:150px;overflow-y:auto;">
                                                                { Vec::<leptos::prelude::View<_>>::new() }
                                                            </div>
                                                        </td>
                                                        <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;max-width:300px;display:none;">
                                                            <div style="display:flex;flex-direction:column;gap:2px;max-height:150px;overflow-y:auto;">
                                                                { Vec::<leptos::prelude::View<_>>::new() }
                                                            </div>
                                                        </td>
                                                    }.into_view();
                                                    
                                                    view! {
                                                        <tr>
                                                            { namespace_cell_view.unwrap_or(empty_namespace_view) }
                                                            <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;min-width:300px;">
                                                                <div style="display:flex;flex-direction:column;gap:4px;">
                                                                    <div style="line-height:1.5;">
                                                                        <code style="display:inline-block;vertical-align:middle;">{ name.clone() }</code>
                                                                        <Show when=move || is_default_sc>
                                                                            <span style="margin-left:8px;padding:1px 6px;border-radius:10px;background:#eef;color:#335;font-size:12px;display:inline-block;vertical-align:middle;">"default"</span>
                                                                        </Show>
                                                                    </div>
                                                                    { if !container_status_views.is_empty() {
                                                                        view! {
                                                                            <div style="display:flex;flex-wrap:wrap;gap:3px;margin-top:2px;line-height:1.4;">
                                                                                { container_status_views }
                                                                            </div>
                                                                        }.into_view()
                                                                    } else {
                                                                        view! {
                                                                            <div style="display:flex;flex-wrap:wrap;gap:3px;margin-top:2px;">
                                                                                { Vec::<leptos::prelude::View<_>>::new() }
                                                                            </div>
                                                                        }.into_view()
                                                                    }}
                                                                </div>
                                                            </td>
                                                            { pv_cells_view.unwrap_or(empty_pv_view) }
                                                            { pvc_cells_view.unwrap_or(empty_pvc_view) }
                                                            { service_cells_view.unwrap_or(empty_service_view) }
                                                            { endpointslice_cells_view.unwrap_or(empty_endpointslice_view) }
                                                            { created_cell_view.unwrap_or(empty_created_view) }
                                                            <td style="text-align:left;padding:8px 12px;border-bottom:1px solid #f0f0f0;vertical-align:top;white-space:nowrap;">
                                                                <div style="display:flex;gap:6px;align-items:flex-start;">
                                                                    <button on:click={
                                                                        let rr = r.clone();
                                                                        move |_| do_describe(rr.clone())
                                                                    }>"Describe"</button>
                                                                    <Show when=move || is_pod>
                                                                        <button on:click={
                                                                            let ns2 = item.reference.namespace.clone().unwrap_or_default();
                                                                            let name2 = item.reference.name.clone();
                                                                            let container_statuses_clone = item.container_statuses.clone();
                                                                            move |_| {
                                                                                // Use the first containers container (not init-containers)
                                                                                // container_statuses list: [init-containers..., containers...]
                                                                                // Skip all init containers (state == "init") and use first containers container
                                                                                let container_name = container_statuses_clone
                                                                                    .as_ref()
                                                                                    .and_then(|cs| {
                                                                                        // Find the first container from containers list (skip init-containers)
                                                                                        cs.iter()
                                                                                            .find(|c| c.state != "init")
                                                                                            .map(|c| c.name.clone())
                                                                                    });
                                                                                do_logs(ns2.clone(), name2.clone(), container_name)
                                                                            }
                                                                        }>"Logs"</button>
                                                                    </Show>
                                                                </div>
                                                            </td>
                                                        </tr>
                                                    }
                                                }).collect_view() }
                                            </tbody>
                                        </table>
                                        <Show when=move || !fr_found.get()>
                                            { move || view! { <div style="color:#666">"Please select a resource type from the left"</div> }.into_view() }
                                        </Show>
                                    </div>
                                </div>
                            }.into_view()
                        }}
                    </Show>
                    <Show when=move || detail.get().is_some()>
                        { move || {
                            let on_close = {
                                let set_detail = set_detail.clone();
                                move |_| set_detail.set(None)
                            };
                            view! {
                                <div style="position:fixed;inset:0;background:rgba(0,0,0,0.45);display:flex;align-items:center;justify-content:center;z-index:1000;" on:click=on_close>
                                    <div style="background:#fff;max-width:80vw;max-height:80vh;width:800px;border-radius:8px;box-shadow:0 8px 24px rgba(0,0,0,0.2);display:flex;flex-direction:column;overflow:hidden;" on:click=move |e| e.stop_propagation()>
                                        <DescribeContent detail=detail set_detail=set_detail />
                                    </div>
                                </div>
                            }.into_view()
                        } }
                    </Show>
                    <Show when=move || logs.get().is_some()>
                        { move || {
                            let (search_query, set_search_query) = signal::<String>(String::new());
                            let search_input_ref = NodeRef::<leptos::html::Input>::new();
                            let logs_content = move || logs.get().unwrap_or_default();
                            
                            // Highlight text with search query - returns HTML string
                            let highlighted_logs_html = move || {
                                let text = logs_content();
                                let query = search_query.get();
                                if query.is_empty() {
                                    return text;
                                }
                                
                                let query_lower = query.to_lowercase();
                                let text_lower = text.to_lowercase();
                                let mut result = String::new();
                                let mut last_end = 0;
                                let mut search_start = 0;
                                
                                // Find all matches and build HTML
                                while let Some(pos) = text_lower[search_start..].find(&query_lower) {
                                    let actual_pos = search_start + pos;
                                    let end_pos = actual_pos + query.len();
                                    
                                    // Add text before match (escape HTML)
                                    if actual_pos > last_end {
                                        let before = &text[last_end..actual_pos];
                                        result.push_str(&before.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;"));
                                    }
                                    
                                    // Add highlighted match
                                    let matched = &text[actual_pos..end_pos];
                                    let escaped = matched.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
                                    result.push_str(&format!("<mark style=\"background-color:#ffeb3b;color:#000;padding:0 2px;\">{}</mark>", escaped));
                                    
                                    last_end = end_pos;
                                    search_start = actual_pos + 1;
                                }
                                
                                // Add remaining text
                                if last_end < text.len() {
                                    let remaining = &text[last_end..];
                                    result.push_str(&remaining.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;"));
                                }
                                
                                if result.is_empty() {
                                    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
                                } else {
                                    result
                                }
                            };

                            let do_copy_logs = move |_| {
                                let content = logs_content();
                                let payload =
                                    serde_wasm_bindgen::to_value(&serde_json::json!({ "text": content })).unwrap();
                                spawn_local(async move {
                                    let _ = invoke("copy_text", payload).await;
                                });
                            };

                            let handle_keydown_logs = move |e: KeyboardEvent| {
                                // Check for Command+F (Mac) or Ctrl+F (Windows/Linux)
                                let meta = e.ctrl_key() || e.meta_key();
                                if meta && e.key_code() == 70 {
                                    e.prevent_default();
                                    if let Some(input) = search_input_ref.get() {
                                        let _ = input.focus();
                                        let _ = input.select();
                                    }
                                }
                            };

                            let on_close = {
                                let set_logs = set_logs.clone();
                                let set_logs_container = set_logs_container.clone();
                                move |_| {
                                    set_logs.set(None);
                                    set_logs_container.set(None);
                                }
                            };
                            view! {
                                <div style="position:fixed;inset:0;background:rgba(0,0,0,0.45);display:flex;align-items:center;justify-content:center;z-index:1000;" on:click=on_close on:keydown=handle_keydown_logs>
                                    <div style="background:#fff;max-width:80vw;max-height:80vh;width:900px;border-radius:8px;box-shadow:0 8px 24px rgba(0,0,0,0.2);display:flex;flex-direction:column;overflow:hidden;" on:click=move |e| e.stop_propagation()>
                                        <div style="display:flex;align-items:center;justify-content:space-between;padding:10px 12px;border-bottom:1px solid #eee;">
                                            <h3 style="margin:0;font-size:14px;color:#333">
                                                { move || {
                                                    if let Some(container) = logs_container.get() {
                                                        format!("Logs - {}", container)
                                                    } else {
                                                        "Logs".to_string()
                                                    }
                                                }}
                                            </h3>
                                            <div style="display:flex;gap:8px;">
                                                <button on:click=do_copy_logs>"Copy"</button>
                                                <button on:click=move |_| {
                                                    set_logs.set(None);
                                                    set_logs_container.set(None);
                                                }>"Close"</button>
                                            </div>
                                        </div>
                                        <div style="padding:8px 12px;border-bottom:1px solid #eee;background:#f5f5f5;">
                                            <input
                                                node_ref=search_input_ref
                                                type="text"
                                                placeholder="Search... (Cmd+F / Ctrl+F)"
                                                value=move || search_query.get()
                                                on:input=move |e| set_search_query.set(event_target_value(&e))
                                                style="width:100%;padding:6px 10px;border:1px solid #ddd;border-radius:4px;font-size:13px;"
                                            />
                                        </div>
                                        <div style="padding:0;flex:1;overflow:auto;background:#0b1021;">
                                            <pre style="margin:0;padding:12px;white-space:pre-wrap;overflow:auto;color:#d6e1ff;text-align:left;font-family:ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, &quot;Liberation Mono&quot;, &quot;Courier New&quot;, monospace;" inner_html=highlighted_logs_html></pre>
                                        </div>
                                    </div>
                                </div>
                            }.into_view()
                        } }
                    </Show>
                </div>
            </div>
        </main>
    }
}
