//! WASM plugin tier (pass-2) - wasmtime-backed execution for plugins whose
//! manifest `entry` ends in ".wasm". Kept behind this narrow surface so the
//! build stays sane.
//!
//! # Plugin ABI
//!
//! Exported by the module:
//! - `umbra_main()` - entry point (preferred); `_start` accepted as a
//!   WASI-style fallback. Anything else is a load error.
//! - `memory` - linear memory (the standard wasm export name).
//!
//! Imported host functions, namespace `env`:
//!
//! - `umbra_log(ptr: i32, len: i32)`
//!   Always linked. Reads UTF-8 bytes at `[ptr, ptr+len)` from linear memory
//!   and forwards them to the same plugin console sink the Lua host uses.
//!   Out-of-range reads are ignored (no trap from the host side).
//!
//! - `umbra_net_request(url_ptr: i32, url_len: i32, out_ptr: i32) -> i32`
//!   Linked ONLY when the manifest grants `net = true`. Performs an HTTP GET
//!   with a 10s timeout.
//!     * `[url_ptr, url_ptr+url_len)` = UTF-8 http(s) URL.
//!     * `out_ptr` points to a guest-provided request block laid out as:
//!         `[out_ptr       .. out_ptr+4      )` i32 LE capacity of the body buffer,
//!         `[out_ptr + 4   .. out_ptr+4+cap  )` body buffer written by the host.
//!     * Returns bytes written (>= 0), or a negative error code:
//!         `-1` invalid memory range / no linear memory,
//!         `-2` non-http(s) URL,
//!         `-3` transport failure,
//!         `-4` response exceeds the hard 5 MiB cap.
//!       Responses larger than the guest capacity (but within the hard cap)
//!       are truncated to the capacity.
//!
//! Linked ONLY when `fs = true`: WASI preview1 - not wired up; such plugins
//! get a clear warning and are skipped rather than breaking the build.
//!
//! CPU cap: the engine runs with `consume_fuel(true)`; each invocation gets
//! FUEL_BUDGET units. Exhaustion aborts with "plugin exceeded CPU budget".
//! Network responses are capped at 5 MiB, mirroring the Lua sandbox.

use std::io::Read;
use std::sync::{Arc, Mutex};
use wasmtime::{Caller, Config, Engine, Linker, Memory, Module, Store};

const FUEL_BUDGET: u64 = 100_000_000;
const MAX_NET_BODY: usize = 5 * 1024 * 1024;
const NET_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

fn memory_of(caller: &mut Caller<'_, ()>) -> Option<Memory> {
    caller.get_export("memory")?.into_memory()
}

/// Read a UTF-8 string out of guest memory (NUL-trimmed).
fn read_mem(
    mem: &Memory,
    ctx: impl wasmtime::AsContextMut,
    ptr: i32,
    len: i32,
) -> Option<String> {
    if ptr < 0 || len < 0 {
        return None;
    }
    let start = ptr as usize;
    if start.checked_add(len as usize).is_none() {
        return None;
    }
    let mut buf = vec![0u8; len as usize];
    mem.read(ctx, start, &mut buf).ok()?;
    String::from_utf8(buf)
        .ok()
        .map(|s| s.trim_end_matches('\0').to_string())
}

/// Run one wasm plugin to completion of its entry point. Returns console /
/// warning lines exactly like the Lua path does.
pub fn run(manifest: &crate::plugins::PluginManifest, wasm_bytes: &[u8]) -> Result<Vec<String>, String> {
    let mut cfg = Config::new();
    cfg.consume_fuel(true);
    let engine = Engine::new(&cfg).map_err(|e| format!("wasmtime init failed: {e}"))?;
    let module =
        Module::from_binary(&engine, wasm_bytes).map_err(|e| format!("wasm compile failed: {e}"))?;

    // Shared console sink → same collection format as Lua plugin logs.
    let logs: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let mut warnings: Vec<String> = Vec::new();

    let mut linker = Linker::<()>::new(&engine);

    {
        let logs = logs.clone();
        linker
            .func_wrap(
                "env",
                "umbra_log",
                move |mut caller: Caller<'_, ()>, ptr: i32, len: i32| {
                    if let Some(mem) = memory_of(&mut caller) {
                        if let Some(msg) = read_mem(&mem, &mut caller, ptr, len) {
                            println!("[umbra:plugin] {msg}");
                            if let Ok(mut sink) = logs.lock() {
                                sink.push(msg);
                            }
                        }
                    }
                },
            )
            .map_err(|e| format!("cannot link umbra_log: {e}"))?;
    }

    if manifest.capabilities.net {
        linker
            .func_wrap(
                "env",
                "umbra_net_request",
                move |mut caller: Caller<'_, ()>,
                      url_ptr: i32,
                      url_len: i32,
                      out_ptr: i32|
                      -> i32 {
                    let Some(mem) = memory_of(&mut caller) else { return -1 };
                    let Some(url) = read_mem(&mem, &mut caller, url_ptr, url_len) else { return -1 };
                    if !url.starts_with("http://") && !url.starts_with("https://") {
                        return -2;
                    }
                    // Guest-declared capacity lives at [out_ptr, out_ptr+4).
                    let mut capbuf = [0u8; 4];
                    if mem.read(&mut caller, out_ptr.max(0) as usize, &mut capbuf).is_err() {
                        return -1;
                    }
                    let cap = i32::from_le_bytes(capbuf).max(0) as usize;
                    let base = match (out_ptr as usize).checked_add(4) {
                        Some(b) => b,
                        None => return -1,
                    };

                    let agent = ureq::AgentBuilder::new().timeout(NET_TIMEOUT).build();
                    let fetched = agent.get(&url).call();
                    let resp = match fetched {
                        Ok(r) => r,
                        Err(_) => return -3,
                    };
                    let mut body = Vec::new();
                    if resp
                        .into_reader()
                        .take(MAX_NET_BODY as u64 + 1)
                        .read_to_end(&mut body)
                        .is_err()
                    {
                        return -3;
                    }
                    if body.len() > MAX_NET_BODY {
                        return -4;
                    }
                    let n = body.len().min(cap);
                    mem.write(&mut caller, base, &body[..n])
                        .map(|_| n as i32)
                        .unwrap_or(-1)
                },
            )
            .map_err(|e| format!("cannot link umbra_net_request: {e}"))?;
    }

    if manifest.capabilities.fs {
        warnings.push(format!(
            "plugin '{}': fs capability requires WASI support - not linked",
            manifest.name
        ));
    }

    let mut store = Store::new(&engine, ());
    store
        .set_fuel(FUEL_BUDGET)
        .map_err(|e| format!("cannot set fuel budget: {e}"))?;

    let instance = linker
        .instantiate(&mut store, &module)
        .map_err(|e| format!("wasm instantiate failed: {e}"))?;

    let entry = if let Ok(f) = instance.get_typed_func::<(), ()>(&mut store, "umbra_main") {
        f
    } else if let Ok(f) = instance.get_typed_func::<(), ()>(&mut store, "_start") {
        f
    } else {
        return Err("module exports neither umbra_main() nor _start".into());
    };

    if let Err(err) = entry.call(&mut store, ()) {
        let fuel_out = matches!(err.downcast_ref::<wasmtime::Trap>(), Some(wasmtime::Trap::OutOfFuel))
            || err.to_string().contains("fuel");
        if fuel_out {
            return Err("plugin exceeded CPU budget".into());
        }
        return Err(format!("wasm trap: {err}"));
    }

    if let Ok(sink) = logs.lock() {
        for msg in sink.iter() {
            warnings.push(format!("[{}] {msg}", manifest.name));
        }
    }
    Ok(warnings)
}
