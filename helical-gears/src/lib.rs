#[cfg(any(target_arch = "wasm32", test))]
mod gear;
#[cfg(any(target_arch = "wasm32", test))]
mod inputs;
#[cfg(target_arch = "wasm32")]
mod plugin;
