function getInvoke() {
  const tauri = window.__TAURI__ && window.__TAURI__.core;
  if (!tauri || !tauri.invoke) {
    throw new Error("Tauri core is not ready yet");
  }
  return tauri.invoke;
}

export async function getK8sResources(kind, namespace = null) {
  const invoke = getInvoke();
  return await invoke("get_k8s_resources", { kind, namespace });
}

export async function getK8sObject(kind, name, namespace = null, format = "yaml") {
  const invoke = getInvoke();
  return await invoke("get_k8s_object", { kind, name, namespace, format });
}

export async function getPodLogs(pod, namespace, tail = 500) {
  const invoke = getInvoke();
  return await invoke("get_pod_logs", { pod, namespace, tail });
}


