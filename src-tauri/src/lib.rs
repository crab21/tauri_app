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
use tauri::{Emitter, Manager, State};
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

/// 格式化错误消息，使其更人性化，去掉类型信息
fn format_error(e: impl std::fmt::Display) -> String {
    let error_str = format!("{}", e);
    
    // 处理常见的错误模式，去掉类型信息
    let cleaned = if error_str.contains("client error (Connect)") {
        "Failed to connect to Kubernetes cluster. Please check your network connection or cluster configuration".to_string()
    } else if error_str.contains("client error") {
        // 提取错误消息，去掉 "client error" 前缀和类型信息
        error_str
            .replace("client error (", "")
            .replace("client error", "Connection error")
            .replace("Connect", "Connection failed")
            .replace(")", "")
            .trim()
            .to_string()
    } else if error_str.contains("failed to load kubeconfig") {
        "Failed to load kubeconfig file".to_string()
    } else if error_str.contains("failed to infer kube config") {
        "Failed to auto-detect Kubernetes configuration".to_string()
    } else if error_str.contains("timeout") {
        "Operation timed out. Please try again later".to_string()
    } else {
        // 对于其他错误，尝试提取主要消息
        // 去掉常见的类型前缀
        error_str
            .replace("kube::Error(", "")
            .replace("kube::", "")
            .trim()
            .to_string()
    };
    
    cleaned
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
pub struct ServicePort {
    pub port: i64,
    pub target_port: Option<String>, // Can be string or number
    pub protocol: Option<String>,
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_port: Option<i64>, // NodePort for NodePort type services
}

#[derive(Debug, Clone, Serialize)]
pub struct EndpointSlicePort {
    pub port: Option<i64>,
    pub protocol: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EndpointSliceEndpoint {
    pub addresses: Vec<String>,
    pub ready: Option<bool>,
    pub serving: Option<bool>,
    pub terminating: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceSummary {
    pub reference: ResourceRef,
    pub labels: serde_json::Value,
    pub annotations: serde_json::Value,
    pub creation_timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_statuses: Option<Vec<ContainerStatus>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_ports: Option<Vec<ServicePort>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_type: Option<String>,
    // PV fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pv_capacity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pv_access_modes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pv_reclaim_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pv_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pv_claim: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pv_storage_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pv_volume_attributes_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pv_reason: Option<String>,
    // PVC fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pvc_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pvc_volume: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pvc_capacity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pvc_access_modes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pvc_storage_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pvc_volume_attributes_class: Option<String>,
    // EndpointSlice fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpointslice_ports: Option<Vec<EndpointSlicePort>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpointslice_endpoints: Option<Vec<EndpointSliceEndpoint>>,
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
    
    // Extract service ports and type for Services
    let (service_ports, service_type) = if gvk.kind == "Service" && gvk.group.is_empty() {
        let mut ports = Vec::new();
        let mut svc_type: Option<String> = None;
        
        if let Some(spec) = obj.data.get("spec") {
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
    let (pv_capacity, pv_access_modes, pv_reclaim_policy, pv_status, pv_claim, pv_storage_class, pv_volume_attributes_class, pv_reason) = if gvk.kind == "PersistentVolume" && gvk.group.is_empty() {
        let mut capacity: Option<String> = None;
        let mut access_modes: Option<Vec<String>> = None;
        let mut reclaim_policy: Option<String> = None;
        let mut status: Option<String> = None;
        let mut claim: Option<String> = None;
        let mut storage_class: Option<String> = None;
        let mut volume_attributes_class: Option<String> = None;
        let mut reason: Option<String> = None;
        
        if let Some(spec) = obj.data.get("spec") {
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
        
        if let Some(status_obj) = obj.data.get("status") {
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
    let (pvc_status, pvc_volume, pvc_capacity, pvc_access_modes, pvc_storage_class, pvc_volume_attributes_class) = if gvk.kind == "PersistentVolumeClaim" && gvk.group.is_empty() {
        let mut status: Option<String> = None;
        let mut volume: Option<String> = None;
        let mut capacity: Option<String> = None;
        let mut access_modes: Option<Vec<String>> = None;
        let mut storage_class: Option<String> = None;
        let mut volume_attributes_class: Option<String> = None;
        
        if let Some(spec) = obj.data.get("spec") {
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
        
        if let Some(status_obj) = obj.data.get("status") {
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
    let (endpointslice_ports, endpointslice_endpoints) = if gvk.kind == "EndpointSlice" && gvk.group == "discovery.k8s.io" {
        let mut ports = Vec::new();
        let mut endpoints = Vec::new();
        
        // Extract ports
        if let Some(ports_array) = obj.data.get("ports").and_then(|v| v.as_array()) {
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
        if let Some(endpoints_array) = obj.data.get("endpoints").and_then(|v| v.as_array()) {
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
            errors.push(format_error(&e));
            return Ok(K8sOverview {
                crds: vec![],
                resources: vec![],
                errors,
            });
        }
        Err(_) => {
            errors.push("Operation timed out. Please try again later".into());
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
            errors.push(format_error(&e));
            Vec::new()
        }
        Err(_) => {
            errors.push("Operation timed out. Please try again later".into());
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
                        errors.push(format_error(&e));
                    }
                    Err(_) => {
                        errors.push(format!(
                            "Timeout while listing {}.{} {} resources",
                            gvk.kind,
                            gvk.version,
                            if gvk.group.is_empty() { "core" } else { &gvk.group }
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
        .map_err(|_| "Operation timed out. Please try again later".to_string())?
        .map_err(|e| format_error(&e))?;
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
        .map_err(|_| "Operation timed out. Please try again later".to_string())?
        .map_err(|e| format_error(&e))?;
    Ok(serde_json::to_value(&obj).unwrap_or(serde_json::json!({ "error": "Serialization failed" })))
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
        .map_err(|_| "Operation timed out. Please try again later".to_string())?
        .map_err(|e| format_error(&e))?;
    let api: Api<Pod> = Api::namespaced(client, &args.namespace);
    let mut lp = LogParams::default();
    lp.container = args.container.clone();
    lp.tail_lines = args.tail_lines;
    lp.previous = args.previous.unwrap_or(false);
    let dur = Duration::from_secs(args.timeout_secs.unwrap_or(5));
    let logs = timeout(dur, api.logs(&args.name, &lp))
        .await
        .map_err(|_| "Operation timed out. Please try again later".to_string())?
        .map_err(|e| format_error(&e))?;
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
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            
            // Get screen size and set window to 50% of screen, centered
            if let Ok(Some(monitor)) = window.primary_monitor() {
                let screen_size = monitor.size();
                
                // Calculate window size (50% of screen, ensure it fits)
                let max_width = (screen_size.width as f64 - 100.0).max(800.0);
                let max_height = (screen_size.height as f64 - 100.0).max(600.0);
                let width = ((screen_size.width as f64 * 0.5).min(max_width)) as u32;
                let height = ((screen_size.height as f64 * 0.5).min(max_height)) as u32;
                
                // Set max size to prevent window from exceeding screen
                let _ = window.set_max_size(Some(tauri::LogicalSize::new(
                    screen_size.width as u32,
                    screen_size.height as u32,
                )));
                
                // Set window size first
                let _ = window.set_size(tauri::LogicalSize::new(width, height));
                
                // Use spawn to center window after a short delay to ensure it's fully initialized
                let window_clone = window.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(100));
                    let _ = window_clone.center();
                });
            }
            
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Error while running Tauri application");
}

#[tauri::command]
fn copy_text(text: String) -> Result<(), String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("Clipboard error: {}", format_error(&e)))?;
    cb.set_text(text)
        .map_err(|e| format!("Failed to copy to clipboard: {}", format_error(&e)))
}
#[tauri::command]
fn list_contexts() -> Result<serde_json::Value, String> {
    // Try read default kubeconfig and extract contexts
    let cfg = Kubeconfig::read().map_err(|e| format_error(&e))?;
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
        .map_err(|_| "Settings lock failed".to_string())?;
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
        .map_err(|_| "Settings lock failed".to_string())?
        .watcher_cancels
        .push(token.clone());
    let current_ctx = {
        settings
            .lock()
            .map_err(|_| "Settings lock failed".to_string())?
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
        .map_err(|_| "Settings lock failed".to_string())?;
    for tok in s.watcher_cancels.drain(..) {
        tok.cancel();
    }
    Ok(())
}
