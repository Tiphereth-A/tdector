//! Shared native runner for ordinary desktop launches and opt-in controlled hosts.
use crate::consts::ui::{WINDOW_HEIGHT, WINDOW_WIDTH};
use crate::ui::DecryptionApp;
use eframe::egui;

fn options(width: Option<f32>, height: Option<f32>, controlled: bool) -> eframe::NativeOptions {
    let mut viewport = egui::ViewportBuilder::default().with_inner_size([
        width.unwrap_or(WINDOW_WIDTH),
        height.unwrap_or(WINDOW_HEIGHT),
    ]);
    if let Some(icon) = load_app_icon() {
        viewport = viewport.with_icon(icon);
    }
    eframe::NativeOptions {
        viewport,
        persist_window: !controlled,
        ..Default::default()
    }
}
pub fn run() -> eframe::Result<()> {
    let _ = env_logger::try_init();
    eframe::run_native(
        "Text Decryption Helper",
        options(None, None, false),
        Box::new(|cc| Ok(DecryptionApp::new(cc))),
    )
}
fn load_app_icon() -> Option<egui::IconData> {
    let image = image::load_from_memory(include_bytes!("../assets/favicon.ico"))
        .ok()?
        .into_rgba8();
    let (width, height) = image.dimensions();
    Some(egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}

#[cfg(feature = "gui-control")]
mod controlled {
    use super::*;
    use crate::control::protocol::{ControlError, PROTOCOL_VERSION, Result, StartupConfig};
    use crate::control::screenshot::ScreenshotCapture;
    use crate::control::transport::{HostBridge, PendingRequest};
    use serde_json::{Value, json};
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::{Duration, Instant};

    type Ready = Arc<Mutex<Option<Box<dyn FnOnce(Result<Value>) + Send>>>>;
    fn announce(ready: &Ready, result: Result<Value>) {
        if let Ok(mut callback) = ready.lock()
            && let Some(callback) = callback.take()
        {
            callback(result);
        }
    }
    struct RenderReceipt {
        request: PendingRequest,
        result: Value,
        observed_frame: Option<u64>,
    }
    struct ControlledApp {
        app: DecryptionApp,
        bridge: HostBridge,
        ready: Ready,
        first_frame: Option<u64>,
        announced: bool,
        pending: Vec<RenderReceipt>,
        screenshots: ScreenshotCapture,
        closing_reply: Option<Arc<AtomicBool>>,
    }
    impl eframe::App for ControlledApp {
        fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
            // A subsequent frame's logic runs after the backend rendered the previous UI frame.
            self.screenshots.poll();
            let current_frame = ctx.cumulative_frame_nr();
            if !self.announced && self.first_frame.is_some_and(|frame| current_frame > frame) {
                self.announced = true;
                match self.app.control_execute(ctx, "status", json!({})) {
                    Ok(status) => announce(
                        &self.ready,
                        Ok(
                            json!({"session":self.bridge.session,"main_handle":status["main_handle"],"protocol_version":PROTOCOL_VERSION}),
                        ),
                    ),
                    Err(error) => {
                        announce(&self.ready, Err(error));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
            let pending = std::mem::take(&mut self.pending);
            for mut receipt in pending {
                if receipt.request.expired() {
                    receipt.request.respond(Err(ControlError::session(
                        "outcome_unknown",
                        "State was applied, but no rendered frame was observed before the deadline",
                    )));
                } else if receipt
                    .observed_frame
                    .is_some_and(|frame| current_frame > frame)
                {
                    if receipt.request.command == "wait" {
                        receipt.result = json!({"rendered":true,"frame":receipt.observed_frame});
                    } else if let Some(handle) = receipt
                        .result
                        .get("handle")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        && let Ok(inspected) =
                            self.app
                                .control_execute(ctx, "inspect", json!({"handle":handle}))
                    {
                        if let Some(geometry) = inspected.get("geometry") {
                            receipt.result["geometry"] = geometry.clone();
                        }
                        receipt.result["rendered"] = json!(true);
                    }
                    receipt.request.respond(Ok(receipt.result));
                } else {
                    self.pending.push(receipt);
                }
            }
            if let Some(written) = &self.closing_reply {
                if written.load(Ordering::Acquire) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                } else {
                    ctx.request_repaint_after(Duration::from_millis(5));
                }
                return;
            }
            let started = Instant::now();
            for _ in 0..16 {
                if started.elapsed() > Duration::from_millis(8) {
                    break;
                }
                let Some(request) = self.bridge.try_recv() else {
                    break;
                };
                // Even cancelled entries consume this update's budget; wake again to drain later requests.
                ctx.request_repaint();
                if !request.begin() {
                    request.respond(Err(ControlError::session(
                        "timeout",
                        "Command expired in the queue and was not executed",
                    )));
                    continue;
                }
                if request.command == "screenshot" {
                    self.screenshots.enqueue(ctx, request);
                    continue;
                }
                if request.command == "wait" {
                    self.pending.push(RenderReceipt {
                        request,
                        result: Value::Null,
                        observed_frame: None,
                    });
                    ctx.request_repaint();
                    continue;
                }
                if matches!(request.command.as_str(), "open" | "focus") {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                }
                let result = self
                    .app
                    .control_execute(ctx, &request.command, request.args.clone());
                match result {
                    Ok(result)
                        if self.app.closing
                            || result.get("closed").and_then(Value::as_bool) == Some(true)
                                && result.get("main").and_then(Value::as_bool) == Some(true) =>
                    {
                        self.closing_reply = Some(request.written.clone());
                        request.respond(Ok(result));
                        ctx.request_repaint();
                        break;
                    }
                    Ok(result)
                        if matches!(request.command.as_str(), "open" | "move" | "resize") =>
                    {
                        self.pending.push(RenderReceipt {
                            request,
                            result,
                            observed_frame: None,
                        });
                        ctx.request_repaint();
                    }
                    result => request.respond(result),
                }
            }
            // A bounded drain yields to painting, but must continue even when minimized.
            if !self.pending.is_empty() || self.screenshots.is_pending() {
                ctx.request_repaint_after(Duration::from_millis(50));
            }
        }
        fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
            self.app.ui(ui, frame);
            if ui.ctx().will_discard() {
                return;
            }
            let frame_nr = ui.ctx().cumulative_frame_nr();
            if self.first_frame.is_none() {
                self.first_frame = Some(frame_nr);
                ui.ctx().request_repaint();
            }
            for receipt in &mut self.pending {
                if receipt.observed_frame.is_none() {
                    receipt.observed_frame = Some(
                        frame_nr
                            + u64::from(matches!(
                                receipt.request.command.as_str(),
                                "open" | "move" | "resize"
                            )),
                    );
                    ui.ctx().request_repaint();
                }
            }
        }
        fn persist_egui_memory(&self) -> bool {
            false
        }
    }

    pub fn run_controlled(
        config: StartupConfig,
        ready: impl FnOnce(Result<Value>) + Send + 'static,
    ) -> Result<()> {
        let _ = env_logger::try_init();
        let ready: Ready = Arc::new(Mutex::new(Some(Box::new(ready))));
        let bridge = match HostBridge::bind() {
            Ok(bridge) => bridge,
            Err(error) => {
                announce(&ready, Err(error.clone()));
                return Err(error);
            }
        };
        // eframe's persist_window flag controls saving only: its runner still
        // reads persisted window geometry and zoom before calling app creation.
        // Use a fresh file path so ordinary desktop state cannot override startup
        // dimensions or presentation. With both persistence hooks disabled this
        // file is never written, and the nonce is unrelated to the session token.
        let mut nonce = [0_u8; 16];
        if let Err(error) = getrandom::fill(&mut nonce) {
            let error = ControlError::session("startup_failed", error.to_string());
            announce(&ready, Err(error.clone()));
            return Err(error);
        }
        let nonce: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
        let mut native_options = options(config.width, config.height, true);
        let persistence_path =
            std::env::temp_dir().join(format!("tdector-gui-control-{nonce}.ron"));
        native_options.persistence_path = Some(persistence_path.clone());
        let startup_ready = ready.clone();
        let result = eframe::run_native(
            "Text Decryption Helper",
            native_options,
            Box::new(move |cc| {
                // eframe's ordinary persisted egui memory must never influence a reproduction.
                cc.egui_ctx
                    .memory_mut(|memory| *memory = egui::Memory::default());
                let mut app = DecryptionApp::new_controlled(cc);
                let initialize = (|| -> Result<()> {
                    if let Some(path) = config.project {
                        app.control_execute(&cc.egui_ctx, "load", json!({"path":path}))?;
                    }
                    if let Some(path) = config.font {
                        app.control_execute(&cc.egui_ctx, "font", json!({"path":path}))?;
                    }
                    Ok(())
                })();
                if let Err(error) = initialize {
                    announce(&startup_ready, Err(error.clone()));
                    return Err(Box::new(error));
                }
                bridge.set_context(&cc.egui_ctx);
                Ok(Box::new(ControlledApp {
                    app,
                    bridge,
                    ready: startup_ready,
                    first_frame: None,
                    announced: false,
                    pending: Vec::new(),
                    screenshots: ScreenshotCapture::default(),
                    closing_reply: None,
                }))
            }),
        );
        // Delete only this host's isolated file if a future eframe version writes it.
        let _ = std::fs::remove_file(&persistence_path);
        match result {
            Ok(()) => {
                announce(
                    &ready,
                    Err(ControlError::session(
                        "startup_failed",
                        "GUI closed before its first rendered frame",
                    )),
                );
                Ok(())
            }
            Err(error) => {
                let error = ControlError::session("startup_failed", error.to_string());
                announce(&ready, Err(error.clone()));
                Err(error)
            }
        }
    }
}
#[cfg(feature = "gui-control")]
pub use controlled::run_controlled;
