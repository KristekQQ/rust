use ndk::looper::{Poll, ThreadLooper};
use std::{ffi::CString, io::Read, time::Duration};
#[ndk_glue::main]
fn main() {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("KristekRim"),
    );
    std::panic::set_hook(Box::new(|info| log::error!("Native panic: {info}")));
    let read = |name: &str| -> Result<Vec<u8>, String> {
        let manager = ndk_glue::native_activity().asset_manager();
        let name = CString::new(format!("rim/{name}")).map_err(|e| e.to_string())?;
        let mut asset = manager
            .open(&name)
            .ok_or_else(|| format!("Missing APK asset {name:?}"))?;
        let mut bytes = Vec::new();
        asset.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        Ok(bytes)
    };
    let looper = ThreadLooper::for_thread().expect("NativeActivity looper");
    let mut destroyed = false;
    let mut alive = || {
        // poll_events reads a blocking pipe: call it only after readiness.
        for _ in 0..128 {
            match looper.poll_once_timeout(Duration::ZERO) {
                Ok(Poll::Event {
                    ident: ndk_glue::NDK_GLUE_LOOPER_EVENT_PIPE_IDENT,
                    ..
                }) => {
                    if ndk_glue::poll_events() == Some(ndk_glue::Event::Destroy) {
                        destroyed = true;
                        return false;
                    }
                }
                Ok(Poll::Event {
                    ident: ndk_glue::NDK_GLUE_LOOPER_INPUT_QUEUE_IDENT,
                    ..
                }) => {
                    if let Some(queue) = ndk_glue::input_queue() {
                        while let Ok(Some(event)) = queue.get_event() {
                            if let Some(event) = queue.pre_dispatch(event) {
                                queue.finish_event(event, false);
                            }
                        }
                    }
                }
                Ok(Poll::Timeout) => break,
                Err(e) => {
                    log::error!("Android event loop: {e}");
                    return false;
                }
                _ => {}
            }
        }
        true
    };
    match crate::xr::run(&read, &mut alive) {
        Ok(()) => log::info!("OpenXR session finished"),
        Err(e) => log::error!("OpenXR startup/frame failed: {e}"),
    }
    if !destroyed {
        ndk_glue::native_activity().finish();
    }
}
