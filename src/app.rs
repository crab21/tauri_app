use leptos::ev::MouseEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[component]
fn DescribeContent(
    detail: ReadSignal<Option<serde_json::Value>>,
    set_detail: WriteSignal<Option<serde_json::Value>>,
) -> impl IntoView {
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

    let do_copy = move |_| {
        let content = yaml_text();
        // Use backend to place text into clipboard (reliable across platforms)
        let payload =
            serde_wasm_bindgen::to_value(&serde_json::json!({ "text": content })).unwrap();
        spawn_local(async move {
            let _ = invoke("copy_text", payload).await;
        });
    };

    view! {
        <div style="display:flex;align-items:center;justify-content:space-between;padding:10px 12px;border-bottom:1px solid #eee;">
            <h3 style="margin:0;font-size:14px;color:#333">"Describe"</h3>
            <div style="display:flex;gap:8px;">
                <button on:click=do_copy>"Copy"</button>
                <button on:click=move |_| set_detail.set(None)>"Close"</button>
            </div>
        </div>
        <div style="padding:0;flex:1;overflow:auto;background:#0b1021;">
            <pre style="margin:0;padding:12px;white-space:pre;overflow:auto;color:#d6e1ff;text-align:left;font-family:ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, \"Liberation Mono\", \"Courier New\", monospace;">{ yaml_text }</pre>
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
struct ResourceSummary {
    reference: ResourceRef,
    labels: serde_json::Value,
    annotations: serde_json::Value,
    creation_timestamp: Option<String>,
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

    let do_logs = move |ns: String, name: String| {
        set_logs.set(None);
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
                    container: None,
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
                        <h2 style="margin:0;">"资源类型"</h2>
                        <button on:click=move |_: MouseEvent| load_overview() disabled=move || loading.get()>"刷新"</button>
                    </div>
                    <Show when=move || loading.get()>
                        <div style="margin-top:8px;color:#555">"加载中..."</div>
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
                    <h2 style="margin-top:0;">"资源列表"</h2>
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
                                                <tr>
                                                    <th style="text-align:left;border-bottom:1px solid #ddd;padding:6px 4px;">"Namespace"</th>
                                                    <th style="text-align:left;border-bottom:1px solid #ddd;padding:6px 4px;">"Name"</th>
                                                    <th style="text-align:left;border-bottom:1px solid #ddd;padding:6px 4px;">"Created"</th>
                                                    <th style="text-align:left;border-bottom:1px solid #ddd;padding:6px 4px;">"Actions"</th>
                                                </tr>
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
                                                    view! {
                                                        <tr>
                                                            <td style="padding:6px 4px;border-bottom:1px solid #f0f0f0;">{ ns }</td>
                                                            <td style="padding:6px 4px;border-bottom:1px solid #f0f0f0;">
                                                                <code>{ name.clone() }</code>
                                                                <Show when=move || is_default_sc>
                                                                    <span style="margin-left:8px;padding:1px 6px;border-radius:10px;background:#eef;color:#335;font-size:12px;">"default"</span>
                                                                </Show>
                                                            </td>
                                                            <td style="padding:6px 4px;border-bottom:1px solid #f0f0f0;">{ created }</td>
                                                            <td style="padding:6px 4px;border-bottom:1px solid #f0f0f0;">
                                                                <button on:click={
                                                                    let rr = r.clone();
                                                                    move |_| do_describe(rr.clone())
                                                                }>"Describe"</button>
                                                                <Show when=move || is_pod>
                                                                    <button style="margin-left:6px" on:click={
                                                                        let ns2 = item.reference.namespace.clone().unwrap_or_default();
                                                                        let name2 = item.reference.name.clone();
                                                                        move |_| do_logs(ns2.clone(), name2.clone())
                                                                    }>"Logs"</button>
                                                                </Show>
                                                            </td>
                                                        </tr>
                                                    }
                                                }).collect_view() }
                                            </tbody>
                                        </table>
                                        <Show when=move || !fr_found.get()>
                                            { move || view! { <div style="color:#666">"请选择左侧资源类型"</div> }.into_view() }
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
                            let on_close = {
                                let set_logs = set_logs.clone();
                                move |_| set_logs.set(None)
                            };
                            view! {
                                <div style="position:fixed;inset:0;background:rgba(0,0,0,0.45);display:flex;align-items:center;justify-content:center;z-index:1000;" on:click=on_close>
                                    <div style="background:#fff;max-width:80vw;max-height:80vh;width:900px;border-radius:8px;box-shadow:0 8px 24px rgba(0,0,0,0.2);display:flex;flex-direction:column;overflow:hidden;" on:click=move |e| e.stop_propagation()>
                                        <div style="display:flex;align-items:center;justify-content:space-between;padding:10px 12px;border-bottom:1px solid #eee;">
                                            <h3 style="margin:0;font-size:14px;color:#333">"Logs"</h3>
                                            <button on:click=move |_| set_logs.set(None)>"Close"</button>
                                        </div>
                                        <div style="padding:0;flex:1;overflow:auto;background:#0b1021;">
                                            <pre style="margin:0;padding:12px;white-space:pre-wrap;overflow:auto;color:#d6e1ff;text-align:left;font-family:ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, &quot;Liberation Mono&quot;, &quot;Courier New&quot;, monospace;">{ move || logs.get().unwrap_or_default() }</pre>
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
