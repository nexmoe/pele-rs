//! Fitting a diagram to the width the host has. Ported from pele's `diagrams/common/fit-width.ts`.

use crate::RenderOptions;
use crate::diagram::Output;

fn available(options: &RenderOptions) -> Option<f64> {
    options.max_width.filter(|&w| w > 0.0)
}

/// Draws a chart whose plot can be any width so that the whole diagram fits the width the host
/// has. What surrounds the plot (labels, a legend) is only known once it is drawn, so the plot
/// gives up the overflow and is drawn again. `narrow` tells the chart that it did not fit at
/// its own width, for a chart that then arranges itself differently.
#[cfg_attr(not(feature = "pie"), allow(dead_code))]
pub(crate) fn fit_width(
    options: &RenderOptions,
    natural: f64,
    least: f64,
    mut draw: impl FnMut(f64, bool) -> Output,
) -> Output {
    let result = draw(natural, false);
    let Some(available) = available(options).filter(|&a| result.width > a) else {
        return result;
    };
    let floor = least.min(natural);
    let mut plot = natural;
    let mut result = draw(plot, true);
    for _ in 0..3 {
        if result.width <= available || plot <= floor {
            break;
        }
        plot = floor.max(plot - (result.width - available));
        result = draw(plot, true);
    }
    result
}

/// For a chart that is drawn to a set width: its own, or the host's when that is smaller, but
/// not under `least`.
#[cfg_attr(not(any(feature = "gantt", feature = "xychart")), allow(dead_code))]
pub(crate) fn cap_width(options: &RenderOptions, natural: f64, least: f64) -> f64 {
    available(options).map_or(natural, |a| least.min(natural).max(natural.min(a)))
}

/// The width a title may take before it wraps.
#[cfg_attr(
    not(any(feature = "pie", feature = "flowchart", feature = "sequence")),
    allow(dead_code)
)]
pub(crate) fn title_room(options: &RenderOptions) -> f64 {
    available(options).map_or(4000.0, |a| {
        80f64.max(a - 2.0 * options.padding.unwrap_or(8.0))
    })
}

const BREAKPOINT: f64 = 640.0;

/// For a diagram that can run across or down: draws it across, and down instead when the host
/// allows it, the width it has is under the breakpoint, across does not fit, and down is
/// narrower.
#[cfg_attr(not(any(feature = "flowchart", feature = "git")), allow(dead_code))]
pub(crate) fn turn_to_fit(options: &RenderOptions, mut draw: impl FnMut(bool) -> Output) -> Output {
    let across = draw(false);
    let Some(available) = available(options) else {
        return across;
    };
    if !options.auto_direction || across.width <= available {
        return across;
    }
    let breakpoint = options
        .direction_breakpoint
        .filter(|&b| b > 0.0)
        .unwrap_or(BREAKPOINT);
    if available >= breakpoint {
        return across;
    }
    let down = draw(true);
    if down.width < across.width {
        down
    } else {
        across
    }
}

#[cfg_attr(not(feature = "flowchart"), allow(dead_code))]
pub(crate) fn runs_across(direction: Option<&str>) -> bool {
    matches!(direction, Some("LR" | "RL"))
}

/// Draws a diagram with room to spare, and tightly instead when that does not fit the width
/// the host has and tight is narrower.
#[cfg_attr(not(feature = "flowchart"), allow(dead_code))]
pub(crate) fn tighten(options: &RenderOptions, mut draw: impl FnMut(bool) -> Output) -> Output {
    let loose = draw(false);
    let Some(available) = available(options) else {
        return loose;
    };
    if loose.width <= available {
        return loose;
    }
    let tight = draw(true);
    if tight.width < loose.width {
        tight
    } else {
        loose
    }
}
