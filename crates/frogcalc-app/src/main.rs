#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, FontFamily, RichText, Stroke, TextEdit, TextStyle};
use serde::{Deserialize, Serialize};

use frogcalc_core::eval::session::{
    AngleMode, DisplayMode, ExecResult, Session, Settings, WorkspaceItem,
};
use frogcalc_core::plot::{
    compile_plot, sample_2d, sample_implicit_2d, sample_implicit_3d, sample_surface, should_break,
    PlotKind,
};

static LIGHT_THEME: AtomicBool = AtomicBool::new(false);

fn is_light() -> bool {
    LIGHT_THEME.load(Ordering::Relaxed)
}

fn set_light(value: bool) {
    LIGHT_THEME.store(value, Ordering::Relaxed);
}

fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

fn bg() -> Color32 {
    if is_light() {
        rgb(244, 243, 240)
    } else {
        rgb(8, 11, 17)
    }
}

fn panel() -> Color32 {
    if is_light() {
        rgb(252, 251, 248)
    } else {
        rgb(15, 22, 34)
    }
}

fn panel2() -> Color32 {
    if is_light() {
        rgb(238, 237, 233)
    } else {
        rgb(19, 28, 43)
    }
}

fn border() -> Color32 {
    if is_light() {
        rgb(215, 213, 207)
    } else {
        rgb(30, 41, 59)
    }
}

fn text() -> Color32 {
    if is_light() {
        rgb(46, 45, 42)
    } else {
        rgb(232, 238, 247)
    }
}

fn muted() -> Color32 {
    if is_light() {
        rgb(109, 107, 102)
    } else {
        rgb(140, 155, 176)
    }
}

fn dim() -> Color32 {
    if is_light() {
        rgb(155, 153, 147)
    } else {
        rgb(91, 106, 128)
    }
}

fn accent() -> Color32 {
    if is_light() {
        rgb(14, 143, 127)
    } else {
        rgb(94, 234, 212)
    }
}

fn violet() -> Color32 {
    if is_light() {
        rgb(117, 86, 201)
    } else {
        rgb(167, 139, 250)
    }
}

fn danger() -> Color32 {
    if is_light() {
        rgb(185, 63, 85)
    } else {
        rgb(251, 113, 133)
    }
}

fn warn() -> Color32 {
    if is_light() {
        rgb(168, 108, 26)
    } else {
        rgb(251, 191, 36)
    }
}

fn sidebar_bg() -> Color32 {
    if is_light() {
        rgb(241, 240, 236)
    } else {
        rgb(11, 16, 24)
    }
}

fn central_bg() -> Color32 {
    if is_light() {
        rgb(248, 247, 244)
    } else {
        rgb(10, 14, 21)
    }
}

fn hover_bg() -> Color32 {
    if is_light() {
        rgb(232, 230, 225)
    } else {
        rgb(28, 40, 58)
    }
}

fn active_bg() -> Color32 {
    if is_light() {
        rgb(223, 221, 215)
    } else {
        rgb(36, 52, 70)
    }
}

fn button_text() -> Color32 {
    if is_light() {
        rgb(255, 255, 255)
    } else {
        rgb(4, 17, 15)
    }
}

fn error_bg() -> Color32 {
    if is_light() {
        rgb(248, 236, 236)
    } else {
        rgb(38, 22, 30)
    }
}

fn error_border() -> Color32 {
    if is_light() {
        rgb(224, 191, 191)
    } else {
        rgb(120, 45, 60)
    }
}

fn error_text() -> Color32 {
    if is_light() {
        rgb(142, 52, 65)
    } else {
        rgb(254, 205, 211)
    }
}

fn toast_bg() -> Color32 {
    if is_light() {
        rgb(253, 252, 250)
    } else {
        rgb(13, 22, 28)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct DisplayResult {
    ok: bool,
    kind: String,
    name: Option<String>,
    plain: String,
    unicode: String,
    approx: Option<String>,
    approx_unicode: Option<String>,
    #[serde(default)]
    approx_exact: bool,
    value_type: String,
    note: Option<String>,
    error: Option<String>,
    error_pos: Option<usize>,
    elapsed_ms: f64,
    suppressed: bool,
}

impl From<ExecResult> for DisplayResult {
    fn from(result: ExecResult) -> Self {
        Self {
            ok: result.ok,
            kind: result.kind,
            name: result.name,
            plain: result.plain,
            unicode: result.unicode,
            approx: result.approx,
            approx_unicode: result.approx_unicode,
            approx_exact: result.approx_exact,
            value_type: result.value_type,
            note: result.note,
            error: result.error,
            error_pos: result.error_pos,
            elapsed_ms: result.elapsed_ms,
            suppressed: result.suppressed,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct HistoryEntry {
    input: String,
    results: Vec<DisplayResult>,
    at: String,
}

#[derive(Clone, Debug)]
struct ScriptRun {
    results: Vec<DisplayResult>,
    at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SettingsSnapshot {
    precision: usize,
    angle_mode: String,
    display_mode: String,
    #[serde(default = "default_theme")]
    theme: String,
}

fn default_theme() -> String {
    "dark".into()
}

impl Default for SettingsSnapshot {
    fn default() -> Self {
        Self {
            precision: 50,
            angle_mode: "rad".into(),
            display_mode: "both".into(),
            theme: default_theme(),
        }
    }
}

impl SettingsSnapshot {
    fn to_settings(&self) -> Settings {
        Settings {
            precision: self.precision.clamp(10, 500),
            angle_mode: if self.angle_mode == "deg" {
                AngleMode::Deg
            } else {
                AngleMode::Rad
            },
            display_mode: match self.display_mode.as_str() {
                "exact" => DisplayMode::Exact,
                "approx" => DisplayMode::Approx,
                _ => DisplayMode::Both,
            },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SavedState {
    version: u32,
    history: Vec<HistoryEntry>,
    settings: SettingsSnapshot,
    workspace_source: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    script: String,
    #[serde(default)]
    plot: PlotState,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PlotState {
    expression: String,
    #[serde(default = "default_dimension")]
    dimension: String,
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
    #[serde(default = "default_z_min")]
    z_min: f64,
    #[serde(default = "default_z_max")]
    z_max: f64,
    samples: usize,
    surface_side: usize,
    #[serde(default = "default_iso_side_2d")]
    implicit_side_2d: usize,
    #[serde(default = "default_iso_side_3d")]
    implicit_side_3d: usize,
    #[serde(default = "default_true")]
    auto_y: bool,
    #[serde(default)]
    wireframe: bool,
}

fn default_z_min() -> f64 {
    -10.0
}

fn default_z_max() -> f64 {
    10.0
}

fn default_iso_side_2d() -> usize {
    140
}

fn default_iso_side_3d() -> usize {
    26
}

fn default_dimension() -> String {
    "2d".into()
}

fn default_true() -> bool {
    true
}

impl Default for PlotState {
    fn default() -> Self {
        Self {
            expression: "sin(x)".into(),
            dimension: default_dimension(),
            x_min: -10.0,
            x_max: 10.0,
            y_min: -10.0,
            y_max: 10.0,
            z_min: -10.0,
            z_max: 10.0,
            samples: 800,
            surface_side: 44,
            implicit_side_2d: 140,
            implicit_side_3d: 26,
            auto_y: true,
            wireframe: false,
        }
    }
}

impl PlotState {
    fn is_3d(&self) -> bool {
        self.dimension == "3d"
    }

    fn signature(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            self.expression,
            self.dimension,
            self.x_min,
            self.x_max,
            self.y_min,
            self.y_max,
            self.z_min,
            self.z_max,
            self.samples,
            self.surface_side,
            self.implicit_side_2d,
            self.implicit_side_3d,
            self.auto_y,
            self.wireframe
        )
    }
}

#[derive(Clone, Debug)]
enum PlotData {
    Curve(Vec<Option<(f64, f64)>>),
    ImplicitCurve {
        segments: Vec<[(f64, f64); 2]>,
        x0: f64,
        x1: f64,
        y0: f64,
        y1: f64,
    },
    Surface {
        values: Vec<Option<f64>>,
        nx: usize,
        ny: usize,
        y0: f64,
        y1: f64,
    },
    ImplicitSurface {
        triangles: Vec<[[f64; 3]; 3]>,
        x0: f64,
        x1: f64,
        y0: f64,
        y1: f64,
        z0: f64,
        z1: f64,
    },
    Error(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FileAction {
    Import,
    Export,
}

struct FileDialog {
    action: FileAction,
    directory: PathBuf,
    filename: String,
    error: Option<String>,
}

fn default_mode() -> String {
    "command".into()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ClearAction {
    Workspace,
    History,
}

impl Default for SavedState {
    fn default() -> Self {
        Self {
            version: 1,
            history: Vec::new(),
            settings: SettingsSnapshot::default(),
            workspace_source: String::new(),
            mode: default_mode(),
            script: String::new(),
            plot: PlotState::default(),
        }
    }
}

struct FrogApp {
    session: Session,
    settings: SettingsSnapshot,
    input: String,
    script: String,
    mode: String,
    history: Vec<HistoryEntry>,
    workspace: Vec<WorkspaceItem>,
    last_script: Option<ScriptRun>,
    settings_open: bool,
    help_open: bool,
    confirm_clear: Option<ClearAction>,
    toast: Option<(String, Instant)>,
    should_scroll: bool,
    focus_input: bool,
    focus_script: bool,
    plot: PlotState,
    plot_data: Option<PlotData>,
    plot_signature: String,
    plot_dirty_since: Option<Instant>,
    plot_kind: Option<PlotKind>,
    plot_fit_key: String,
    plot_yaw: f32,
    plot_pitch: f32,
    focus_plot: bool,
    file_dialog: Option<FileDialog>,
}

impl FrogApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (saved, migrated) = load_state();
        install_style(&cc.egui_ctx, &saved.settings.theme);
        install_fonts(&cc.egui_ctx);

        let mut session = Session::new();
        session.set_settings(saved.settings.to_settings());
        if saved.workspace_source.trim().len() > 0 {
            let _ = session.execute(&saved.workspace_source);
        }
        let workspace = session.workspace();
        let mode = if saved.mode == "script" {
            "script".to_string()
        } else if saved.mode == "plot" {
            "plot".to_string()
        } else {
            "command".to_string()
        };
        let mode_is_script = mode == "script";
        let mode_is_plot = mode == "plot";
        let app = Self {
            session,
            settings: saved.settings,
            input: String::new(),
            script: saved.script,
            mode,
            history: saved.history,
            workspace,
            last_script: None,
            settings_open: false,
            help_open: false,
            confirm_clear: None,
            toast: None,
            should_scroll: true,
            focus_input: !mode_is_script,
            focus_script: mode_is_script,
            plot: saved.plot,
            plot_data: None,
            plot_signature: String::new(),
            plot_dirty_since: None,
            plot_kind: None,
            plot_fit_key: String::new(),
            plot_yaw: -0.6,
            plot_pitch: 0.55,
            focus_plot: mode_is_plot,
            file_dialog: None,
        };
        if migrated {
            let legacy = legacy_state_path();
            if app.save() {
                let backup = legacy.with_extension("json.bak");
                let _ = fs::rename(&legacy, &backup);
            }
        }
        app
    }

    fn execute_input(&mut self) {
        let input = self.input.trim().to_string();
        if input.is_empty() {
            return;
        }
        let results: Vec<DisplayResult> = self
            .session
            .execute(&input)
            .into_iter()
            .map(DisplayResult::from)
            .collect();
        let at = time_now();
        self.history.push(HistoryEntry { input, results, at });
        if self.history.len() > 200 {
            self.history.remove(0);
        }
        self.workspace = self.session.workspace();
        self.input.clear();
        self.should_scroll = true;
        self.focus_input = true;
        self.save();
    }

    fn is_script(&self) -> bool {
        self.mode == "script"
    }

    fn is_plot(&self) -> bool {
        self.mode == "plot"
    }

    fn set_mode(&mut self, mode: &str) {
        if self.mode == mode {
            return;
        }
        self.mode = mode.to_string();
        if self.is_script() {
            self.focus_script = true;
        } else if self.is_plot() {
            self.focus_plot = true;
            if self.plot_data.is_none() {
                self.run_plot(true, true);
            }
        } else {
            self.focus_input = true;
        }
        self.save();
    }

    fn execute_script(&mut self) {
        let script = self.script.trim().to_string();
        if script.is_empty() {
            return;
        }
        let results: Vec<DisplayResult> = self
            .session
            .execute(&script)
            .into_iter()
            .map(DisplayResult::from)
            .collect();
        let failed = results.iter().any(|result| !result.ok);
        self.last_script = Some(ScriptRun {
            results,
            at: time_now(),
        });
        self.workspace = self.session.workspace();
        self.focus_script = true;
        self.save();
        if failed {
            self.toast("脚本已中止：请检查错误");
        } else {
            self.toast("脚本运行完成");
        }
    }

    // -----------------------------------------------------------------------
    // 绘图
    // -----------------------------------------------------------------------

    fn run_plot(&mut self, persist: bool, allow_dimension_switch: bool) {
        let mut target = compile_plot(&self.plot.expression, self.plot.is_3d());
        // 2D 下输入含 z 的方程/表达式时自动切到 3D，避免“明明支持却报错”。
        // 手动点击 2D/3D 时不允许自动切回，否则按钮会失效。
        if allow_dimension_switch && target.is_err() && !self.plot.is_3d() {
            if let Ok(candidate) = compile_plot(&self.plot.expression, true) {
                if candidate.kind == PlotKind::Implicit3D || candidate.kind == PlotKind::Explicit3D
                {
                    self.plot.dimension = "3d".into();
                    target = Ok(candidate);
                }
            }
        }
        let target = match target {
            Ok(target) => target,
            Err(error) => {
                self.plot_kind = None;
                self.plot_data = Some(PlotData::Error(error.message));
                self.plot_signature = self.plot.signature();
                return;
            }
        };
        self.plot_kind = Some(target.kind);
        self.sample_plot(target.kind, &target.node);
        // 第一次绘制某个隐函数时，按曲面包围盒自动缩放，避免球面在 [-10,10]^3 里只有一个点。
        if target.kind.is_implicit() {
            let key = format!("{}|{}", self.plot.expression.trim(), self.plot.is_3d());
            if self.plot_fit_key != key {
                self.plot_fit_key = key.clone();
                // 旧范围完全不含零等值面时，先回到默认范围再采样一次。
                if self.implicit_data_empty() && !self.ranges_are_default() {
                    self.reset_plot_ranges();
                    self.plot_fit_key = key;
                    self.sample_plot(target.kind, &target.node);
                }
                if self.fit_ranges_from_data(true) {
                    self.sample_plot(target.kind, &target.node);
                }
            }
        }
        self.plot_signature = self.plot.signature();
        if persist {
            self.save();
        }
    }

    fn implicit_data_empty(&self) -> bool {
        match &self.plot_data {
            Some(PlotData::ImplicitCurve { segments, .. }) => segments.is_empty(),
            Some(PlotData::ImplicitSurface { triangles, .. }) => triangles.is_empty(),
            _ => false,
        }
    }

    fn ranges_are_default(&self) -> bool {
        let near = |a: f64, b: f64| (a - b).abs() < 1e-9;
        let planar = near(self.plot.x_min, -10.0)
            && near(self.plot.x_max, 10.0)
            && near(self.plot.y_min, -10.0)
            && near(self.plot.y_max, 10.0);
        planar
            && (!self.plot.is_3d() || (near(self.plot.z_min, -10.0) && near(self.plot.z_max, 10.0)))
    }

    fn sample_plot(&mut self, kind: PlotKind, node: &frogcalc_core::parser::Node) {
        match kind {
            PlotKind::Explicit2D => {
                let curve = sample_2d(
                    &mut self.session,
                    node,
                    "x",
                    self.plot.x_min,
                    self.plot.x_max,
                    self.plot.samples,
                );
                self.plot_data = Some(PlotData::Curve(curve.points));
            }
            PlotKind::Explicit3D => {
                let surface = sample_surface(
                    &mut self.session,
                    node,
                    "x",
                    "y",
                    self.plot.x_min,
                    self.plot.x_max,
                    self.plot.y_min,
                    self.plot.y_max,
                    self.plot.surface_side,
                );
                self.plot_data = Some(PlotData::Surface {
                    values: surface.values,
                    nx: surface.nx,
                    ny: surface.ny,
                    y0: surface.y0,
                    y1: surface.y1,
                });
            }
            PlotKind::Implicit2D => {
                let sampled = sample_implicit_2d(
                    &mut self.session,
                    node,
                    "x",
                    "y",
                    self.plot.x_min,
                    self.plot.x_max,
                    self.plot.y_min,
                    self.plot.y_max,
                    self.plot.implicit_side_2d,
                );
                self.plot_data = Some(PlotData::ImplicitCurve {
                    segments: sampled.segments,
                    x0: sampled.x0,
                    x1: sampled.x1,
                    y0: sampled.y0,
                    y1: sampled.y1,
                });
            }
            PlotKind::Implicit3D => {
                let sampled = sample_implicit_3d(
                    &mut self.session,
                    node,
                    "x",
                    "y",
                    "z",
                    self.plot.x_min,
                    self.plot.x_max,
                    self.plot.y_min,
                    self.plot.y_max,
                    self.plot.z_min,
                    self.plot.z_max,
                    self.plot.implicit_side_3d,
                );
                self.plot_data = Some(PlotData::ImplicitSurface {
                    triangles: sampled.triangles,
                    x0: sampled.x0,
                    x1: sampled.x1,
                    y0: sampled.y0,
                    y1: sampled.y1,
                    z0: sampled.z0,
                    z1: sampled.z1,
                });
            }
        }
    }

    /// 用当前隐函数数据计算包围盒；`only_if_small` 为真时只在曲面明显小于视窗时缩放。
    fn fit_ranges_from_data(&mut self, only_if_small: bool) -> bool {
        let mut low = [f64::INFINITY; 3];
        let mut high = [f64::NEG_INFINITY; 3];
        let mut any = false;
        let mut include = |point: [f64; 3]| {
            for axis in 0..3 {
                low[axis] = low[axis].min(point[axis]);
                high[axis] = high[axis].max(point[axis]);
            }
            any = true;
        };
        match &self.plot_data {
            Some(PlotData::ImplicitCurve { segments, .. }) => {
                for segment in segments {
                    for (x, y) in segment {
                        include([*x, *y, 0.0]);
                    }
                }
            }
            Some(PlotData::ImplicitSurface { triangles, .. }) => {
                for triangle in triangles {
                    for point in triangle {
                        include(*point);
                    }
                }
            }
            _ => return false,
        }
        if !any || !low[0].is_finite() {
            return false;
        }
        let view = [
            (self.plot.x_max - self.plot.x_min).abs(),
            (self.plot.y_max - self.plot.y_min).abs(),
            (self.plot.z_max - self.plot.z_min).abs(),
        ];
        let surface = [
            (high[0] - low[0]).max(0.0),
            (high[1] - low[1]).max(0.0),
            (high[2] - low[2]).max(0.0),
        ];
        if only_if_small {
            let surface_diagonal =
                (surface[0] * surface[0] + surface[1] * surface[1] + surface[2] * surface[2])
                    .sqrt();
            let view_diagonal = if self.plot.is_3d() {
                (view[0] * view[0] + view[1] * view[1] + view[2] * view[2]).sqrt()
            } else {
                (view[0] * view[0] + view[1] * view[1]).sqrt()
            };
            if view_diagonal > 0.0 && surface_diagonal > view_diagonal * 0.55 {
                return false;
            }
        }
        let pad = |span: f64| {
            if span > 1e-9 {
                span * 0.08
            } else {
                0.5
            }
        };
        let (x0, x1) = (low[0] - pad(surface[0]), high[0] + pad(surface[0]));
        let (y0, y1) = (low[1] - pad(surface[1]), high[1] + pad(surface[1]));
        self.plot.x_min = x0;
        self.plot.x_max = x1;
        self.plot.y_min = y0;
        self.plot.y_max = y1;
        if self.plot.is_3d() {
            let (z0, z1) = (low[2] - pad(surface[2]), high[2] + pad(surface[2]));
            self.plot.z_min = z0;
            self.plot.z_max = z1;
        }
        true
    }

    fn auto_fit_plot(&mut self) {
        if self.fit_ranges_from_data(false) {
            self.run_plot(true, true);
        }
    }

    fn reset_plot_ranges(&mut self) {
        self.plot.x_min = -10.0;
        self.plot.x_max = 10.0;
        self.plot.y_min = -10.0;
        self.plot.y_max = 10.0;
        self.plot.z_min = -10.0;
        self.plot.z_max = 10.0;
        self.plot.auto_y = true;
        self.plot_fit_key.clear();
    }

    /// 参数变化后自动重采样：2D 快速刷新，3D 略微延迟以免拖动时卡顿。
    fn auto_refresh_plot(&mut self, ctx: &egui::Context) {
        if self.plot_data.is_none() {
            self.run_plot(false, true);
            self.plot_dirty_since = None;
            ctx.request_repaint();
            return;
        }
        if self.plot_signature == self.plot.signature() {
            self.plot_dirty_since = None;
            return;
        }
        let delay_ms = if self.plot.is_3d() { 220 } else { 60 };
        let ready = self.plot_dirty_since.map_or(false, |since| {
            since.elapsed() >= Duration::from_millis(delay_ms)
        });
        if ready {
            self.run_plot(false, true);
            self.plot_dirty_since = None;
            ctx.request_repaint();
        } else {
            if self.plot_dirty_since.is_none() {
                self.plot_dirty_since = Some(Instant::now());
            }
            ctx.request_repaint_after(Duration::from_millis(delay_ms));
        }
    }

    fn plot_view(&mut self, ui: &mut egui::Ui) {
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            let three_d = self.plot.is_3d();
            let implicit = self.plot_kind.is_some_and(|kind| kind.is_implicit());
            ui.label(
                RichText::new(if implicit {
                    "F ="
                } else if three_d {
                    "z ="
                } else {
                    "y ="
                })
                .size(15.0)
                .color(accent()),
            );
            let width = (ui.available_width() - 230.0).max(150.0);
            let response = ui.add(
                TextEdit::singleline(&mut self.plot.expression)
                    .hint_text(if implicit {
                        "例如 x^2 + y^2 + z^2 = 1、x^2 + y^2 - 1"
                    } else if three_d {
                        "例如 x^2 - y^2、sin(x)*cos(y)"
                    } else {
                        "例如 sin(x)、x^2 - 2"
                    })
                    .font(TextStyle::Monospace)
                    .text_color(text())
                    .background_color(panel2())
                    .desired_width(width)
                    .margin(egui::Margin::symmetric(9, 7)),
            );
            let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if self.focus_plot {
                response.request_focus();
                self.focus_plot = false;
            }
            if ui
                .button(RichText::new("绘制").size(11.5))
                .on_hover_text("按当前参数重新采样")
                .clicked()
                || enter
            {
                self.run_plot(true, true);
            }
            if ui.selectable_label(!self.plot.is_3d(), "2D").clicked() {
                self.plot.dimension = "2d".into();
                self.run_plot(true, false);
            }
            if ui.selectable_label(self.plot.is_3d(), "3D").clicked() {
                self.plot.dimension = "3d".into();
                self.run_plot(true, false);
            }
        });
        ui.add_space(3.0);
        let detected_implicit = self.plot_kind.is_some_and(|kind| kind.is_implicit());
        let has_error = matches!(&self.plot_data, Some(PlotData::Error(_)));
        ui.horizontal(|ui| {
            let (label, color) = if has_error {
                ("输入无法绘制", danger())
            } else {
                match self.plot_kind {
                    Some(PlotKind::Implicit2D) => ("隐函数 F(x, y) = 0", violet()),
                    Some(PlotKind::Implicit3D) => ("隐函数 F(x, y, z) = 0", violet()),
                    Some(PlotKind::Explicit3D) => ("显式曲面 z = f(x, y)", accent()),
                    Some(PlotKind::Explicit2D) => ("显式曲线 y = f(x)", accent()),
                    None => ("识别中…", dim()),
                }
            };
            badge(ui, label, color);
            if !detected_implicit && !has_error {
                let hint = if self.plot.is_3d() {
                    "输入含 z，或输入 x^2 + y^2 + z^2 = 1 这类方程时，会自动按隐函数 F(x, y, z) = 0 绘制"
                } else {
                    "输入含 y 时，会自动按隐函数 F(x, y) = 0 绘制"
                };
                ui.label(RichText::new(hint).size(9.0).color(dim()));
            }
        });
        ui.add_space(4.0);
        let kind = self.plot_kind;
        let implicit2d = matches!(kind, Some(PlotKind::Implicit2D));
        let implicit3d = matches!(kind, Some(PlotKind::Implicit3D));
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("x ∈").size(10.0).color(dim()));
            ui.add(
                egui::DragValue::new(&mut self.plot.x_min)
                    .speed(0.1)
                    .max_decimals(2)
                    .range(-1.0e6..=1.0e6),
            );
            ui.label(RichText::new("→").size(10.0).color(dim()));
            ui.add(
                egui::DragValue::new(&mut self.plot.x_max)
                    .speed(0.1)
                    .max_decimals(2)
                    .range(-1.0e6..=1.0e6),
            );
            if self.plot.is_3d() {
                ui.label(RichText::new("y ∈").size(10.0).color(dim()));
                ui.add(
                    egui::DragValue::new(&mut self.plot.y_min)
                        .speed(0.1)
                        .max_decimals(2)
                        .range(-1.0e6..=1.0e6),
                );
                ui.label(RichText::new("→").size(10.0).color(dim()));
                ui.add(
                    egui::DragValue::new(&mut self.plot.y_max)
                        .speed(0.1)
                        .max_decimals(2)
                        .range(-1.0e6..=1.0e6),
                );
                if implicit3d {
                    ui.label(RichText::new("z ∈").size(10.0).color(dim()));
                    ui.add(
                        egui::DragValue::new(&mut self.plot.z_min)
                            .speed(0.1)
                            .max_decimals(2)
                            .range(-1.0e6..=1.0e6),
                    );
                    ui.label(RichText::new("→").size(10.0).color(dim()));
                    ui.add(
                        egui::DragValue::new(&mut self.plot.z_max)
                            .speed(0.1)
                            .max_decimals(2)
                            .range(-1.0e6..=1.0e6),
                    );
                    ui.label(RichText::new("分辨率").size(10.0).color(dim()));
                    ui.add(egui::DragValue::new(&mut self.plot.implicit_side_3d).range(8..=64));
                    if ui
                        .small_button(RichText::new("自动范围").size(9.5).color(dim()))
                        .on_hover_text("把 x / y / z 范围缩放到曲面附近")
                        .clicked()
                    {
                        self.auto_fit_plot();
                    }
                    if ui
                        .small_button(RichText::new("重置").size(9.5).color(dim()))
                        .on_hover_text("把 x / y / z 范围恢复为 [-10, 10] 后再自动适应")
                        .clicked()
                    {
                        self.reset_plot_ranges();
                        self.run_plot(true, true);
                    }
                } else {
                    ui.label(RichText::new("网格").size(10.0).color(dim()));
                    ui.add(egui::DragValue::new(&mut self.plot.surface_side).range(8..=80));
                    ui.checkbox(&mut self.plot.wireframe, RichText::new("线框").size(10.0));
                }
            } else if implicit2d {
                ui.label(RichText::new("y ∈").size(10.0).color(dim()));
                ui.add(
                    egui::DragValue::new(&mut self.plot.y_min)
                        .speed(0.1)
                        .max_decimals(2)
                        .range(-1.0e6..=1.0e6),
                );
                ui.label(RichText::new("→").size(10.0).color(dim()));
                ui.add(
                    egui::DragValue::new(&mut self.plot.y_max)
                        .speed(0.1)
                        .max_decimals(2)
                        .range(-1.0e6..=1.0e6),
                );
                ui.label(RichText::new("分辨率").size(10.0).color(dim()));
                ui.add(egui::DragValue::new(&mut self.plot.implicit_side_2d).range(40..=320));
                if ui
                    .small_button(RichText::new("自动范围").size(9.5).color(dim()))
                    .on_hover_text("把 x / y 范围缩放到曲线附近")
                    .clicked()
                {
                    self.auto_fit_plot();
                }
                if ui
                    .small_button(RichText::new("重置").size(9.5).color(dim()))
                    .on_hover_text("把 x / y 范围恢复为 [-10, 10] 后再自动适应")
                    .clicked()
                {
                    self.reset_plot_ranges();
                    self.run_plot(true, true);
                }
            } else {
                ui.checkbox(
                    &mut self.plot.auto_y,
                    RichText::new("自动 y 范围").size(10.0),
                );
                if !self.plot.auto_y {
                    ui.label(RichText::new("y ∈").size(10.0).color(dim()));
                    ui.add(
                        egui::DragValue::new(&mut self.plot.y_min)
                            .speed(0.1)
                            .max_decimals(2)
                            .range(-1.0e6..=1.0e6),
                    );
                    ui.label(RichText::new("→").size(10.0).color(dim()));
                    ui.add(
                        egui::DragValue::new(&mut self.plot.y_max)
                            .speed(0.1)
                            .max_decimals(2)
                            .range(-1.0e6..=1.0e6),
                    );
                    if ui
                        .small_button(RichText::new("自动").size(9.5).color(dim()))
                        .on_hover_text("按采样数据自动确定 y 范围")
                        .clicked()
                    {
                        self.plot.auto_y = true;
                    }
                }
                ui.label(RichText::new("采样").size(10.0).color(dim()));
                ui.add(egui::DragValue::new(&mut self.plot.samples).range(32..=4000));
            }
        });

        // 固定 y 范围时提示裁剪，避免误以为曲线消失。
        let clipped = match &self.plot_data {
            Some(PlotData::Curve(points)) if !self.plot.auto_y => points
                .iter()
                .flatten()
                .any(|(_, y)| *y < self.plot.y_min || *y > self.plot.y_max),
            _ => false,
        };
        if clipped {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("部分曲线超出当前 y 范围，已被裁剪")
                        .size(9.5)
                        .color(warn()),
                );
                if ui
                    .small_button(RichText::new("自动 y").size(9.5).color(dim()))
                    .clicked()
                {
                    self.plot.auto_y = true;
                }
            });
        }
        if self.plot_kind.is_some_and(|kind| kind.is_implicit()) {
            let empty = match &self.plot_data {
                Some(PlotData::ImplicitCurve { segments, .. }) => segments.is_empty(),
                Some(PlotData::ImplicitSurface { triangles, .. }) => triangles.is_empty(),
                _ => false,
            };
            if empty {
                ui.label(
                    RichText::new("当前范围内没有找到 F = 0 的等值面，试试「自动范围」或扩大范围")
                        .size(9.5)
                        .color(warn()),
                );
            }
        }
        ui.add_space(5.0);
        self.auto_refresh_plot(ui.ctx());
        let height = (ui.available_height() - 26.0).max(180.0);
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), height),
            egui::Sense::drag(),
        );
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 10.0, panel2());
        match self.plot_data.clone() {
            Some(PlotData::Error(message)) => {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    message,
                    egui::FontId::proportional(12.0),
                    danger(),
                );
            }
            Some(PlotData::Curve(points)) => {
                draw_curve(
                    &painter,
                    rect,
                    &points,
                    self.plot.x_min,
                    self.plot.x_max,
                    self.plot.auto_y,
                    self.plot.y_min,
                    self.plot.y_max,
                );
            }
            Some(PlotData::ImplicitCurve {
                segments,
                x0,
                x1,
                y0,
                y1,
            }) => {
                draw_implicit_curve(&painter, rect, &segments, x0, x1, y0, y1);
            }
            Some(PlotData::Surface {
                values,
                nx,
                ny,
                y0,
                y1,
            }) => {
                if response.dragged() {
                    self.plot_yaw += response.drag_delta().x * 0.01;
                    self.plot_pitch =
                        (self.plot_pitch + response.drag_delta().y * 0.01).clamp(-1.45, 1.45);
                }
                draw_surface(
                    &painter,
                    rect,
                    &values,
                    nx,
                    ny,
                    y0,
                    y1,
                    self.plot.x_min,
                    self.plot.x_max,
                    self.plot_yaw,
                    self.plot_pitch,
                    self.plot.wireframe,
                );
            }
            Some(PlotData::ImplicitSurface {
                triangles,
                x0,
                x1,
                y0,
                y1,
                z0,
                z1,
            }) => {
                if response.dragged() {
                    self.plot_yaw += response.drag_delta().x * 0.01;
                    self.plot_pitch =
                        (self.plot_pitch + response.drag_delta().y * 0.01).clamp(-1.45, 1.45);
                }
                draw_implicit_surface(
                    &painter,
                    rect,
                    &triangles,
                    x0,
                    x1,
                    y0,
                    y1,
                    z0,
                    z1,
                    self.plot_yaw,
                    self.plot_pitch,
                );
            }
            None => {}
        }
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            let hint = match self.plot_kind {
                Some(PlotKind::Implicit3D) => "3D 隐函数曲面 F(x, y, z) = 0 · 拖动旋转视角",
                Some(PlotKind::Implicit2D) => "2D 隐函数曲线 F(x, y) = 0 · 扫描零等值线",
                Some(PlotKind::Explicit3D) => {
                    "3D 曲面 z = f(x, y) · 拖动旋转视角 · z 轴按数据自动缩放"
                }
                _ => "2D 曲线 y = f(x) · 函数、分段函数与用户函数都可绘制",
            };
            ui.label(RichText::new(hint).size(9.0).color(dim()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let implicit2d = matches!(self.plot_kind, Some(PlotKind::Implicit2D));
                let y_text = if self.plot.is_3d() || implicit2d || !self.plot.auto_y {
                    format!(
                        "y ∈ [{}, {}]",
                        trim_number(self.plot.y_min),
                        trim_number(self.plot.y_max)
                    )
                } else {
                    "y 自动".to_string()
                };
                let mut ranges = format!(
                    "x ∈ [{}, {}] · {}",
                    trim_number(self.plot.x_min),
                    trim_number(self.plot.x_max),
                    y_text
                );
                if matches!(self.plot_kind, Some(PlotKind::Implicit3D)) {
                    ranges.push_str(&format!(
                        " · z ∈ [{}, {}]",
                        trim_number(self.plot.z_min),
                        trim_number(self.plot.z_max)
                    ));
                }
                ui.label(RichText::new(ranges).size(9.0).color(dim()));
            });
        });
    }

    // -----------------------------------------------------------------------
    // 脚本文件导入 / 导出
    // -----------------------------------------------------------------------

    fn open_file_dialog(&mut self, action: FileAction) {
        let directory = script_directory();
        let filename = if action == FileAction::Export {
            "script.frogcalc".to_string()
        } else {
            String::new()
        };
        self.file_dialog = Some(FileDialog {
            action,
            directory,
            filename,
            error: None,
        });
    }

    fn import_script(&mut self, path: &Path) {
        match fs::read_to_string(path) {
            Ok(text) => {
                self.script = text;
                self.file_dialog = None;
                self.set_mode("script");
                self.focus_script = true;
                self.save();
                self.toast(format!("已导入 {}", path.display()));
            }
            Err(error) => {
                let message = format!("读取失败：{error}");
                if let Some(dialog) = self.file_dialog.as_mut() {
                    dialog.error = Some(message.clone());
                }
                self.toast(message);
            }
        }
    }

    fn export_script(&mut self, path: &Path) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        match fs::write(path, &self.script) {
            Ok(()) => {
                self.file_dialog = None;
                self.save();
                self.toast(format!("已导出到 {}", path.display()));
            }
            Err(error) => {
                let message = format!("写入失败：{error}");
                if let Some(dialog) = self.file_dialog.as_mut() {
                    dialog.error = Some(message.clone());
                }
                self.toast(message);
            }
        }
    }

    fn file_dialog_window(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.file_dialog.take() else {
            return;
        };
        let mut open = true;
        let mut chosen: Option<PathBuf> = None;
        let mut close = false;
        let title = match dialog.action {
            FileAction::Import => "导入脚本",
            FileAction::Export => "导出脚本",
        };
        egui::Window::new(title)
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([540.0, 430.0])
            .show(ctx, |ui| {
                ui.set_min_width(480.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("目录").size(10.0).color(dim()));
                    ui.label(
                        RichText::new(dialog.directory.display().to_string())
                            .size(10.0)
                            .monospace()
                            .color(muted()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button(RichText::new("↑ 上一级").size(10.0))
                            .clicked()
                        {
                            if let Some(parent) = dialog.directory.parent() {
                                dialog.directory = parent.to_path_buf();
                            }
                        }
                    });
                });
                ui.separator();
                let mut directories = Vec::new();
                let mut files = Vec::new();
                if let Ok(entries) = fs::read_dir(&dialog.directory) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            directories.push(path);
                        } else if is_script_file(&path) {
                            files.push(path);
                        }
                    }
                }
                directories.sort();
                files.sort();
                egui::ScrollArea::vertical()
                    .id_salt("file_dialog_list")
                    .max_height(280.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for directory in &directories {
                            let name = directory
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            if ui
                                .selectable_label(
                                    false,
                                    RichText::new(format!("📁 {name}")).size(11.0),
                                )
                                .clicked()
                            {
                                dialog.directory = directory.clone();
                            }
                        }
                        for file in &files {
                            let name = file
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            let selected = dialog.filename == name;
                            if ui
                                .selectable_label(
                                    selected,
                                    RichText::new(format!("📄 {name}")).size(11.0),
                                )
                                .clicked()
                            {
                                dialog.filename = name.clone();
                                if dialog.action == FileAction::Import {
                                    chosen = Some(file.clone());
                                }
                            }
                        }
                        if directories.is_empty() && files.is_empty() {
                            ui.label(
                                RichText::new("此目录下没有可用的脚本文件")
                                    .size(10.5)
                                    .color(dim()),
                            );
                        }
                    });
                ui.add_space(6.0);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(RichText::new("文件名").size(10.0).color(dim()));
                    ui.add(
                        TextEdit::singleline(&mut dialog.filename)
                            .desired_width(280.0)
                            .hint_text("script.frogcalc"),
                    );
                });
                if let Some(error) = &dialog.error {
                    ui.label(RichText::new(error).size(10.0).color(danger()));
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let label = match dialog.action {
                        FileAction::Import => "导入",
                        FileAction::Export => "导出",
                    };
                    if ui.button(label).clicked() {
                        let name = dialog.filename.trim();
                        if name.is_empty() {
                            dialog.error = Some("请输入文件名".into());
                        } else {
                            let path = dialog.directory.join(name);
                            if dialog.action == FileAction::Import {
                                if !path.exists() {
                                    dialog.error = Some(format!("文件不存在：{}", path.display()));
                                } else {
                                    chosen = Some(path);
                                }
                            } else {
                                chosen = Some(path);
                            }
                        }
                    }
                    if ui.button("取消").clicked() {
                        close = true;
                    }
                });
            });
        if let Some(path) = chosen {
            match dialog.action {
                FileAction::Import => self.import_script(&path),
                FileAction::Export => self.export_script(&path),
            }
            return;
        }
        if !open || close {
            self.file_dialog = None;
        } else {
            self.file_dialog = Some(dialog);
        }
    }

    fn save(&self) -> bool {
        let state = SavedState {
            version: 1,
            history: self.history.clone(),
            settings: self.settings.clone(),
            workspace_source: self.session.workspace_source(),
            mode: self.mode.clone(),
            script: self.script.clone(),
            plot: self.plot.clone(),
        };
        if let Ok(text) = serde_json::to_string(&state) {
            let path = state_path();
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let tmp = path.with_extension("json.tmp");
            if fs::write(&tmp, text).is_ok() {
                return fs::rename(tmp, path).is_ok();
            }
        }
        false
    }

    fn toast(&mut self, message: impl Into<String>) {
        self.toast = Some((message.into(), Instant::now()));
    }

    fn insert(&mut self, text: &str) {
        if self.is_script() {
            if !self.script.is_empty() && !self.script.ends_with('\n') {
                self.script.push('\n');
            }
            self.script.push_str(text);
            self.focus_script = true;
        } else {
            if !self.input.is_empty() && !self.input.ends_with(' ') {
                self.input.push(' ');
            }
            self.input.push_str(text);
            self.focus_input = true;
        }
    }

    fn clear_workspace(&mut self) {
        self.session.clear_all();
        self.workspace.clear();
        self.last_script = None;
        self.save();
        self.toast("工作区已清空");
    }

    fn clear_history(&mut self) {
        self.history.clear();
        self.save();
        self.toast("历史记录已清空");
    }

    fn copy(&mut self, ctx: &egui::Context, text: &str, label: &str) {
        ctx.copy_text(text.to_string());
        self.toast(format!("已复制{label}"));
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("ƒ").size(28.0).strong().color(accent()));
            ui.vertical(|ui| {
                ui.label(RichText::new("FrogCalc").size(17.0).strong().color(text()));
                ui.label(
                    RichText::new("精确计算器 · CAS · Rust 原生")
                        .size(10.0)
                        .color(dim()),
                );
            });
            ui.add_space(18.0);
            badge(ui, &format!("{} 位", self.settings.precision), accent());
            badge(
                ui,
                if self.settings.angle_mode == "deg" {
                    "角度制"
                } else {
                    "弧度制"
                },
                muted(),
            );
            badge(ui, "精确优先", accent());
            ui.add_space(6.0);
            egui::Frame::new()
                .fill(panel2())
                .stroke(Stroke::new(1.0, border()))
                .corner_radius(8.0)
                .inner_margin(egui::Margin::symmetric(3, 2))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for (value, label) in
                            [("command", "命令"), ("script", "脚本"), ("plot", "绘图")]
                        {
                            if ui
                                .selectable_label(
                                    self.mode == value,
                                    RichText::new(label).size(11.0),
                                )
                                .clicked()
                            {
                                self.set_mode(value);
                            }
                        }
                    });
                });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(RichText::new("⚙ 设置").size(12.0)).clicked() {
                    self.settings_open = true;
                }
                if ui.button(RichText::new("? 帮助").size(12.0)).clicked() {
                    self.help_open = true;
                }
                let theme_label = if is_light() { "深色" } else { "亮色" };
                if ui.button(RichText::new(theme_label).size(12.0)).clicked() {
                    self.toggle_theme(ui.ctx());
                }
                ui.separator();
                if ui
                    .button(RichText::new("清空历史").size(11.0))
                    .on_hover_text("清空命令模式的历史记录")
                    .clicked()
                {
                    self.confirm_clear = Some(ClearAction::History);
                }
                if ui
                    .button(RichText::new("清空工作区").size(11.0))
                    .on_hover_text("删除当前所有变量和函数定义")
                    .clicked()
                {
                    self.confirm_clear = Some(ClearAction::Workspace);
                }
            });
        });
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("工作区").size(11.0).strong().color(dim()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{}", self.workspace.len()))
                        .size(10.0)
                        .color(muted()),
                );
            });
        });
        ui.add_space(6.0);
        if self.workspace.is_empty() {
            ui.label(
                RichText::new("赋值后会显示在这里。\n例如 a = 1/3")
                    .size(11.0)
                    .color(dim()),
            );
        } else {
            let items = self.workspace.clone();
            for item in items {
                if item.kind == "assumption" {
                    ui.label(
                        RichText::new(format!("assume {} {}", item.name, item.plain))
                            .size(10.5)
                            .color(violet()),
                    );
                    continue;
                }
                let label = if let Some(sig) = &item.signature {
                    format!("{}\n{}", sig, item.value_type)
                } else {
                    format!("{}\n{}", item.name, item.plain)
                };
                let response = ui.add(
                    egui::Button::new(RichText::new(label).size(11.0).color(text()))
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::NONE)
                        .min_size(egui::vec2(ui.available_width(), 0.0)),
                );
                if response.clicked() {
                    let insert = item.signature.clone().unwrap_or(item.name.clone());
                    self.insert(&insert);
                }
            }
            ui.add_space(4.0);
            if ui
                .button(RichText::new("清空工作区").size(10.5).color(dim()))
                .clicked()
            {
                self.confirm_clear = Some(ClearAction::Workspace);
            }
        }

        ui.add_space(14.0);
        ui.separator();
        ui.add_space(6.0);
        ui.label(RichText::new("常量").size(11.0).strong().color(dim()));
        ui.horizontal_wrapped(|ui| {
            for (name, label) in [("pi", "π"), ("e", "e"), ("i", "i")] {
                if ui
                    .small_button(RichText::new(label).size(13.0).color(violet()))
                    .on_hover_text(name)
                    .clicked()
                {
                    self.insert(name);
                }
            }
        });

        ui.add_space(14.0);
        ui.separator();
        ui.add_space(6.0);
        ui.label(RichText::new("语法速记").size(11.0).strong().color(dim()));
        for line in [
            "a = 1/3      精确分数",
            "f(x) = x^2+1 定义函数",
            "2x + 3(x-1) 隐式乘法",
            "piecewise    分段函数",
            "assume(x>0)  变量假设",
            "help(\"syntax\") 帮助",
        ] {
            ui.label(RichText::new(line).size(10.0).color(dim()));
        }
    }

    fn history_view(&mut self, ui: &mut egui::Ui) {
        if self.history.is_empty() {
            self.empty_state(ui);
            return;
        }
        let entries = self.history.clone();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.add_space(4.0);
                for entry in entries {
                    self.history_entry(ui, &entry);
                }
                ui.add_space(12.0);
            });
    }

    fn empty_state(&mut self, ui: &mut egui::Ui) {
        let top = ((ui.available_height() - 32.0) * 0.5).max(0.0);
        ui.add_space(top);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new("输入表达式，按 Enter 开始计算")
                    .size(21.0)
                    .color(muted()),
            );
        });
    }

    fn history_entry(&mut self, ui: &mut egui::Ui, entry: &HistoryEntry) {
        ui.add_space(8.0);
        egui::Frame::new()
            .fill(panel())
            .stroke(Stroke::new(1.0, border()))
            .corner_radius(10.0)
            .inner_margin(10.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("›").size(15.0).color(accent()));
                    ui.label(
                        RichText::new(&entry.input)
                            .size(12.5)
                            .monospace()
                            .color(text()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(&entry.at).size(9.5).color(dim()));
                        if ui
                            .small_button(RichText::new("复制").size(9.0).color(dim()))
                            .clicked()
                        {
                            self.copy(ui.ctx(), &entry.input, "输入");
                        }
                    });
                });
            });
        for result in &entry.results {
            self.result_card(ui, result);
        }
    }

    fn result_card(&mut self, ui: &mut egui::Ui, result: &DisplayResult) {
        if result.suppressed {
            ui.label(RichText::new("（分号抑制输出）").size(10.0).color(dim()));
            return;
        }
        let is_error = !result.ok;
        let fill = if is_error { error_bg() } else { panel2() };
        let stroke = if is_error {
            Stroke::new(1.0, error_border())
        } else {
            Stroke::new(1.0, border())
        };
        egui::Frame::new()
            .fill(fill)
            .stroke(stroke)
            .corner_radius(12.0)
            .inner_margin(12.0)
            .show(ui, |ui| {
                if is_error {
                    ui.label(
                        RichText::new("计算出错")
                            .size(11.5)
                            .strong()
                            .color(danger()),
                    );
                    ui.label(
                        RichText::new(result.error.clone().unwrap_or_default())
                            .size(12.0)
                            .color(error_text()),
                    );
                    if let Some(pos) = result.error_pos {
                        ui.label(
                            RichText::new(format!("位置：第 {} 个字符", pos + 1))
                                .size(9.5)
                                .color(dim()),
                        );
                    }
                    return;
                }
                ui.horizontal(|ui| {
                    badge(ui, &result.value_type, accent());
                    if self.settings.display_mode == "approx" && result.approx.is_some() {
                        badge(ui, "近似", warn());
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{:.1} ms", result.elapsed_ms))
                                .size(9.0)
                                .color(dim()),
                        );
                    });
                });
                ui.add_space(5.0);
                let approx_primary =
                    self.settings.display_mode == "approx" && result.approx_unicode.is_some();
                let prefix = if result.kind == "assign" {
                    result
                        .name
                        .as_ref()
                        .map(|name| format!("{name} = "))
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                let primary = if approx_primary {
                    result.approx_unicode.clone().unwrap_or_default()
                } else {
                    format!("{prefix}{}", result.unicode)
                };
                self.output_text(ui, &primary);
                if !approx_primary {
                    if let Some(approx) = &result.approx_unicode {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(if result.approx_exact { "=" } else { "≈" })
                                    .size(11.0)
                                    .color(if result.approx_exact {
                                        accent()
                                    } else {
                                        warn()
                                    }),
                            );
                            ui.label(RichText::new(approx).size(11.0).monospace().color(muted()));
                        });
                    }
                } else if !result.unicode.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("精确值").size(9.5).color(dim()));
                        ui.label(
                            RichText::new(&result.unicode)
                                .size(10.5)
                                .monospace()
                                .color(muted()),
                        );
                    });
                }
                if let Some(note) = &result.note {
                    ui.add_space(4.0);
                    ui.label(RichText::new(note).size(10.0).color(muted()));
                }
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    if ui
                        .small_button(RichText::new("复制").size(9.5).color(dim()))
                        .clicked()
                    {
                        self.copy(ui.ctx(), &result.plain, "结果");
                    }
                    if ui
                        .small_button(RichText::new("插入").size(9.5).color(dim()))
                        .clicked()
                    {
                        self.insert(&result.plain);
                    }
                });
            });
        ui.add_space(4.0);
    }

    fn output_text(&self, ui: &mut egui::Ui, value: &str) {
        if value.chars().count() > 1200 {
            egui::ScrollArea::vertical()
                .max_height(170.0)
                .show(ui, |ui| {
                    ui.label(RichText::new(value).size(12.0).monospace().color(text()));
                });
        } else {
            ui.label(RichText::new(value).size(19.0).monospace().color(text()));
        }
    }

    fn composer(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("›").size(20.0).color(accent()));
            let button_width = 40.0;
            let input_width = (ui.available_width() - button_width - 18.0).max(220.0);
            let response = ui.add(
                TextEdit::singleline(&mut self.input)
                    .hint_text("输入表达式，例如 1/3 + 1/6、diff(x^3*sin(x), x)")
                    .font(TextStyle::Monospace)
                    .text_color(text())
                    .background_color(panel2())
                    .desired_width(input_width)
                    .margin(egui::Margin::symmetric(10, 8)),
            );
            let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let clicked = run_icon_button(ui, egui::vec2(button_width, 36.0))
                .on_hover_text("计算 (Enter)")
                .clicked();
            if clicked || enter {
                self.execute_input();
                response.request_focus();
            }
            if self.focus_input {
                response.request_focus();
                self.focus_input = false;
            }
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("命令模式 · Enter 计算 · 精确优先 · 大数 · 复数 · 符号运算")
                    .size(9.0)
                    .color(dim()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{} 条历史", self.history.len()))
                        .size(9.0)
                        .color(dim()),
                );
            });
        });
        ui.add_space(4.0);
    }

    fn script_view(&mut self, ui: &mut egui::Ui) {
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("脚本").size(13.0).strong().color(violet()));
            ui.label(
                RichText::new("多行编辑 · Ctrl+Enter 运行 · 变量与工作区共享")
                    .size(10.0)
                    .color(dim()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let run =
                    run_icon_button(ui, egui::vec2(40.0, 28.0)).on_hover_text("运行 (Ctrl+Enter)");
                if run.clicked() {
                    self.execute_script();
                }
                if ui
                    .small_button(RichText::new("导出").size(10.5).color(dim()))
                    .on_hover_text("导出脚本 (Ctrl+S)")
                    .clicked()
                {
                    self.open_file_dialog(FileAction::Export);
                }
                if ui
                    .small_button(RichText::new("导入").size(10.5).color(dim()))
                    .on_hover_text("导入脚本 (Ctrl+O)，也可以直接把文件拖进窗口")
                    .clicked()
                {
                    self.open_file_dialog(FileAction::Import);
                }
                if self.script.trim().is_empty() {
                    if ui
                        .small_button(RichText::new("载入示例").size(10.5).color(dim()))
                        .clicked()
                    {
                        self.script = sample_script();
                        self.focus_script = true;
                    }
                } else if ui
                    .small_button(RichText::new("清空").size(10.5).color(dim()))
                    .clicked()
                {
                    self.script.clear();
                    self.focus_script = true;
                }
            });
        });
        ui.add_space(6.0);
        let editor_height = (ui.available_height() - 22.0).max(120.0);
        let rows = (((editor_height - 24.0) / 15.5) as usize).max(6);
        let mut response = None;
        egui::ScrollArea::vertical()
            .id_salt("script_editor")
            .max_height(editor_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let width = ui.available_width();
                response = Some(ui.add(
                    TextEdit::multiline(&mut self.script)
                        .hint_text(
                            "# 每行一条语句，Ctrl+Enter 运行\n# 变量与函数定义可跨行复用，# 是注释，; 抑制该句输出",
                        )
                        .font(TextStyle::Monospace)
                        .text_color(text())
                        .background_color(panel2())
                        .desired_width(width)
                        .desired_rows(rows)
                        .margin(egui::Margin::symmetric(12, 10)),
                ));
            });
        let response = response.expect("script editor");
        if response.has_focus()
            && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter))
        {
            self.execute_script();
        }
        if self.focus_script {
            response.request_focus();
            self.focus_script = false;
        }
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Ctrl+Enter 运行 · 遇到错误停止后续语句 · 分号抑制该句输出")
                    .size(9.0)
                    .color(dim()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let lines = self
                    .script
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .count();
                ui.label(RichText::new(format!("{lines} 行")).size(9.0).color(dim()));
            });
        });
        ui.add_space(2.0);
    }

    fn script_results(&mut self, ui: &mut egui::Ui) {
        let workspace = self.workspace.clone();
        let last = self.last_script.clone();

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("运行结果").size(12.0).strong().color(text()));
            if let Some(run) = &last {
                let outputs = run.results.iter().filter(|r| !r.suppressed).count();
                badge(ui, &format!("{outputs} 条输出"), muted());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(run) = &last {
                    ui.label(RichText::new(&run.at).size(9.0).color(dim()));
                }
            });
        });
        ui.add_space(2.0);
        egui::Panel::bottom("script_final")
            .exact_size(100.0)
            .show_separator_line(true)
            .frame(
                egui::Frame::new()
                    .fill(bg())
                    .inner_margin(egui::Margin::symmetric(0, 4)),
            )
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("script_final_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.label(RichText::new("最终结果").size(10.5).strong().color(dim()));
                        ui.add_space(3.0);
                        let Some(run) = &last else {
                            ui.label(
                                RichText::new("运行脚本后，这里显示最终结果。")
                                    .size(10.5)
                                    .color(dim()),
                            );
                            return;
                        };
                        if let Some((index, result)) =
                            run.results.iter().enumerate().find(|(_, r)| !r.ok)
                        {
                            ui.label(
                                RichText::new(format!("第 {} 条语句出错", index + 1))
                                    .size(10.5)
                                    .strong()
                                    .color(danger()),
                            );
                            ui.label(
                                RichText::new(result.error.clone().unwrap_or_default())
                                    .size(11.5)
                                    .color(error_text()),
                            );
                            return;
                        }
                        let Some(result) = run.results.iter().rev().find(|r| !r.suppressed) else {
                            ui.label(
                                RichText::new("没有可显示的输出；可在脚本末尾写一个表达式。")
                                    .size(10.5)
                                    .color(dim()),
                            );
                            return;
                        };
                        let prefix = if result.kind == "assign" {
                            result
                                .name
                                .as_ref()
                                .map(|name| format!("{name} = "))
                                .unwrap_or_default()
                        } else {
                            String::new()
                        };
                        ui.horizontal(|ui| {
                            badge(ui, &result.value_type, accent());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .small_button(RichText::new("复制").size(9.0).color(dim()))
                                        .clicked()
                                    {
                                        self.copy(ui.ctx(), &result.plain, "结果");
                                    }
                                },
                            );
                        });
                        ui.add_space(3.0);
                        ui.label(
                            RichText::new(format!("{prefix}{}", result.unicode))
                                .size(18.0)
                                .monospace()
                                .color(text()),
                        );
                        if let Some(approx) = &result.approx_unicode {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(if result.approx_exact { "=" } else { "≈" })
                                        .size(11.0)
                                        .color(if result.approx_exact {
                                            accent()
                                        } else {
                                            warn()
                                        }),
                                );
                                ui.label(
                                    RichText::new(approx).size(11.0).monospace().color(muted()),
                                );
                            });
                        }
                    });
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(bg())
                    .inner_margin(egui::Margin::ZERO),
            )
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("script_variables")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 4.0;
                        ui.label(RichText::new("变量").size(10.5).strong().color(dim()));
                        ui.add_space(2.0);
                        if workspace.is_empty() {
                            ui.label(RichText::new("（暂无变量）").size(10.5).color(dim()));
                        } else {
                            for item in &workspace {
                                ui.horizontal(|ui| {
                                    let row = if item.kind == "function" {
                                        format!(
                                            "{} 已定义",
                                            item.signature
                                                .clone()
                                                .unwrap_or_else(|| item.name.clone())
                                        )
                                    } else {
                                        format!("{} = {}", item.name, item.unicode)
                                    };
                                    ui.label(
                                        RichText::new(row).size(12.0).monospace().color(text()),
                                    );
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new(&item.value_type).size(9.0).color(dim()),
                                    );
                                });
                            }
                        }
                    });
            });
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut open = self.settings_open;
        egui::Window::new("设置")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.set_width(380.0);
                ui.label(RichText::new("外观").size(11.0).color(muted()));
                ui.horizontal(|ui| {
                    if ui.selectable_label(!is_light(), "深色").clicked() {
                        self.set_theme("dark", ctx);
                    }
                    if ui.selectable_label(is_light(), "浅色").clicked() {
                        self.set_theme("light", ctx);
                    }
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);
                ui.label(RichText::new("近似精度").size(11.0).color(muted()));
                let mut precision = self.settings.precision;
                if ui
                    .add(
                        egui::Slider::new(&mut precision, 10..=500)
                            .text("位有效数字")
                            .clamping(egui::SliderClamping::Always),
                    )
                    .changed()
                {
                    self.settings.precision = precision;
                    self.apply_settings();
                }
                ui.add_space(8.0);
                ui.label(RichText::new("角度制").size(11.0).color(muted()));
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(self.settings.angle_mode == "rad", "弧度 rad")
                        .clicked()
                    {
                        self.settings.angle_mode = "rad".into();
                        self.apply_settings();
                    }
                    if ui
                        .selectable_label(self.settings.angle_mode == "deg", "角度 deg")
                        .clicked()
                    {
                        self.settings.angle_mode = "deg".into();
                        self.apply_settings();
                    }
                });
                ui.add_space(8.0);
                ui.label(RichText::new("结果显示").size(11.0).color(muted()));
                ui.horizontal(|ui| {
                    for (value, label) in [
                        ("exact", "仅精确"),
                        ("both", "精确 + 近似"),
                        ("approx", "近似优先"),
                    ] {
                        if ui
                            .selectable_label(self.settings.display_mode == value, label)
                            .clicked()
                        {
                            self.settings.display_mode = value.into();
                            self.apply_settings();
                        }
                    }
                });
                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!("状态文件：{}", state_path().display()))
                        .size(9.0)
                        .color(dim()),
                );
                ui.add_space(10.0);
                ui.label(
                    RichText::new(
                        "FrogCalc 0.1.0 · Rust 原生版\n自研词法/语法解析器、按需高精度数值、符号代数、微积分与方程求解。",
                    )
                    .size(9.5)
                    .color(dim()),
                );
            });
        self.settings_open = open;
    }

    fn help_window(&mut self, ctx: &egui::Context) {
        if !self.help_open {
            return;
        }
        let mut open = self.help_open;
        egui::Window::new("帮助文档")
            .open(&mut open)
            .default_size([720.0, 640.0])
            .max_height(720.0)
            .min_width(560.0)
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(580.0)
                    .min_scrolled_height(540.0)
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .show(ui, |ui| {
                        ui.set_min_width(640.0);
                        help_content(ui);
                    });
            });
        self.help_open = open;
    }

    fn apply_settings(&mut self) {
        let settings = self.settings.to_settings();
        self.session.set_settings(settings);
        self.save();
    }

    fn toggle_theme(&mut self, ctx: &egui::Context) {
        let next = if is_light() { "dark" } else { "light" };
        self.set_theme(next, ctx);
    }

    fn set_theme(&mut self, theme: &str, ctx: &egui::Context) {
        self.settings.theme = theme.to_string();
        install_style(ctx, theme);
        ctx.request_repaint();
        self.save();
    }

    fn confirm_window(&mut self, ctx: &egui::Context) {
        let Some(action) = self.confirm_clear else {
            return;
        };
        let mut open = true;
        egui::Window::new("确认清空")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.set_width(300.0);
                let (title, detail) = match action {
                    ClearAction::Workspace => (
                        "确定清空工作区？",
                        "所有变量和函数定义都会被删除，此操作无法撤销。",
                    ),
                    ClearAction::History => ("确定清空历史记录？", "此操作无法撤销。"),
                };
                ui.label(RichText::new(title).size(13.0).strong().color(text()));
                ui.add_space(3.0);
                ui.label(RichText::new(detail).size(11.0).color(muted()));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("取消").clicked() {
                        self.confirm_clear = None;
                    }
                    if ui.button(RichText::new("清空").color(danger())).clicked() {
                        match action {
                            ClearAction::Workspace => self.clear_workspace(),
                            ClearAction::History => self.clear_history(),
                        }
                        self.confirm_clear = None;
                    }
                });
            });
        if !open {
            self.confirm_clear = None;
        }
    }

    fn toast_view(&mut self, ctx: &egui::Context) {
        let Some((message, created)) = &self.toast else {
            return;
        };
        if created.elapsed() > Duration::from_millis(1800) {
            self.toast = None;
            return;
        }
        egui::Area::new(egui::Id::unique("toast"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-22.0, -22.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(toast_bg())
                    .stroke(Stroke::new(1.0, accent()))
                    .corner_radius(8.0)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.label(RichText::new(message).size(11.0).color(accent()));
                    });
            });
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

impl eframe::App for FrogApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("top")
            .exact_size(56.0)
            .frame(
                egui::Frame::new()
                    .fill(bg())
                    .inner_margin(egui::Margin::symmetric(16, 8)),
            )
            .show(ui, |ui| self.top_bar(ui));

        if ui.input(|input| input.modifiers.command && input.key_pressed(egui::Key::O)) {
            self.open_file_dialog(FileAction::Import);
        }
        if ui.input(|input| input.modifiers.command && input.key_pressed(egui::Key::S)) {
            self.open_file_dialog(FileAction::Export);
        }

        if self.is_plot() {
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(central_bg())
                        .inner_margin(egui::Margin::symmetric(18, 10)),
                )
                .show(ui, |ui| self.plot_view(ui));
        } else if self.is_script() {
            egui::Panel::bottom("script_results")
                .resizable(true)
                .default_size(280.0)
                .min_size(170.0)
                .max_size(440.0)
                .frame(
                    egui::Frame::new()
                        .fill(bg())
                        .inner_margin(egui::Margin::symmetric(18, 8)),
                )
                .show(ui, |ui| self.script_results(ui));

            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(central_bg())
                        .inner_margin(egui::Margin::symmetric(18, 10)),
                )
                .show(ui, |ui| self.script_view(ui));
        } else {
            egui::Panel::left("workspace")
                .resizable(true)
                .default_size(240.0)
                .min_size(180.0)
                .max_size(340.0)
                .frame(
                    egui::Frame::new()
                        .fill(sidebar_bg())
                        .inner_margin(egui::Margin::symmetric(12, 10)),
                )
                .show(ui, |ui| self.sidebar(ui));

            egui::Panel::bottom("composer")
                .resizable(false)
                .exact_size(104.0)
                .frame(
                    egui::Frame::new()
                        .fill(bg())
                        .inner_margin(egui::Margin::symmetric(18, 8)),
                )
                .show(ui, |ui| self.composer(ui));

            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(central_bg())
                        .inner_margin(egui::Margin::symmetric(20, 12)),
                )
                .show(ui, |ui| self.history_view(ui));
        }

        self.settings_window(ui.ctx());
        self.help_window(ui.ctx());
        self.confirm_window(ui.ctx());
        self.file_dialog_window(ui.ctx());
        self.toast_view(ui.ctx());
        let dropped: Vec<PathBuf> = ui.ctx().input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .filter_map(|file| Some(file.path().to_path_buf()))
                .collect()
        });
        if let Some(path) = dropped.first() {
            if is_script_file(path) {
                self.import_script(path);
            } else {
                self.toast("只支持 .frogcalc / .frog / .calc / .txt 脚本文件");
            }
        }
        if self.should_scroll {
            self.should_scroll = false;
        }
    }
}

fn install_style(ctx: &egui::Context, theme_name: &str) {
    let light = theme_name == "light";
    set_light(light);
    let theme = if light {
        egui::Theme::Light
    } else {
        egui::Theme::Dark
    };
    ctx.set_theme(if light {
        egui::ThemePreference::Light
    } else {
        egui::ThemePreference::Dark
    });
    let mut visuals = if light {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    visuals.panel_fill = bg();
    visuals.window_fill = panel();
    visuals.extreme_bg_color = panel2();
    visuals.faint_bg_color = panel2();
    visuals.override_text_color = Some(text());
    visuals.hyperlink_color = accent();
    visuals.warn_fg_color = warn();
    visuals.error_fg_color = danger();
    visuals.code_bg_color = panel2();
    visuals.widgets.noninteractive.bg_fill = panel();
    visuals.widgets.noninteractive.weak_bg_fill = panel();
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, text());
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, border());
    visuals.widgets.inactive.bg_fill = panel2();
    visuals.widgets.inactive.weak_bg_fill = panel2();
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, muted());
    visuals.widgets.hovered.bg_fill = hover_bg();
    visuals.widgets.hovered.weak_bg_fill = hover_bg();
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, text());
    visuals.widgets.active.bg_fill = active_bg();
    visuals.widgets.active.weak_bg_fill = active_bg();
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, accent());
    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(
        accent().r(),
        accent().g(),
        accent().b(),
        if light { 70 } else { 60 },
    );
    visuals.selection.stroke = Stroke::new(1.0, accent());
    visuals.window_stroke = Stroke::new(1.0, border());
    visuals.window_corner_radius = 10.0.into();
    ctx.set_visuals(visuals);
    ctx.style_mut_of(theme, |style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        style.text_styles.insert(
            TextStyle::Monospace,
            egui::FontId::new(13.0, FontFamily::Monospace),
        );
    });
}

fn install_fonts(ctx: &egui::Context) {
    let candidates = [
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\Deng.ttf",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simsun.ttc",
    ];
    for path in candidates {
        if let Ok(bytes) = fs::read(path) {
            let mut fonts = egui::FontDefinitions::default();
            fonts
                .font_data
                .insert("cjk".to_owned(), egui::FontData::from_owned(bytes).into());
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, "cjk".to_owned());
            fonts
                .families
                .entry(FontFamily::Monospace)
                .or_default()
                .push("cjk".to_owned());
            ctx.set_fonts(fonts);
            break;
        }
    }
}

fn badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(
            color.r(),
            color.g(),
            color.b(),
            18,
        ))
        .stroke(Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 55),
        ))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(9.5).color(color));
        });
}

fn nice_step(range: f64) -> f64 {
    if !range.is_finite() || range <= 0.0 {
        return 1.0;
    }
    let raw = range / 8.0;
    let exponent = raw.log10().floor();
    let base = 10f64.powf(exponent);
    let fraction = raw / base;
    let nice = if fraction <= 1.0 {
        1.0
    } else if fraction <= 2.0 {
        2.0
    } else if fraction <= 5.0 {
        5.0
    } else {
        10.0
    };
    nice * base
}

fn trim_number(value: f64) -> String {
    if !value.is_finite() {
        return "-".into();
    }
    let text = format!("{value:.4}");
    let text = text.trim_end_matches('0').trim_end_matches('.').to_string();
    if text == "-0" {
        "0".into()
    } else {
        text
    }
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}

fn plot_area(rect: egui::Rect) -> egui::Rect {
    rect.shrink2(egui::vec2(34.0, 16.0))
}

fn draw_axes_grid(painter: &egui::Painter, area: egui::Rect, x0: f64, x1: f64, y0: f64, y1: f64) {
    let to_x = |x: f64| area.left() + ((x - x0) / (x1 - x0)) as f32 * area.width();
    let to_y = |y: f64| area.bottom() - ((y - y0) / (y1 - y0)) as f32 * area.height();
    let step_x = nice_step(x1 - x0);
    let step_y = nice_step(y1 - y0);
    let grid = Color32::from_rgba_unmultiplied(border().r(), border().g(), border().b(), 140);
    let mut gx = (x0 / step_x).ceil() * step_x;
    while gx <= x1 {
        let px = to_x(gx);
        painter.line_segment(
            [egui::pos2(px, area.top()), egui::pos2(px, area.bottom())],
            Stroke::new(1.0, grid),
        );
        painter.text(
            egui::pos2(px + 2.0, area.bottom() + 2.0),
            egui::Align2::LEFT_TOP,
            trim_number(gx),
            egui::FontId::proportional(8.5),
            dim(),
        );
        gx += step_x;
    }
    let mut gy = (y0 / step_y).ceil() * step_y;
    while gy <= y1 {
        let py = to_y(gy);
        painter.line_segment(
            [egui::pos2(area.left(), py), egui::pos2(area.right(), py)],
            Stroke::new(1.0, grid),
        );
        painter.text(
            egui::pos2(area.left() - 3.0, py),
            egui::Align2::RIGHT_CENTER,
            trim_number(gy),
            egui::FontId::proportional(8.5),
            dim(),
        );
        gy += step_y;
    }
    let axis = Stroke::new(1.2, muted());
    if x0 <= 0.0 && x1 >= 0.0 {
        let px = to_x(0.0);
        painter.line_segment(
            [egui::pos2(px, area.top()), egui::pos2(px, area.bottom())],
            axis,
        );
    }
    if y0 <= 0.0 && y1 >= 0.0 {
        let py = to_y(0.0);
        painter.line_segment(
            [egui::pos2(area.left(), py), egui::pos2(area.right(), py)],
            axis,
        );
    }
}

fn draw_curve(
    painter: &egui::Painter,
    rect: egui::Rect,
    points: &[Option<(f64, f64)>],
    x_min: f64,
    x_max: f64,
    auto_y: bool,
    fixed_y_min: f64,
    fixed_y_max: f64,
) {
    let area = plot_area(rect);
    let (x0, x1) = if (x_max - x_min).abs() < 1e-12 {
        (x_min - 1.0, x_max + 1.0)
    } else {
        (x_min, x_max)
    };
    let (mut y0, mut y1) = (fixed_y_min, fixed_y_max);
    if auto_y {
        let mut low = f64::INFINITY;
        let mut high = f64::NEG_INFINITY;
        for (_, y) in points.iter().flatten() {
            if y.is_finite() {
                low = low.min(*y);
                high = high.max(*y);
            }
        }
        if !low.is_finite() || !high.is_finite() {
            low = -1.0;
            high = 1.0;
        }
        if (high - low).abs() < 1e-9 {
            low -= 1.0;
            high += 1.0;
        }
        let padding = (high - low) * 0.08;
        y0 = low - padding;
        y1 = high + padding;
    }
    if (y1 - y0).abs() < 1e-12 {
        y1 = y0 + 1.0;
    }
    let to_screen = |x: f64, y: f64| {
        let px = area.left() + ((x - x0) / (x1 - x0)) as f32 * area.width();
        let py = area.bottom() - ((y - y0) / (y1 - y0)) as f32 * area.height();
        egui::pos2(px, py)
    };
    draw_axes_grid(painter, area, x0, x1, y0, y1);

    let stroke = Stroke::new(1.8, accent());
    let y_span = y1 - y0;
    let mut segment: Vec<egui::Pos2> = Vec::new();
    let mut previous: Option<f64> = None;
    for point in points {
        match point {
            Some((x, y)) if y.is_finite() => {
                if should_break(previous, Some(*y), y_span) && !segment.is_empty() {
                    if segment.len() > 1 {
                        painter.add(egui::Shape::line(std::mem::take(&mut segment), stroke));
                    } else {
                        segment.clear();
                    }
                }
                segment.push(to_screen(*x, *y));
                previous = Some(*y);
            }
            _ => {
                if segment.len() > 1 {
                    painter.add(egui::Shape::line(std::mem::take(&mut segment), stroke));
                } else {
                    segment.clear();
                }
                previous = None;
            }
        }
    }
    if segment.len() > 1 {
        painter.add(egui::Shape::line(segment, stroke));
    }
}

/// 2D 隐函数曲线：线段已经由 marching triangles 给出，直接映射并绘制。
fn draw_implicit_curve(
    painter: &egui::Painter,
    rect: egui::Rect,
    segments: &[[(f64, f64); 2]],
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
) {
    let area = plot_area(rect);
    draw_axes_grid(painter, area, x0, x1, y0, y1);
    let to_screen = |x: f64, y: f64| {
        let px = area.left() + ((x - x0) / (x1 - x0)) as f32 * area.width();
        let py = area.bottom() - ((y - y0) / (y1 - y0)) as f32 * area.height();
        egui::pos2(px, py)
    };
    let stroke = Stroke::new(1.6, accent());
    for segment in segments {
        painter.line_segment(
            [
                to_screen(segment[0].0, segment[0].1),
                to_screen(segment[1].0, segment[1].1),
            ],
            stroke,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_surface(
    painter: &egui::Painter,
    rect: egui::Rect,
    values: &[Option<f64>],
    nx: usize,
    ny: usize,
    y0: f64,
    y1: f64,
    x_min: f64,
    x_max: f64,
    yaw: f32,
    pitch: f32,
    wireframe: bool,
) {
    let (x0, x1) = if (x_max - x_min).abs() < 1e-12 {
        (x_min - 1.0, x_max + 1.0)
    } else {
        (x_min, x_max)
    };
    let (y0, y1) = if (y1 - y0).abs() < 1e-12 {
        (y0 - 1.0, y0 + 1.0)
    } else {
        (y0, y1)
    };
    let mut low = f64::INFINITY;
    let mut high = f64::NEG_INFINITY;
    for value in values.iter().flatten() {
        if value.is_finite() {
            low = low.min(*value);
            high = high.max(*value);
        }
    }
    if !low.is_finite() || !high.is_finite() {
        low = -1.0;
        high = 1.0;
    }
    if (high - low).abs() < 1e-9 {
        low -= 1.0;
        high += 1.0;
    }
    let padding = (high - low) * 0.05;
    let (z0, z1) = (low - padding, high + padding);
    let cos_yaw = yaw.cos();
    let sin_yaw = yaw.sin();
    let cos_pitch = pitch.cos();
    let sin_pitch = pitch.sin();
    let scale = rect.width().min(rect.height()) as f64 * 0.36;
    let center = rect.center();
    let project = |x: f64, y: f64, z: f64| -> (egui::Pos2, f64) {
        let xn = ((x - x0) / (x1 - x0)) * 2.0 - 1.0;
        let yn = ((y - y0) / (y1 - y0)) * 2.0 - 1.0;
        let zn = ((z - z0) / (z1 - z0)) * 2.0 - 1.0;
        let rx = xn * cos_yaw as f64 - yn * sin_yaw as f64;
        let spin = xn * sin_yaw as f64 + yn * cos_yaw as f64;
        // 屏幕 y 向下：抬高的点向上移动，视线深度随 z 减小（高处更靠近观察者）。
        let ry = -(zn * cos_pitch as f64 + spin * sin_pitch as f64);
        let depth = spin * cos_pitch as f64 - zn * sin_pitch as f64;
        (
            egui::pos2(
                center.x + (rx * scale) as f32,
                center.y + (ry * scale) as f32,
            ),
            depth,
        )
    };
    let x_at = |i: usize| x0 + (x1 - x0) * i as f64 / (nx.saturating_sub(1).max(1)) as f64;
    let y_at = |j: usize| y0 + (y1 - y0) * j as f64 / (ny.saturating_sub(1).max(1)) as f64;
    let value_at = |i: usize, j: usize| values.get(j * nx + i).copied().flatten();

    if wireframe {
        let stroke = Stroke::new(1.0, accent().gamma_multiply(0.85));
        for j in 0..ny {
            let mut line = Vec::new();
            for i in 0..nx {
                if let Some(z) = value_at(i, j) {
                    line.push(project(x_at(i), y_at(j), z).0);
                } else if line.len() > 1 {
                    painter.add(egui::Shape::line(std::mem::take(&mut line), stroke));
                } else {
                    line.clear();
                }
            }
            if line.len() > 1 {
                painter.add(egui::Shape::line(line, stroke));
            }
        }
        for i in 0..nx {
            let mut line = Vec::new();
            for j in 0..ny {
                if let Some(z) = value_at(i, j) {
                    line.push(project(x_at(i), y_at(j), z).0);
                } else if line.len() > 1 {
                    painter.add(egui::Shape::line(
                        std::mem::take(&mut line),
                        Stroke::new(1.0, violet().gamma_multiply(0.7)),
                    ));
                } else {
                    line.clear();
                }
            }
            if line.len() > 1 {
                painter.add(egui::Shape::line(
                    line,
                    Stroke::new(1.0, violet().gamma_multiply(0.7)),
                ));
            }
        }
        return;
    }

    let mut cells: Vec<(f64, [egui::Pos2; 4], f64)> = Vec::new();
    for j in 0..ny.saturating_sub(1) {
        for i in 0..nx.saturating_sub(1) {
            let corners = [
                value_at(i, j),
                value_at(i + 1, j),
                value_at(i + 1, j + 1),
                value_at(i, j + 1),
            ];
            let Some(zs) = corners
                .iter()
                .map(|z| *z)
                .collect::<Option<Vec<f64>>>()
                .map(|v| {
                    let average = v.iter().sum::<f64>() / v.len() as f64;
                    average
                })
            else {
                continue;
            };
            let mut positions = [egui::Pos2::ZERO; 4];
            let mut depth = 0.0;
            let mut valid = true;
            for (k, (pi, pj)) in [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)]
                .iter()
                .enumerate()
            {
                if let Some(z) = value_at(*pi, *pj) {
                    let (pos, d) = project(x_at(*pi), y_at(*pj), z);
                    positions[k] = pos;
                    depth += d;
                } else {
                    valid = false;
                    break;
                }
            }
            if valid {
                cells.push((depth / 4.0, positions, zs));
            }
        }
    }
    cells.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let span = (z1 - z0).max(1e-12);
    for (_, positions, z) in cells {
        let t = ((z - z0) / span).clamp(0.0, 1.0) as f32;
        let base = lerp_color(accent(), violet(), t * 0.75);
        let shade = 0.45 + 0.65 * (1.0 - t);
        let color = Color32::from_rgba_unmultiplied(
            (base.r() as f32 * shade) as u8,
            (base.g() as f32 * shade) as u8,
            (base.b() as f32 * shade) as u8,
            232,
        );
        painter.add(egui::Shape::convex_polygon(
            positions.to_vec(),
            color,
            Stroke::NONE,
        ));
    }
}

fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize3(value: [f64; 3]) -> [f64; 3] {
    let length = (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt();
    if length < 1e-12 {
        [0.0, 0.0, 0.0]
    } else {
        [value[0] / length, value[1] / length, value[2] / length]
    }
}

/// 3D 隐函数等值面：三角网格按深度排序后填充，用视线空间法线做简单光照。
#[allow(clippy::too_many_arguments)]
fn draw_implicit_surface(
    painter: &egui::Painter,
    rect: egui::Rect,
    triangles: &[[[f64; 3]; 3]],
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
    z0: f64,
    z1: f64,
    yaw: f32,
    pitch: f32,
) {
    let cos_yaw = yaw.cos() as f64;
    let sin_yaw = yaw.sin() as f64;
    let cos_pitch = pitch.cos() as f64;
    let sin_pitch = pitch.sin() as f64;
    let scale = rect.width().min(rect.height()) as f64 * 0.36;
    let center = rect.center();
    let project = |point: [f64; 3]| -> (egui::Pos2, f64) {
        let xn = ((point[0] - x0) / (x1 - x0)) * 2.0 - 1.0;
        let yn = ((point[1] - y0) / (y1 - y0)) * 2.0 - 1.0;
        let zn = ((point[2] - z0) / (z1 - z0)) * 2.0 - 1.0;
        let rx = xn * cos_yaw - yn * sin_yaw;
        let spin = xn * sin_yaw + yn * cos_yaw;
        let ry = -(zn * cos_pitch + spin * sin_pitch);
        let depth = spin * cos_pitch - zn * sin_pitch;
        (
            egui::pos2(
                center.x + (rx * scale) as f32,
                center.y + (ry * scale) as f32,
            ),
            depth,
        )
    };
    let light = normalize3([0.35, -0.35, 0.87]);
    let mut faces: Vec<(f64, [egui::Pos2; 3], f32, f32)> = Vec::with_capacity(triangles.len());
    for triangle in triangles {
        let (p0, d0) = project(triangle[0]);
        let (p1, d1) = project(triangle[1]);
        let (p2, d2) = project(triangle[2]);
        let normal = normalize3(cross3(
            sub3(triangle[1], triangle[0]),
            sub3(triangle[2], triangle[0]),
        ));
        let spin = normal[0] * sin_yaw + normal[1] * cos_yaw;
        let ny = -(normal[2] * cos_pitch + spin * sin_pitch);
        let nx = normal[0] * cos_yaw - normal[1] * sin_yaw;
        let nz = spin * cos_pitch - normal[2] * sin_pitch;
        let diffuse = (nx * light[0] + ny * light[1] + nz * light[2]).abs() as f32;
        let height =
            ((triangle[0][2] + triangle[1][2] + triangle[2][2]) / 3.0 - z0) / (z1 - z0).max(1e-12);
        faces.push((
            (d0 + d1 + d2) / 3.0,
            [p0, p1, p2],
            height.clamp(0.0, 1.0) as f32,
            diffuse,
        ));
    }
    faces.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    for (_, positions, height, diffuse) in faces {
        let base = lerp_color(accent(), violet(), height * 0.6);
        let shade = 0.35 + 0.65 * diffuse;
        let color = Color32::from_rgba_unmultiplied(
            (base.r() as f32 * shade) as u8,
            (base.g() as f32 * shade) as u8,
            (base.b() as f32 * shade) as u8,
            236,
        );
        painter.add(egui::Shape::convex_polygon(
            positions.to_vec(),
            color,
            Stroke::NONE,
        ));
    }
}

fn script_directory() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn is_script_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            matches!(
                ext.to_lowercase().as_str(),
                "frogcalc" | "frog" | "calc" | "txt"
            )
        })
        .unwrap_or(false)
}

fn run_icon_button(ui: &mut egui::Ui, size: egui::Vec2) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let fill = if response.hovered() {
            accent().gamma_multiply(1.12)
        } else {
            accent()
        };
        let painter = ui.painter();
        painter.rect_filled(rect, 9.0, fill);
        // 用几何图形绘制播放三角，包围盒与按钮同心，避免字体边距造成视觉偏移。
        let center = rect.center();
        painter.add(egui::Shape::convex_polygon(
            vec![
                center + egui::vec2(-5.5, -6.5),
                center + egui::vec2(-5.5, 6.5),
                center + egui::vec2(5.5, 0.0),
            ],
            button_text(),
            Stroke::NONE,
        ));
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

fn state_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("state.json")
}

fn legacy_state_path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("FrogCalc")
        .join("state.json")
}

fn read_state(path: &Path) -> Option<SavedState> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn load_state() -> (SavedState, bool) {
    let current = state_path();
    if let Some(state) = read_state(&current) {
        return (state, false);
    }
    let legacy = legacy_state_path();
    if legacy != current {
        if let Some(state) = read_state(&legacy) {
            return (state, true);
        }
    }
    (SavedState::default(), false)
}

fn time_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let secs = now % 86_400;
    let h = (secs / 3600 + 8) % 24;
    let m = (secs % 3600) / 60;
    format!("{h:02}:{m:02}")
}

fn sample_script() -> String {
    [
        "a = 1/3",
        "b = 2/3",
        "a + b",
        "",
        "sumto(n) = {",
        "    total = 0",
        "    for i in range(1, n) {",
        "        total = total + i",
        "    }",
        "    total",
        "}",
        "",
        "sumto(10)",
        "",
        "if sumto(10) == 55 {",
        "    tag = \"正确\"",
        "} else {",
        "    tag = \"错误\"",
        "}",
        "tag",
    ]
    .join("\n")
}

fn help_content(ui: &mut egui::Ui) {
    ui.label(
        RichText::new("FrogCalc 帮助")
            .size(24.0)
            .strong()
            .color(text()),
    );
    ui.label(
        RichText::new(
            "精确优先的桌面计算器与计算机代数系统。所有数字默认进入精确有理数域，只有显式近似或结果下方的近似值才使用高精度小数。",
        )
        .size(11.5)
        .color(muted()),
    );

    help_heading(ui, "语法");
    help_code(
        ui,
        &[
            "a = 1 / 3                赋值，得到精确分数",
            "f(x) = x^2 + 1           定义单表达式函数",
            "f(x) = { ... }           定义多语句函数，最后一句为返回值",
            "f(3)                     调用函数",
            "2x + 3(x - 1)            隐式乘法",
            "sqrt(2) * sqrt(2)        根式精确化简",
            "solve(x^2 - 2x - 3, x)   解方程",
            "x^2 - 2x - 3 == 0        方程表达式，可直接传给 solve",
            "if x > 0 { y = 1 }       条件分支，可加 else",
            "for i in range(1, 10) { total = total + i }  循环",
            "while / for 内可用 break 与 continue",
            "f(x) = piecewise(x < 0, -x, x)  分段函数",
            "assume(x > 0)            变量假设，增强化简",
            "{1, 2, 3}                列表字面量",
            "ans                      上一次结果",
            "# 这是行注释",
            "a = 2; b = 3; a + b      分号分隔语句，并抑制前两句输出",
        ],
    );
    ui.add_space(6.0);
    help_table(
        ui,
        &[
            ("+ - * /", "四则运算"),
            ("^ 或 **", "乘方，右结合；-2^2 = -4，2^3^2 = 512"),
            ("!", "后缀阶乘，如 5!"),
            ("== ", "方程等号；在 if/while 条件中判断相等"),
            ("< <= > >= !=", "大小比较，结果为 true / false"),
            ("and / or / not", "逻辑运算，支持短路"),
            ("if / while / for", "分支与循环，使用 { } 包围代码块"),
            ("break / continue", "跳出循环 / 跳过本次迭代"),
            (
                "piecewise(c1, v1[, c2, v2 ...][, 默认])",
                "分段函数，条件按顺序判断",
            ),
            ("assume(x > 0)", "为变量添加假设（>、>=、<、<=、!=）"),
            ("f(x) = { ... }", "多语句函数，最后一个语句为返回值"),
            ("range(a, b[, step])", "生成整数列表，包含 b"),
            ("true / false", "布尔值"),
            ("( )", "分组；括号内允许换行"),
            ("{ }", "列表；控制流中作为代码块"),
            ("#", "行注释"),
            (";", "语句分隔；该句输出被抑制"),
        ],
    );

    help_heading(ui, "输入模式");
    help_table(
        ui,
        &[
            ("命令模式", "底部单行输入，Enter 立即计算，适合逐条探索"),
            (
                "脚本模式",
                "上方多行编辑，Ctrl+Enter 运行整段脚本；下方列出变量与最终结果",
            ),
            (
                "绘图模式",
                "输入 f(x) 或 f(x, y)，在 2D 曲线与 3D 曲面之间切换，拖动旋转 3D 视角",
            ),
            (
                "脚本规则",
                "按书写顺序执行；遇到错误停止后续语句；; 抑制该句输出",
            ),
            (
                "脚本文件",
                "脚本模式支持导入/导出（Ctrl+O / Ctrl+S），也可以直接把脚本文件拖进窗口",
            ),
            (
                "清空按钮",
                "顶栏提供「清空工作区」「清空历史」，操作前会二次确认",
            ),
        ],
    );

    help_heading(ui, "脚本控制流与函数");
    ui.label(
        RichText::new(
            "控制流与函数定义语句本身不输出结果；在脚本末尾写一个表达式查看最终值。循环总次数上限 100000。",
        )
        .size(11.0)
        .color(muted()),
    );
    help_code(
        ui,
        &[
            "total = 0",
            "for i in range(1, 10) {",
            "    total = total + i",
            "}",
            "",
            "if total == 55 {",
            "    tag = \"正确\"",
            "} else {",
            "    tag = \"错误\"",
            "}",
            "tag",
        ],
    );
    ui.add_space(4.0);
    help_table(
        ui,
        &[
            ("if 条件 { ... }", "条件为真时执行，可加 else"),
            ("if ... { } else if ... { }", "多分支判断"),
            ("while 条件 { ... }", "条件为真时重复执行"),
            (
                "for i in range(1, 10) { ... }",
                "范围循环，包含终点；range(起, 终[, 步长])",
            ),
            ("for x in {1, 2, 3} { ... }", "遍历列表"),
            ("break", "立即结束最内层循环"),
            ("continue", "跳过本次迭代的剩余语句"),
            ("and / or / not", "逻辑运算，and / or 短路"),
            ("< <= > >= != ==", "比较，得到 true / false"),
        ],
    );

    ui.add_space(10.0);
    ui.label(
        RichText::new(
            "函数体用 { } 包含多条语句时，最后一个语句的值就是返回值。函数内的赋值是局部的，不会修改同名全局变量；支持递归。",
        )
        .size(11.0)
        .color(muted()),
    );
    help_code(
        ui,
        &[
            "fact(n) = {",
            "    if n <= 1 {",
            "        1",
            "    } else {",
            "        n * fact(n - 1)",
            "    }",
            "}",
            "fact(5)                  # 120",
        ],
    );
    ui.add_space(4.0);
    help_table(
        ui,
        &[
            ("f(x) = { ... }", "多语句函数；最后一个语句的值作为返回值"),
            ("局部变量", "函数内赋值不会修改同名全局变量"),
            ("递归", "可直接调用自身，调用深度上限 200"),
            ("函数内定义函数", "目前不支持"),
        ],
    );

    help_heading(ui, "数字、精确性与显示");
    ui.label(
        RichText::new(
            "整数、分数、有限小数和根式默认保持精确；sqrt(2) 保存为 2^(1/2)，sqrt(2)*sqrt(2) 会化成 2。复数使用 i，例如 (2+3i)*(4-5i)。",
        )
        .size(11.0)
        .color(muted()),
    );
    help_table(
        ui,
        &[
            ("1 / 3", "精确有理数，结果显示为分数"),
            ("0.1 + 0.2", "精确结果为 3/10，下方显示 ≈ 0.3"),
            ("1 / 2", "精确结果 1/2，下方显示 = 0.5（有限小数精确等价）"),
            ("1 / 3", "精确结果 1/3，下方显示 ≈ 0.333...（循环小数）"),
            (
                "N(expr[, digits])",
                "近似求值；默认 50 位有效数字，可指定 10–500 位",
            ),
            (
                "recurring(\"0.1(3)\")",
                "无限循环小数转精确分数（循环节用 ( ) 或 [ ] 包围）",
            ),
            ("setprecision(n)", "修改默认近似精度"),
            (
                "setangle(\"deg\")",
                "切换到角度制；setangle(\"rad\") 切回弧度",
            ),
        ],
    );

    help_heading(ui, "数值函数");
    help_table(
        ui,
        &[
            ("abs(x)", "绝对值"),
            ("sign(x)", "符号：-1、0、1"),
            ("floor(x) / ceil(x) / round(x)", "取整"),
            ("min(...) / max(...)", "最小值 / 最大值"),
            ("gcd(a, b, ...)", "最大公约数"),
            ("lcm(a, b, ...)", "最小公倍数"),
            ("mod(a, b)", "取模"),
            ("factorial(n) / n!", "阶乘"),
            ("binomial(n, k)", "二项式系数"),
        ],
    );

    help_heading(ui, "初等函数");
    help_table(
        ui,
        &[
            ("sqrt(x) / cbrt(x) / root(x, n)", "平方根 / 立方根 / n 次根"),
            ("pow(x, y)", "幂，等价于 x^y"),
            ("exp(x)", "自然指数函数"),
            ("ln(x) / log(x)", "自然对数"),
            ("log(x, b)", "以 b 为底的对数"),
            ("log10(x) / log2(x)", "常用对数 / 二进制对数"),
            ("sin(x) cos(x) tan(x)", "三角函数，默认弧度制"),
            ("asin(x) acos(x) atan(x)", "反三角函数"),
            ("sinh(x) cosh(x) tanh(x)", "双曲函数"),
        ],
    );

    help_heading(ui, "符号运算与微积分");
    help_table(
        ui,
        &[
            ("simplify(expr)", "合并同类项、通分、约去公因子"),
            ("expand(expr)", "展开乘积与整数幂"),
            ("factor(expr)", "一元多项式因式分解"),
            ("diff(expr, x[, n])", "求导，n 为阶数"),
            ("integrate(expr, x)", "不定积分（省略常数 C）"),
            ("integrate(expr, x, a, b)", "定积分"),
            ("solve(expr, x)", "解方程；表达式视为 = 0"),
            ("subs(expr, x, value)", "符号替换"),
        ],
    );

    help_heading(ui, "变量假设与分段函数");
    ui.label(
        RichText::new(
            "假设会随工作区一起保存。它影响结果展示与 simplify：例如 assume(x > 0) 之后，sqrt(x^2) 显示为 x、abs(x) 显示为 x；工作区里存放的仍是原始表达式。",
        )
        .size(11.0)
        .color(muted()),
    );
    help_table(
        ui,
        &[
            ("assume(x > 0)", "记录假设，支持 >、>=、<、<=、!= 与 0 比较"),
            ("assumptions()", "列出当前所有假设"),
            ("forget(x)", "删除变量 x 的假设"),
            ("clearassumptions()", "清除全部假设"),
            (
                "piecewise(c1, v1, c2, v2, 默认)",
                "分段函数；条件依次判断，只有全部为假时才取默认值",
            ),
            ("diff(piecewise(...), x)", "逐段求导，条件保持不变"),
        ],
    );

    help_heading(ui, "绘图");
    ui.label(
        RichText::new(
            "绘图模式重新采样并绘制当前表达式：2D 绘制 y = f(x)，3D 绘制 z = f(x, y)。3D 中 x、y 是自变量，z 是高度；也可以直接输入 z = ...，绘图会取等号右侧。还可以输入隐方程 F(x, y) = 0 或 F(x, y, z) = 0（例如 x^2 + y^2 + z^2 = 1），此时按零等值面绘制。用户函数、变量、分段函数与角度制设置都会生效。",
        )
        .size(11.0)
        .color(muted()),
    );
    help_table(
        ui,
        &[
            (
                "y = sin(x)",
                "2D 曲线；x 为自变量，y 由表达式决定，自动确定 y 范围",
            ),
            (
                "z = x^2 - y^2",
                "3D 曲面；x、y 为自变量，z 是高度（也可只写右侧表达式）",
            ),
            (
                "x^2 + y^2 = 1",
                "2D 隐函数曲线 F(x, y) = 0；圆、椭圆等闭曲线",
            ),
            (
                "x^2 + y^2 + z^2 = 1",
                "3D 隐函数曲面 F(x, y, z) = 0；球面、锥面等",
            ),
            (
                "自动识别",
                "输入框下方显示识别结果；含 z 或写成方程时自动按隐函数绘制",
            ),
            (
                "x ∈ [a, b]",
                "采样区间，3D 时另有 y 区间；隐函数 3D 还有 z 区间",
            ),
            ("分辨率", "隐函数的采样网格密度；隐函数 3D 默认 26"),
            ("自动范围", "隐函数第一次绘制会按曲面自动缩放；也可手动点击"),
            ("重置", "把 x / y / z 范围恢复为 [-10, 10] 后重新自动适应"),
            ("采样 / 网格", "显式 2D 采样点数量与显式 3D 网格密度"),
            ("线框", "显式 3D 只绘制网格线，不填充曲面"),
            ("绘制", "立即重新采样；修改表达式或范围后也会自动刷新"),
            (
                "自动 y 范围",
                "显式 2D 默认按数据自动缩放；固定范围外的部分会被裁剪并提示",
            ),
        ],
    );

    help_heading(ui, "工作区命令");
    help_table(
        ui,
        &[
            ("help(\"syntax\")", "查看内置语言帮助"),
            ("clear()", "清空工作区"),
            ("clear(\"x\")", "删除变量或函数 x"),
            ("vars()", "列出工作区内容"),
            ("setprecision(n)", "设置近似精度（10–500 位）"),
            ("setangle(\"deg\" / \"rad\")", "切换角度制"),
        ],
    );

    help_heading(ui, "常用示例");
    help_code(
        ui,
        &[
            "a = 1/3",
            "b = 2/3",
            "a + b                    # 1",
            "sqrt(8)                  # 2*sqrt(2)",
            "diff(x^3*sin(x), x)      # 求导",
            "integrate(x*exp(x), x)   # 分部积分",
            "integrate(1/(x^2-1), x)  # 有理函数积分",
            "solve(sin(x) == 0, x)    # 主值分支",
            "N(pi, 50)                # 50 位圆周率",
            "a = recurring(\"0.1(3)\")  # 2/15",
            "x^2 + y^2 + z^2 = 1      # 3D 球面（绘图模式）",
            "x^2 + y^2 = 1            # 2D 圆（绘图模式）",
        ],
    );

    help_heading(ui, "注意事项");
    for line in [
        "• 三角/周期方程只给出主值分支，不自动生成 + 2πk 通解。",
        "• 无法求出的积分会保留 integrate(...) 未求值形式，不会猜测结果。",
        "• 有理函数积分覆盖重根与二次不可约因子；更高次不可约因子仍需数值积分。",
        "• 高次多项式已使用 Berlekamp–Zassenhaus 分解；超过次数上限时退回有理根 + Durand–Kerner 数值求根。",
        "• 近似结果没有严格的最后一位正确性保证；需要精确时请保持分数/根式/符号形式。",
        "• 矩阵运算尚未实现；绘图目前只支持单条表达式，不支持参数曲线与隐函数。",
    ] {
        ui.label(RichText::new(line).size(10.5).color(muted()));
    }
}

fn help_heading(ui: &mut egui::Ui, title: &str) {
    ui.add_space(14.0);
    ui.label(RichText::new(title).size(14.0).strong().color(text()));
    ui.add_space(3.0);
}

fn help_table(ui: &mut egui::Ui, rows: &[(&str, &str)]) {
    egui::Grid::new(ui.next_auto_id())
        .num_columns(2)
        .spacing([18.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            for (name, desc) in rows {
                ui.label(RichText::new(*name).monospace().size(11.0).color(accent()));
                ui.label(RichText::new(*desc).size(11.0).color(muted()));
                ui.end_row();
            }
        });
}

fn help_code(ui: &mut egui::Ui, lines: &[&str]) {
    egui::Frame::new()
        .fill(panel())
        .stroke(Stroke::new(1.0, border()))
        .corner_radius(8.0)
        .inner_margin(10.0)
        .show(ui, |ui| {
            for line in lines {
                ui.label(RichText::new(*line).monospace().size(10.8).color(text()));
            }
        });
}

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1040.0, 700.0])
        .with_min_inner_size([860.0, 600.0])
        .with_title("FrogCalc");
    if let Some(icon) = load_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "FrogCalc",
        options,
        Box::new(|cc| Ok(Box::new(FrogApp::new(cc)))),
    )
}

fn load_icon() -> Option<egui::IconData> {
    let bytes = include_bytes!("../../../resources/icon.png");
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    Some(egui::IconData {
        rgba: image.as_raw().clone(),
        width: image.width(),
        height: image.height(),
    })
}
