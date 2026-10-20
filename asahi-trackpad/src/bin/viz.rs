use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use eframe::egui;
use evdev::{AbsoluteAxisType, Device, InputEventKind};

#[derive(Clone, Debug, Default)]
struct TouchPoint {
    id: i32,
    x: f32,
    y: f32,
    pressure: f32,
    size: f32,
}

#[derive(Clone, Debug)]
struct Ranges {
    min_x: f32, max_x: f32,
    min_y: f32, max_y: f32,
    min_p: f32, max_p: f32,
    source: &'static str,
}

impl Default for Ranges {
    fn default() -> Self {
        Self {
            min_x: -2909.0, max_x: 3167.0,
            min_y: -2456.0, max_y: 2565.0,
            min_p: 0.0,    max_p: 253.0,
            source: "apple-defaults",
        }
    }
}

#[derive(Default, Clone)]
struct TrackpadState {
    ranges: Ranges,
    points: Vec<TouchPoint>,
}

fn find_trackpad() -> Option<Device> {
    for (_path, dev) in evdev::enumerate() {
        if let Some(name) = dev.name() {
            let n = name.to_lowercase();
            if n.contains("trackpad") && (n.contains("apple") || n.contains("spi")) {
                return Some(dev);
            }
        }
    }
    None
}

fn read_abs_ranges(dev: &Device) -> Ranges {
    let mut r = Ranges::default();
    let abs = match dev.get_abs_state() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("viz: get_abs_state failed ({e}), using apple defaults");
            return r;
        }
    };
    let ix = AbsoluteAxisType::ABS_MT_POSITION_X.0 as usize;
    let iy = AbsoluteAxisType::ABS_MT_POSITION_Y.0 as usize;
    let ip_mt = AbsoluteAxisType::ABS_MT_PRESSURE.0 as usize;
    let ip = AbsoluteAxisType::ABS_PRESSURE.0 as usize;

    let mut ok = false;
    if ix < abs.len() && abs[ix].maximum > abs[ix].minimum {
        r.min_x = abs[ix].minimum as f32;
        r.max_x = abs[ix].maximum as f32;
        ok = true;
    }
    if iy < abs.len() && abs[iy].maximum > abs[iy].minimum {
        r.min_y = abs[iy].minimum as f32;
        r.max_y = abs[iy].maximum as f32;
        ok = true;
    }
    if ip_mt < abs.len() && abs[ip_mt].maximum > 0 {
        r.min_p = abs[ip_mt].minimum as f32;
        r.max_p = abs[ip_mt].maximum as f32;
    } else if ip < abs.len() && abs[ip].maximum > 0 {
        r.min_p = abs[ip].minimum as f32;
        r.max_p = abs[ip].maximum as f32;
    }
    if ok { r.source = "absinfo"; }
    r
}

fn read_events(state: Arc<Mutex<TrackpadState>>) {
    let mut dev = match find_trackpad() {
        Some(d) => d,
        None => {
            eprintln!("viz: no Apple trackpad found");
            return;
        }
    };
    if let Some(n) = dev.name() { eprintln!("viz: opened {n}"); }

    let r = read_abs_ranges(&dev);
    eprintln!(
        "viz: ranges [{}]  x=[{:.0},{:.0}]  y=[{:.0},{:.0}]  p=[{:.0},{:.0}]",
        r.source, r.min_x, r.max_x, r.min_y, r.max_y, r.min_p, r.max_p
    );
    state.lock().unwrap().ranges = r.clone();

    let mut slots: Vec<TouchPoint> = (0..16).map(|i| TouchPoint { id: -(i+1) as i32, ..Default::default() }).collect();
    let mut cur = 0usize;

    loop {
        let events = match dev.fetch_events() {
            Ok(e) => e,
            Err(e) => { eprintln!("viz: read error {e}"); break; }
        };
        let mut dirty = false;
        for ev in events {
            if let InputEventKind::AbsAxis(axis) = ev.kind() {
                match axis {
                    AbsoluteAxisType::ABS_MT_SLOT => {
                        cur = ev.value().max(0) as usize;
                        while cur >= slots.len() { slots.push(TouchPoint::default()); }
                    }
                    AbsoluteAxisType::ABS_MT_POSITION_X => { slots[cur].x = ev.value() as f32; dirty = true; }
                    AbsoluteAxisType::ABS_MT_POSITION_Y => { slots[cur].y = ev.value() as f32; dirty = true; }
                    AbsoluteAxisType::ABS_MT_PRESSURE | AbsoluteAxisType::ABS_PRESSURE => {
                        // Prefer ABS_MT_PRESSURE, but fall back to ABS_PRESSURE if higher
                        let v = ev.value() as f32;
                        if v > slots[cur].pressure { slots[cur].pressure = v; }
                        dirty = true;
                    }
                    AbsoluteAxisType::ABS_MT_TOUCH_MAJOR => { slots[cur].size = ev.value() as f32; dirty = true; }
                    AbsoluteAxisType::ABS_MT_TRACKING_ID => {
                        let id = ev.value();
                        slots[cur].id = id;
                        if id < 0 { slots[cur] = TouchPoint { id: -1, ..Default::default() }; }
                        dirty = true;
                    }
                    _ => {}
                }
            }
        }
        if dirty {
            let mut s = state.lock().unwrap();
            s.points = slots.iter()
                .filter(|p| p.id >= 0 && p.pressure > 0.0)
                .cloned().collect();
        }
        thread::sleep(Duration::from_millis(6));
    }
}

fn norm(v: f32, min: f32, max: f32) -> f32 {
    let span = max - min;
    if span.abs() < 1.0 { return 0.5; }
    ((v - min) / span).clamp(0.0, 1.0)
}

struct VizApp {
    state: Arc<Mutex<TrackpadState>>,
    flip_y: bool,
}

impl eframe::App for VizApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let s = self.state.lock().unwrap().clone();
        egui::CentralPanel::default().show(ctx, |ui| {
            let (resp, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::hover());
            let rect = resp.rect;
            painter.rect_filled(rect, 4.0, egui::Color32::from_gray(20));

            painter.text(
                rect.left_top() + egui::vec2(8.0, 8.0),
                egui::Align2::LEFT_TOP,
                format!(
                    "points:{}  [{}]  x[{:.0},{:.0}] y[{:.0},{:.0}] p[{:.0},{:.0}]  flip_y={}",
                    s.points.len(), s.ranges.source,
                    s.ranges.min_x, s.ranges.max_x,
                    s.ranges.min_y, s.ranges.max_y,
                    s.ranges.min_p, s.ranges.max_p,
                    self.flip_y
                ),
                egui::FontId::monospace(10.0),
                egui::Color32::GRAY,
            );

            // Draw trackpad bounds as faint outline
            painter.rect_stroke(rect.shrink(20.0), 4.0, egui::Stroke::new(1.0_f32, egui::Color32::from_gray(50)));

            for p in &s.points {
                let nx = norm(p.x, s.ranges.min_x, s.ranges.max_x);
                let ny_raw = norm(p.y, s.ranges.min_y, s.ranges.max_y);
                let ny = if self.flip_y { 1.0 - ny_raw } else { ny_raw };

                let inner = rect.shrink(20.0);
                let cx = inner.left() + nx * inner.width();
                let cy = inner.top() + ny * inner.height();
                let press = norm(p.pressure, s.ranges.min_p, s.ranges.max_p);

                let radius = 10.0 + p.size * 0.02;
                let color = egui::Color32::from_rgb(
                    (255.0 * press) as u8,
                    (200.0 * (1.0 - press)) as u8,
                    (120.0 * (1.0 - press)) as u8,
                );
                painter.circle_filled(egui::pos2(cx, cy), radius, color);
                painter.circle_stroke(egui::pos2(cx, cy), radius + 2.0, egui::Stroke::new(1.0_f32, egui::Color32::WHITE));
                painter.text(
                    egui::pos2(cx + radius + 4.0, cy),
                    egui::Align2::LEFT_CENTER,
                    format!("x{:.0} y{:.0} p{:.0}", p.x, p.y, p.pressure),
                    egui::FontId::monospace(9.0),
                    egui::Color32::WHITE,
                );
            }
        });
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

fn main() -> eframe::Result<()> {
    let flip_y = std::env::var("ASAHI_TRACKPAD_FLIP_Y")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let state = Arc::new(Mutex::new(TrackpadState::default()));
    let st = state.clone();
    thread::spawn(move || read_events(st));

    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 600.0])
            .with_title("Asahi Trackpad Visualizer"),
        ..Default::default()
    };
    eframe::run_native(
        "asahi-trackpad-viz",
        opts,
        Box::new(|_cc| Ok(Box::new(VizApp { state, flip_y }))),
    )
}
