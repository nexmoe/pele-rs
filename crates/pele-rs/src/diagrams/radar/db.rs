//! The model of a radar chart: its axes, its curves and its options.
//! Ported from Mermaid's `diagrams/radar/db.ts`, which pele's `diagrams/radar/model.ts` follows.

use super::parser::{OptionName, OptionValue, RadarAst};
use crate::error::{Error, ErrorCode};

/// The largest number of rings a radar chart may draw, as Mermaid allows.
const MAX_TICKS: f64 = 32.0;

/// The shape of the grid of a radar chart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Graticule {
    Circle,
    Polygon,
}

/// An axis of a radar chart.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RadarAxis {
    pub name: String,
    pub label: String,
}

/// A curve of a radar chart: a name and one value for every axis.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RadarCurve {
    pub name: String,
    pub label: String,
    pub entries: Vec<f64>,
}

/// The options of a radar chart.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RadarOptions {
    pub show_legend: bool,
    pub ticks: f64,
    pub max: Option<f64>,
    pub min: f64,
    pub graticule: Graticule,
}

impl Default for RadarOptions {
    fn default() -> Self {
        Self {
            show_legend: true,
            ticks: 5.0,
            max: None,
            min: 0.0,
            graticule: Graticule::Circle,
        }
    }
}

/// The model of a radar chart.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct RadarModel {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub axes: Vec<RadarAxis>,
    pub curves: Vec<RadarCurve>,
    pub options: RadarOptions,
}

/// The values of a curve, in the order of the axes. A curve either names the axis of every one of
/// its values, or gives them in the order of the axes.
fn curve_entries(
    entries: &[super::parser::RadarEntry],
    axes: &[RadarAxis],
) -> Result<Vec<f64>, Error> {
    let Some(first) = entries.first() else {
        return Ok(Vec::new());
    };
    if first.axis.is_none() {
        return Ok(entries.iter().map(|entry| entry.value).collect());
    }
    if axes.is_empty() {
        return Err(Error::new(
            ErrorCode::Semantic,
            "Axes must be populated before curves for reference entries",
        ));
    }
    let mut values = Vec::with_capacity(axes.len());
    for axis in axes {
        let entry = entries
            .iter()
            .find(|entry| entry.axis.as_deref() == Some(axis.name.as_str()));
        match entry {
            Some(entry) => values.push(entry.value),
            None => {
                return Err(Error::new(
                    ErrorCode::Semantic,
                    format!("Missing entry for axis {}", axis.label),
                ));
            }
        }
    }
    Ok(values)
}

/// Builds the model of a radar chart out of what the parser read.
pub(crate) fn build_radar(ast: &RadarAst, title: Option<&str>) -> Result<RadarModel, Error> {
    let axes: Vec<RadarAxis> = ast
        .axes
        .iter()
        .map(|axis| RadarAxis {
            name: axis.name.clone(),
            label: axis.label.clone().unwrap_or_else(|| axis.name.clone()),
        })
        .collect();

    let mut curves = Vec::with_capacity(ast.curves.len());
    for curve in &ast.curves {
        curves.push(RadarCurve {
            name: curve.name.clone(),
            label: curve.label.clone().unwrap_or_else(|| curve.name.clone()),
            entries: curve_entries(&curve.entries, &axes)?,
        });
    }

    // The last of the options of a name is the one that counts.
    let mut options = RadarOptions::default();
    for option in &ast.options {
        match (option.name, &option.value) {
            (OptionName::ShowLegend, OptionValue::Bool(value)) => options.show_legend = *value,
            (OptionName::Graticule, OptionValue::Graticule(value)) => {
                options.graticule = if value == "polygon" {
                    Graticule::Polygon
                } else {
                    Graticule::Circle
                };
            }
            (OptionName::Ticks, OptionValue::Number(value)) => options.ticks = *value,
            (OptionName::Max, OptionValue::Number(value)) => options.max = Some(*value),
            (OptionName::Min, OptionValue::Number(value)) => options.min = *value,
            _ => {}
        }
    }
    if options.ticks > MAX_TICKS {
        options.ticks = MAX_TICKS;
    }

    Ok(RadarModel {
        title: ast.title.clone().or_else(|| title.map(str::to_string)),
        acc_title: ast.acc_title.clone(),
        acc_descr: ast.acc_descr.clone(),
        axes,
        curves,
        options,
    })
}
