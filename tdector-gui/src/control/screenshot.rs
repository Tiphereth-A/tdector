//! Captures the application's rendered framebuffer through eframe's public screenshot API.
use super::protocol::{ControlError, Result};
use super::transport::PendingRequest;
use eframe::egui;
use image::ImageEncoder;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};

struct Capture {
    request: PendingRequest,
    output: PathBuf,
    overwrite: bool,
    image: mpsc::Receiver<Arc<egui::ColorImage>>,
}
#[derive(Default)]
pub(crate) struct ScreenshotCapture {
    next: u64,
    pending: BTreeMap<u64, Capture>,
}
impl ScreenshotCapture {
    pub(crate) fn is_pending(&self) -> bool {
        !self.pending.is_empty()
    }
    pub(crate) fn enqueue(&mut self, ctx: &egui::Context, request: PendingRequest) {
        let Some(output) = request
            .args
            .get("output")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
        else {
            request.respond(Err(ControlError::invalid(
                "A screenshot output path is required",
            )));
            return;
        };
        let overwrite = request
            .args
            .get("overwrite")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if output.exists() && !overwrite {
            request.respond(Err(ControlError::new(
                "output_exists",
                "Screenshot output already exists; use --overwrite to replace it",
                6,
            )));
            return;
        }
        let id = self.next;
        self.next += 1;
        let (send, image) = mpsc::sync_channel(1);
        self.pending.insert(
            id,
            Capture {
                request,
                output,
                overwrite,
                image,
            },
        );
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        let repaint = ctx.clone();
        ctx.request_screenshot(move |image| {
            if send.send(image).is_ok() {
                repaint.request_repaint();
            }
        });
        ctx.request_repaint();
    }
    pub(crate) fn poll(&mut self) {
        let frames = self
            .pending
            .iter()
            .filter_map(|(id, capture)| capture.image.try_recv().ok().map(|image| (*id, image)))
            .collect::<Vec<_>>();
        for (id, image) in frames {
            if let Some(capture) = self.pending.remove(&id) {
                if capture.request.expired() {
                    capture.request.respond(Err(ControlError::session(
                        "timeout",
                        "Screenshot expired before capture completed; the output was not written",
                    )));
                    continue;
                }
                let result = save_png(&image, &capture.output, capture.overwrite);
                capture.request.respond(result);
            }
        }
        let expired = self
            .pending
            .iter()
            .filter(|(_, capture)| capture.request.expired())
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        for id in expired {
            if let Some(capture) = self.pending.remove(&id) {
                capture.request.respond(Err(ControlError::session("timeout", "No rendered screenshot arrived before the deadline; the output was not written")));
            }
        }
    }
}
fn save_png(image: &egui::ColorImage, output: &std::path::Path, overwrite: bool) -> Result<Value> {
    let width = u32::try_from(image.size[0])
        .map_err(|_| ControlError::invalid("Screenshot width is too large"))?;
    let height = u32::try_from(image.size[1])
        .map_err(|_| ControlError::invalid("Screenshot height is too large"))?;
    let rgba = image
        .pixels
        .iter()
        .flat_map(|pixel| pixel.to_srgba_unmultiplied())
        .collect::<Vec<_>>();
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(&rgba, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|error| ControlError::new("image_error", error.to_string(), 5))?;
    tdector_io::atomic_write(output, &png, overwrite, None).map_err(|error| {
        let conflict = matches!(error, tdector_io::IoError::OutputExists { .. });
        ControlError::new(
            if conflict {
                "output_exists"
            } else {
                "io_error"
            },
            error.to_string(),
            if conflict { 6 } else { 5 },
        )
    })?;
    Ok(json!({"output":output,"format":"png","width":width,"height":height,"rendered":true}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn png_capture_preserves_pixels_and_refuses_unrequested_overwrite() {
        let path = std::env::temp_dir().join(format!(
            "tdector-capture-{}-{}.png",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let image = egui::ColorImage::new([2, 1], vec![egui::Color32::RED, egui::Color32::BLUE]);
        let result = save_png(&image, &path, false).unwrap();
        assert_eq!(result["width"], 2);
        let bytes = std::fs::read(&path).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert_eq!(decoded.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(decoded.get_pixel(1, 0).0, [0, 0, 255, 255]);
        assert_eq!(
            save_png(&image, &path, false).unwrap_err().code,
            "output_exists"
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        save_png(&image, &path, true).unwrap();
        std::fs::remove_file(&path).unwrap();
    }
}
