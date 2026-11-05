//! Whether the first word of a box title is a CSS color. Mermaid asks the browser; this knows
//! the color keywords and the `rgb()` and `hsl()` notations. Ported from pele's `colors.ts`.

const KEYWORDS: &str = concat!(
    "aliceblue antiquewhite aqua aquamarine azure beige bisque black blanchedalmond blue ",
    "blueviolet brown burlywood cadetblue chartreuse chocolate coral cornflowerblue cornsilk ",
    "crimson cyan darkblue darkcyan darkgoldenrod darkgray darkgreen darkgrey darkkhaki ",
    "darkmagenta darkolivegreen darkorange darkorchid darkred darksalmon darkseagreen ",
    "darkslateblue darkslategray darkslategrey darkturquoise darkviolet deeppink deepskyblue ",
    "dimgray dimgrey dodgerblue firebrick floralwhite forestgreen fuchsia gainsboro ",
    "ghostwhite gold goldenrod gray green greenyellow grey honeydew hotpink indianred indigo ",
    "ivory khaki lavender lavenderblush lawngreen lemonchiffon lightblue lightcoral lightcyan ",
    "lightgoldenrodyellow lightgray lightgreen lightgrey lightpink lightsalmon lightseagreen ",
    "lightskyblue lightslategray lightslategrey lightsteelblue lightyellow lime limegreen ",
    "linen magenta maroon mediumaquamarine mediumblue mediumorchid mediumpurple ",
    "mediumseagreen mediumslateblue mediumspringgreen mediumturquoise mediumvioletred ",
    "midnightblue mintcream mistyrose moccasin navajowhite navy oldlace olive olivedrab ",
    "orange orangered orchid palegoldenrod palegreen paleturquoise palevioletred papayawhip ",
    "peachpuff peru pink plum powderblue purple rebeccapurple red rosybrown royalblue ",
    "saddlebrown salmon sandybrown seagreen seashell sienna silver skyblue slateblue ",
    "slategray slategrey snow springgreen steelblue tan teal thistle tomato turquoise violet ",
    "wheat white whitesmoke yellow yellowgreen transparent currentcolor inherit initial unset ",
    "revert accentcolor accentcolortext activetext buttonborder buttonface buttontext canvas ",
    "canvastext field fieldtext graytext highlight highlighttext linktext mark marktext ",
    "selecteditem selecteditemtext visitedtext activeborder activecaption appworkspace ",
    "background buttonhighlight buttonshadow captiontext inactiveborder inactivecaption ",
    "inactivecaptiontext infobackground infotext menu menutext scrollbar threeddarkshadow ",
    "threedface threedhighlight threedlightshadow threedshadow window windowframe windowtext",
);

#[derive(Clone, Copy, PartialEq, Eq)]
enum K {
    Bad,
    Number,
    Percent,
    Angle,
    None,
}

/// `^[+-]?(?:\d+(?:\.\d+)?|\.\d+)(?:e[+-]?\d+)?(%|deg|grad|rad|turn)?$`, ignoring case.
fn kind(term: &str) -> K {
    let lower = term.to_ascii_lowercase();
    if lower == "none" {
        return K::None;
    }
    let mut s = lower.as_str();
    let mut unit = K::Number;
    for (suffix, k) in [
        ("%", K::Percent),
        ("deg", K::Angle),
        ("grad", K::Angle),
        ("rad", K::Angle),
        ("turn", K::Angle),
    ] {
        if let Some(rest) = s.strip_suffix(suffix) {
            s = rest;
            unit = k;
            break;
        }
    }
    let s = s.strip_prefix(['+', '-']).unwrap_or(s);
    let (mantissa, exp) = match s.split_once('e') {
        Some((m, e)) => (m, Some(e.strip_prefix(['+', '-']).unwrap_or(e))),
        None => (s, None),
    };
    let digits = |x: &str| !x.is_empty() && x.bytes().all(|b| b.is_ascii_digit());
    let ok = match mantissa.split_once('.') {
        Some((i, f)) => (i.is_empty() || digits(i)) && digits(f),
        None => digits(mantissa),
    } && exp.is_none_or(digits);
    if ok { unit } else { K::Bad }
}

fn is_alpha(term: &str, modern: bool) -> bool {
    let k = kind(term);
    k == K::Number || k == K::Percent || (modern && k == K::None)
}

fn is_function(color: &str) -> bool {
    let hsl = color.starts_with("hsl");
    let Some(open) = color.find('(') else {
        return false;
    };
    let name = &color[..open];
    if !matches!(name, "rgb" | "rgba" | "hsl" | "hsla") {
        return false;
    }
    if color.encode_utf16().count() > 120 || color.find(')') != Some(color.len() - 1) {
        return false;
    }
    let inner = crate::util::trim(&color[open + 1..color.len() - 1]);
    let legacy = inner.contains(',');
    let mut alpha = None;
    let parts: Vec<&str> = if legacy {
        let mut parts: Vec<&str> = inner.split(',').map(crate::util::trim).collect();
        if parts.len() == 4 {
            alpha = parts.pop();
        }
        parts
    } else {
        let slash: Vec<&str> = inner.split('/').collect();
        if slash.len() > 2 {
            return false;
        }
        if slash.len() == 2 {
            alpha = Some(crate::util::trim(slash[1]));
        }
        crate::util::trim(slash[0])
            .split(crate::util::is_space)
            .filter(|p| !p.is_empty())
            .collect()
    };
    if parts.len() != 3 || alpha.is_some_and(|a| !is_alpha(a, !legacy)) {
        return false;
    }
    let kinds: Vec<K> = parts.iter().map(|p| kind(p)).collect();
    if legacy {
        if hsl {
            return matches!(kinds[0], K::Number | K::Angle)
                && kinds[1] == K::Percent
                && kinds[2] == K::Percent;
        }
        return matches!(kinds[0], K::Number | K::Percent)
            && kinds[1] == kinds[0]
            && kinds[2] == kinds[0];
    }
    kinds.iter().enumerate().all(|(i, &k)| {
        k == K::Number
            || k == K::None
            || if hsl && i == 0 {
                k == K::Angle
            } else {
                k == K::Percent
            }
    })
}

pub(crate) fn is_css_color(color: &str) -> bool {
    if color.contains('(') {
        return is_function(color);
    }
    let lower = color.to_lowercase();
    KEYWORDS.split(' ').any(|k| k == lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_colors() {
        assert!(is_css_color("Aqua"));
        assert!(is_css_color("transparent"));
        assert!(is_css_color("rgb(10, 20, 30)"));
        assert!(is_css_color("rgba(10,20,30,0.5)"));
        assert!(is_css_color("hsl(120deg 50% 50% / 0.2)"));
        assert!(!is_css_color("rgb(10, 20%, 30)"));
        assert!(!is_css_color("Bob"));
        assert!(!is_css_color("url(x)"));
    }
}
