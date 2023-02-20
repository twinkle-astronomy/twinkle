use std::ops::{Range, RangeInclusive};

use egui::plot::{Legend, Line, Plot, PlotPoint, PlotPoints, Points};

pub struct FocusGraph {
    points: Vec<[f64; 2]>,
    model: Option<crate::analysis::HyperbolicFit>,
    x_range: Range<f64>,
}

impl Default for FocusGraph {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            model: None,
            x_range: f64::MAX..f64::MIN,
        }
    }
}
impl FocusGraph {
    pub fn new<'a>(_cc: &'a eframe::CreationContext<'a>) -> Self {
        Default::default()
    }

    pub fn add_point(&mut self, point: [f64; 2]) {
        if point[0] < self.x_range.start {
            self.x_range.start = point[0];
        }
        if point[0] > self.x_range.end {
            self.x_range.end = point[0];
        }
        self.points.push(point);
        let fit = crate::analysis::HyperbolicFit::new(&self.points);
        match fit {
            Ok(model) => self.model = Some(model),
            Err(e) => {
                dbg!(e);
                self.model = None
            }
        }
    }
}

impl eframe::App for FocusGraph {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let points = self.points.clone();
        let points = PlotPoints::new(points);
        let points = Points::new(points).radius(10.0).name("Samples");

        let model = self.model.clone();
        let range = self.x_range.clone();

        egui::CentralPanel::default().show(ctx, move |ui| {
            ui.vertical(|ui| {
                let x_fmt = |x, _range: &RangeInclusive<f64>| format!("{:.0}", x);
                let y_fmt = |y, _range: &RangeInclusive<f64>| format!("{:.2}", y);
                let label_fmt =
                    |_s: &str, val: &PlotPoint| format!("step: {:.0}\nfwhm: {:.2}", val.x, val.y);
                Plot::new("my_plot")
                    .view_aspect(2.0)
                    .x_axis_formatter(x_fmt)
                    .y_axis_formatter(y_fmt)
                    .label_formatter(label_fmt)
                    .legend(Legend::default())
                    .show(ui, |plot_ui| {
                        plot_ui.points(points);

                        if let Some(model) = model {
                            let target = vec![[model.c, model.expected_y(model.c)]];

                            let target_pp = PlotPoints::new(target);
                            let target_p = Points::new(target_pp).name("Solution").radius(5.0);
                            plot_ui.points(target_p);

                            plot_ui.line(
                                Line::new(PlotPoints::from_explicit_callback(
                                    move |x| model.expected_y(x),
                                    range,
                                    512,
                                ))
                                .name("Hyperbolic Curve Fit"),
                            );
                        }
                    });
            });
        });
    }
}
