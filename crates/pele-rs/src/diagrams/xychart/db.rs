//! The model of an XY chart.

/// A text of the chart. Mermaid has a second kind of text, a markdown string, that its parser
/// never produces.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct XyText {
    pub text: String,
}

/// A value of a series, with the label that stands beside it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DataPoint {
    pub value: f64,
    pub label: String,
}

/// The x axis: either categories or a range of numbers.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum XAxis {
    Band {
        title: String,
        categories: Vec<String>,
    },
    Linear {
        title: String,
        min: f64,
        max: f64,
    },
}

impl XAxis {
    /// The title of the axis, whichever kind it is.
    pub(crate) fn title(&self) -> &str {
        match self {
            XAxis::Band { title, .. } | XAxis::Linear { title, .. } => title,
        }
    }
}

/// The y axis: a range of numbers.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LinearAxis {
    pub title: String,
    pub min: f64,
    pub max: f64,
}

/// Each value with its place on the x axis: a category, or a number written as text. A category
/// with no value has none.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct PlotData {
    pub entries: Vec<(String, Option<f64>)>,
}

/// Whether a series is a line or a bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlotKind {
    Line,
    Bar,
}

impl PlotKind {
    #[allow(
        dead_code,
        reason = "read by the compat test, which is only built for the tests"
    )]
    pub(crate) fn name(self) -> &'static str {
        match self {
            PlotKind::Line => "line",
            PlotKind::Bar => "bar",
        }
    }
}

/// A series of the chart.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Plot {
    pub kind: PlotKind,
    pub title: String,
    pub data: PlotData,
    /// The labels of the points of a line, when the series has any.
    pub point_labels: Option<Vec<String>>,
}

/// How the chart lies on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Orientation {
    Vertical,
    Horizontal,
}

/// The chart as Mermaid's parser builds it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct XyChartDb {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub orientation: Option<Orientation>,
    pub x_axis: XAxis,
    pub y_axis: LinearAxis,
    /// Whether the x range comes from a statement and not from the data.
    pub has_set_x_axis: bool,
    /// Whether the y range comes from a statement and not from the data.
    pub has_set_y_axis: bool,
    pub plots: Vec<Plot>,
}

impl Default for XyChartDb {
    fn default() -> Self {
        Self {
            title: None,
            acc_title: None,
            acc_descr: None,
            orientation: None,
            x_axis: XAxis::Band {
                title: String::new(),
                categories: Vec::new(),
            },
            y_axis: LinearAxis {
                title: String::new(),
                min: f64::INFINITY,
                max: f64::NEG_INFINITY,
            },
            has_set_x_axis: false,
            has_set_y_axis: false,
            plots: Vec::new(),
        }
    }
}

impl XyChartDb {
    pub(crate) fn set_orientation(&mut self, orientation: &str) {
        self.orientation = Some(if orientation == "horizontal" {
            Orientation::Horizontal
        } else {
            Orientation::Vertical
        });
    }

    pub(crate) fn set_x_axis_title(&mut self, title: &XyText) {
        let title = title.text.trim().to_string();
        match &mut self.x_axis {
            XAxis::Band { title: t, .. } | XAxis::Linear { title: t, .. } => *t = title,
        }
    }

    pub(crate) fn set_x_axis_range_data(&mut self, min: f64, max: f64) {
        let title = self.x_axis.title().to_string();
        self.x_axis = XAxis::Linear { title, min, max };
        self.has_set_x_axis = true;
    }

    pub(crate) fn set_x_axis_band(&mut self, categories: Vec<XyText>) {
        let title = self.x_axis.title().to_string();
        let categories = categories
            .iter()
            .map(|c| c.text.trim().to_string())
            .collect();
        self.x_axis = XAxis::Band { title, categories };
        self.has_set_x_axis = true;
    }

    pub(crate) fn set_y_axis_title(&mut self, title: &XyText) {
        self.y_axis.title = title.text.trim().to_string();
    }

    pub(crate) fn set_y_axis_range_data(&mut self, min: f64, max: f64) {
        self.y_axis.min = min;
        self.y_axis.max = max;
        self.has_set_y_axis = true;
    }

    /// Puts the values of a series on the x axis.
    fn plot_data(&mut self, values: &[f64]) -> PlotData {
        if values.is_empty() {
            return PlotData::default();
        }
        // The first series fixes the x range when no x-axis statement did.
        if !self.has_set_x_axis {
            self.set_x_axis_range_data(1.0, values.len() as f64);
        }
        let mut values = values.to_vec();
        if let XAxis::Band { categories, .. } = &self.x_axis {
            if values.len() > categories.len() {
                values.truncate(categories.len());
            }
        }
        if !self.has_set_y_axis {
            let mut min = self.y_axis.min;
            let mut max = self.y_axis.max;
            for value in &values {
                if *value < min {
                    min = *value;
                }
                if *value > max {
                    max = *value;
                }
            }
            self.y_axis.min = min;
            self.y_axis.max = max;
        }
        match &self.x_axis {
            XAxis::Band { categories, .. } => PlotData {
                entries: categories
                    .iter()
                    .enumerate()
                    .map(|(i, category)| (category.clone(), values.get(i).copied()))
                    .collect(),
            },
            XAxis::Linear { min, max, .. } => {
                if values.len() == 1 {
                    return PlotData {
                        entries: vec![(crate::util::js_number(*min), Some(values[0]))],
                    };
                }
                let step = (max - min) / (values.len() as f64 - 1.0);
                PlotData {
                    entries: values
                        .iter()
                        .enumerate()
                        .map(|(i, value)| {
                            (crate::util::js_number(min + i as f64 * step), Some(*value))
                        })
                        .collect(),
                }
            }
        }
    }

    fn add_plot(&mut self, kind: PlotKind, title: &XyText, data: &[DataPoint]) {
        let values: Vec<f64> = data.iter().map(|d| d.value).collect();
        let plot_data = self.plot_data(&values);
        // Mermaid accepts labels on the points of a bar series and ignores them.
        let point_labels =
            if kind == PlotKind::Line && data.iter().any(|d| !d.label.trim().is_empty()) {
                Some(data.iter().map(|d| d.label.trim().to_string()).collect())
            } else {
                None
            };
        self.plots.push(Plot {
            kind,
            title: title.text.trim().to_string(),
            data: plot_data,
            point_labels,
        });
    }

    pub(crate) fn set_line_data(&mut self, title: &XyText, data: &[DataPoint]) {
        self.add_plot(PlotKind::Line, title, data);
    }

    pub(crate) fn set_bar_data(&mut self, title: &XyText, data: &[DataPoint]) {
        self.add_plot(PlotKind::Bar, title, data);
    }

    pub(crate) fn set_diagram_title(&mut self, title: &str) {
        self.title = Some(title.to_string());
    }

    pub(crate) fn set_acc_title(&mut self, text: &str) {
        self.acc_title = Some(text.trim_start().to_string());
    }

    pub(crate) fn set_acc_description(&mut self, text: &str) {
        self.acc_descr = Some(collapse_lines(text));
    }
}

/// Takes the space after a line break away, as Mermaid does with an accessible description.
fn collapse_lines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    loop {
        let Some(at) = rest.find('\n') else {
            out.push_str(rest);
            break;
        };
        let (before, after) = rest.split_at(at);
        out.push_str(before);
        out.push('\n');
        rest = after.trim_start();
    }
    out
}
