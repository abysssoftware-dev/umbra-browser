use mlua::{Lua, Table};
use serde::Serialize;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

/// Declared capabilities from a plugin manifest. No filesystem/network access
/// unless explicitly granted - the sandbox is built around this.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Capabilities {
    #[serde(default)]
    pub fs: bool,
    #[serde(default)]
    pub net: bool,
    #[serde(default)]
    pub ui: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    /// plugin | theme | layout
    #[serde(rename = "type")]
    pub kind: String,
    pub entry: String,
    #[serde(default)]
    pub iface_version: u32,
    #[serde(default)]
    pub capabilities: Capabilities,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub status: String,
    pub errors: Vec<String>,
}

pub struct PluginHost {
    pub loaded: Vec<LoadedPlugin>,
}

/// Cap on plugin net response bodies (mirrors the old ai.rs fetch rule).
const MAX_NET_BODY_BYTES: usize = 5 * 1024 * 1024;

impl Default for PluginHost {
    fn default() -> Self {
        PluginHost { loaded: vec![] }
    }
}

/// Lenient TOML manifest parser (also reused by the .umbrapkg pipeline).
pub fn parse_manifest(raw: &str) -> Result<PluginManifest, String> {
    let mut name = None;
    let mut version = String::from("0.0.0");
    let mut kind = String::from("plugin");
    let mut entry = String::from("main.lua");
    let mut iface_version = crate::slots::SLOT_INTERFACE_VERSION;
    let mut caps = Capabilities::default();
    let mut in_caps = false;
    for line in raw.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_caps = line == "[capabilities]";
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let k = k.trim();
        let v = v.trim().trim_matches('"');
        match (k, in_caps) {
            ("name", false) => name = Some(v.to_string()),
            ("version", false) => version = v.to_string(),
            ("type", false) => kind = v.to_string(),
            ("entry", false) => entry = v.to_string(),
            ("interface_version", false) | ("iface_version", false) => {
                iface_version = v.parse().unwrap_or(iface_version)
            }
            ("fs", true) => caps.fs = v == "true",
            ("net", true) => caps.net = v == "true",
            ("ui", true) => caps.ui = v == "true",
            _ => {}
        }
    }
    Ok(PluginManifest {
        name: name.ok_or("manifest.toml missing 'name'")?,
        version,
        kind,
        entry,
        iface_version,
        capabilities: caps,
    })
}

impl PluginHost {
    /// Load every enabled plugin from plugins/<name>/. Lua plugins run in a
    /// sandboxed VM; `.wasm` entries are routed to the wasmtime runner
    /// (wasm_plugins.rs). Returns per-plugin warnings for the console.
    pub fn load_all(
        &mut self,
        plugins_dir: PathBuf,
        enabled: &[String],
        registry: &mut crate::slots::SlotRegistry,
    ) -> Vec<String> {
        let mut warnings = Vec::new();
        self.loaded.clear();
        for dir_name in enabled {
            let pdir = plugins_dir.join(dir_name);
            let mf_path = pdir.join("manifest.toml");
            let Ok(mf_raw) = fs::read_to_string(&mf_path) else {
                warnings.push(format!("plugin '{dir_name}': manifest.toml not found"));
                continue;
            };
            let manifest = match parse_manifest(&mf_raw) {
                Ok(m) => m,
                Err(e) => {
                    warnings.push(format!("plugin '{dir_name}': {e}"));
                    continue;
                }
            };
            if manifest.kind != "plugin" {
                warnings.push(format!(
                    "'{dir_name}' is of type '{}' - only 'plugin' is executable here",
                    manifest.kind
                ));
                continue;
            }
            let script_path = pdir.join(&manifest.entry);

            // WASM tier: .wasm entries run via wasmtime instead of Lua.
            if manifest.entry.ends_with(".wasm") {
                match fs::read(&script_path) {
                    Ok(bytes) => match crate::wasm_plugins::run(&manifest, &bytes) {
                        Ok(mut w) => {
                            warnings.append(&mut w);
                            self.loaded.push(LoadedPlugin {
                                manifest,
                                status: "loaded".into(),
                                errors: vec![],
                            });
                        }
                        Err(e) => {
                            warnings.push(format!("plugin '{}': {e}", manifest.name));
                            self.loaded.push(LoadedPlugin {
                                manifest,
                                status: "error".into(),
                                errors: vec![e],
                            });
                        }
                    },
                    Err(_) => warnings.push(format!(
                        "plugin '{}': cannot read entry {}",
                        manifest.name,
                        script_path.display()
                    )),
                }
                continue;
            }

            let Ok(script) = fs::read_to_string(&script_path) else {
                warnings.push(format!(
                    "plugin '{}': cannot read entry {}",
                    manifest.name,
                    script_path.display()
                ));
                continue;
            };

            match run_sandboxed(&manifest, &script, registry) {
                Ok(mut w) => {
                    warnings.append(&mut w);
                    self.loaded.push(LoadedPlugin {
                        manifest,
                        status: "loaded".into(),
                        errors: vec![],
                    });
                }
                Err(e) => {
                    warnings.push(format!("plugin '{}': {e}", manifest.name));
                    self.loaded.push(LoadedPlugin {
                        manifest,
                        status: "error".into(),
                        errors: vec![e],
                    });
                }
            }
        }
        warnings
    }
}

/// Sandboxed Lua execution: no io/os access unless granted, memory-capped.
/// Slot registrations are collected via a 'static channel and applied to the
/// registry after the script finishes.
fn run_sandboxed(
    manifest: &PluginManifest,
    script: &str,
    registry: &mut crate::slots::SlotRegistry,
) -> Result<Vec<String>, String> {
    let lua = Lua::new();
    // Per-plugin resource cap: 32 MiB of Lua heap.
    lua.set_memory_limit(32 * 1024 * 1024)
        .map_err(|e| e.to_string())?;

    let globals = lua.globals();

    // Strip dangerous globals; grant back only what the capability manifest allows.
    {
        let os: Table = globals.get::<Table>("os").map_err(|e| e.to_string())?;
        // keep only clock/time for timing, remove execution + env
        for key in ["execute", "exit", "getenv", "remove", "rename", "setenv", "tmpname"] {
            os.set(key, mlua::Value::Nil).ok();
        }
        if !manifest.capabilities.fs {
            globals.set("io", mlua::Value::Nil).ok();
            os.set("open", mlua::Value::Nil).ok();
        }
        globals.set("require", mlua::Value::Nil).ok();
        globals.set("dofile", mlua::Value::Nil).ok();
        globals.set("loadfile", mlua::Value::Nil).ok();
    }

    // umbra.* API surface exposed to plugins.
    let umbra = lua.create_table().map_err(|e| e.to_string())?;
    let mut warnings: Vec<String> = Vec::new();

    let pending: Arc<Mutex<Vec<(String, String, u32)>>> = Arc::new(Mutex::new(Vec::new()));
    let pending_in_fn = pending.clone();
    let plugin_name = manifest.name.clone();
    let reg_slot = lua
        .create_function_mut(
            move |_lua, args: Table| -> mlua::Result<()> {
                let component: String = args.get("component")?;
                let slot: String = args.get("slot")?;
                let iface: Option<u32> = args.get("interface_version").ok();
                pending_in_fn
                    .lock()
                    .map_err(|_| mlua::Error::RuntimeError("lock poisoned".into()))?
                    .push((
                        component,
                        slot,
                        iface.unwrap_or(crate::slots::SLOT_INTERFACE_VERSION),
                    ));
                Ok(())
            },
        )
        .map_err(|e| e.to_string())?;
    drop(plugin_name);
    umbra.set("register_component", reg_slot).map_err(|e| e.to_string())?;

    let log_sink: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let log_sink_in_fn = log_sink.clone();
    let log = lua
        .create_function(move |_, msg: String| {
            println!("[umbra:plugin] {msg}");
            // Plugin logs are surfaced to the frontend via the umbra://console
            // event stream (lib.rs / watcher emit the returned vec).
            if let Ok(mut sink) = log_sink_in_fn.lock() {
                sink.push(msg);
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?;
    umbra.set("log", log).map_err(|e| e.to_string())?;
    umbra
        .set("interface_version", crate::slots::SLOT_INTERFACE_VERSION)
        .map_err(|e| e.to_string())?;

    // Capability-gated host APIs. Without the grant the tables simply never
    // exist (nil) - that IS the sandbox guarantee.
    if manifest.capabilities.net {
        let net = lua.create_table().map_err(|e| e.to_string())?;
        let request = lua
            .create_function(move |lua, args: Table| -> mlua::Result<mlua::Table> {
                let url: String = args.get("url")?;
                let method: Option<String> = args.get("method").unwrap_or(None);
                let body: Option<String> = args.get("body").unwrap_or(None);
                plugin_net_request(lua, &url, method.as_deref(), body.as_deref())
            })
            .map_err(|e| e.to_string())?;
        net.set("request", request).map_err(|e| e.to_string())?;
        umbra.set("net", net).map_err(|e| e.to_string())?;
    }

    // Publish the umbra.* table only after capability-gated hosts are attached.
    globals.set("umbra", umbra).map_err(|e| e.to_string())?;

    // Per-plugin CPU cap: instruction-counting debug hook. The hook fires
    // every 100k VM instructions and errors out once the cumulative budget
    // (50M instructions) is exceeded, so runaway plugins can't peg a core.
    const HOOK_TICK: u32 = 100_000;
    const INSTRUCTION_BUDGET: u64 = 50_000_000;
    let used = Arc::new(std::sync::atomic::AtomicU64::new(0));
    {
        let used = used.clone();
        lua.set_hook(
            mlua::HookTriggers::new().every_nth_instruction(HOOK_TICK),
            move |_lua, _dbg| {
                let n = used.fetch_add(HOOK_TICK as u64, Ordering::Relaxed) + HOOK_TICK as u64;
                if n > INSTRUCTION_BUDGET {
                    Err(mlua::Error::RuntimeError(format!(
                        "plugin exceeded CPU budget ({INSTRUCTION_BUDGET} instructions)"
                    )))
                } else {
                    Ok(mlua::VmState::Continue)
                }
            },
        );
    }

    let chunk = lua
        .load(script)
        .set_name(manifest.entry.clone())
        .into_function()
        .map_err(|e| e.to_string())?;
    let result = chunk.call::<()>(());
    let init_result = match &result {
        Ok(()) => match globals.get::<mlua::Function>("init") {
            Ok(init) => init.call::<()>(()).map_err(|e| format!("init(): {e}")),
            _ => Ok(()),
        },
        Err(e) => Err(format!("lua error: {e}")),
    };
    // Hook only guards load-time execution; remove before dropping the VM.
    lua.remove_hook();
    init_result?;

    // Collect plugin log lines into the console vec returned to the caller.
    if let Ok(sink) = log_sink.lock() {
        for msg in sink.iter() {
            warnings.push(format!("[{}] {msg}", manifest.name));
        }
    }

    // Apply collected slot registrations now that the VM is done with them.
    if let Ok(queue) = pending.lock() {
        for (component, slot, iface) in queue.iter() {
            for w in registry.mount(
                component.clone(),
                slot.clone(),
                *iface,
                manifest.name.clone(),
            ) {
                warnings.push(format!("plugin '{}': {w}", manifest.name));
            }
        }
    }

    Ok(warnings)
}

/// umbra.net.request{url, method?, body?} → {status, body}.
/// 10s timeout, 5 MiB response cap, http(s) only - mirrors ai.rs fetch rules.
fn plugin_net_request(
    lua: &Lua,
    url: &str,
    method: Option<&str>,
    body: Option<&str>,
) -> mlua::Result<mlua::Table> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(mlua::Error::RuntimeError(
            "umbra.net.request: only http(s) URLs are allowed".into(),
        ));
    }
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(10))
        .build();
    let send = || -> Result<ureq::Response, String> {
        let m = method.unwrap_or(if body.is_some() { "POST" } else { "GET" }).to_uppercase();
        match (m.as_str(), body) {
            ("GET", _) => agent.get(url).call().map_err(|e| e.to_string()),
            ("HEAD", _) => agent.head(url).call().map_err(|e| e.to_string()),
            ("DELETE", _) => agent.delete(url).call().map_err(|e| e.to_string()),
            // ureq surfaces non-2xx as Error::Status(code, response) - that is
            // still a successful request for the plugin API.
            (_, b) => {
                let req = if m == "PUT" { agent.put(url) } else { agent.post(url) };
                match req.send_string(b.unwrap_or_default()) {
                    Ok(r) => Ok(r),
                    Err(ureq::Error::Status(_, r)) => Ok(r),
                    Err(e) => Err(e.to_string()),
                }
            }
        }
    };
    let (status, body_text) = match send() {
        Err(e) => {
            return Err(mlua::Error::RuntimeError(format!("umbra.net.request failed: {e}")));
        }
        Ok(resp) => {
            let status = resp.status() as i32;
            let mut buf = Vec::new();
            let read = resp
                .into_reader()
                .take(MAX_NET_BODY_BYTES as u64)
                .read_to_end(&mut buf);
            match read {
                Ok(_) => (status, String::from_utf8_lossy(&buf).into_owned()),
                Err(e) => {
                    return Err(mlua::Error::RuntimeError(format!(
                        "umbra.net.request read failed: {e}"
                    )));
                }
            }
        }
    };
    let out = lua.create_table()?;
    out.set("status", status)?;
    out.set("body", body_text)?;
    Ok(out)
}
