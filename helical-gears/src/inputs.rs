use std::f64::consts::TAU;

use serde_json::Value;

#[derive(Debug, PartialEq)]
pub struct InputError {
    pub field: &'static str,
    pub status: String,
    pub reason: String,
}

impl InputError {
    pub fn new(field: &'static str, status: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            field,
            status: status.into(),
            reason: reason.into(),
        }
    }
}

pub(crate) fn minimum_mm(limit: f64) -> String {
    let (scale, decimals) = millimeter_grid(limit);
    let steps = (limit * scale).round();
    let mut rounded = steps / scale;
    if rounded < limit {
        rounded = (steps + 1.0) / scale;
    }
    format_limit(rounded, decimals)
}

pub(crate) fn maximum_mm(limit: f64, accepts: impl Fn(f64) -> bool) -> (&'static str, String) {
    let (scale, decimals) = millimeter_grid(limit);
    let steps = (limit * scale).round();
    let rounded = steps / scale;
    if rounded > 0.0 {
        if accepts(rounded) {
            return ("<=", format_limit(rounded, decimals));
        }
        // Keep a clean cutoff such as <40 when the next smaller value is valid.
        if accepts(rounded.next_down()) {
            return ("<", format_limit(rounded, decimals));
        }
    }

    let rounded_down = ((steps - 1.0) / scale).max(0.0);
    if rounded_down == 0.0 && limit > 0.0 {
        if accepts(limit) {
            return ("<=", format!("{limit:e}"));
        }
        if accepts(limit.next_down()) {
            return ("<", format!("{limit:e}"));
        }
    }
    ("<=", format_limit(rounded_down, decimals))
}

fn millimeter_grid(limit: f64) -> (f64, usize) {
    if limit >= 1.0 {
        (10.0, 1)
    } else if limit >= 0.1 {
        (100.0, 2)
    } else {
        (1000.0, 3)
    }
}

fn format_limit(value: f64, decimals: usize) -> String {
    format!("{value:.decimals$}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GearType {
    Helical,
    DoubleHelical,
    Herringbone,
}

impl GearType {
    fn parse(value: Option<&Value>) -> Result<Self, InputError> {
        match value.and_then(Value::as_str) {
            Some("helical") => Ok(Self::Helical),
            Some("double-helical") => Ok(Self::DoubleHelical),
            Some("herringbone") => Ok(Self::Herringbone),
            _ => Err(InputError::new(
                "gear_type",
                "Choose a gear type",
                "Choose Helical, Double-Helical, or Herringbone",
            )),
        }
    }

    pub fn default_name(self) -> &'static str {
        match self {
            Self::Helical => "Helical Gear",
            Self::DoubleHelical => "Double-Helical Gear",
            Self::Herringbone => "Herringbone Gear",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateAs {
    Component,
    Body,
}

#[derive(Debug)]
pub struct Inputs {
    pub gear_type: GearType,
    pub create_as: CreateAs,
    pub name: Option<String>,
    pub teeth: u32,
    pub normal_module: f64,
    pub normal_pressure_angle: f64,
    pub helix_angle: f64,
    pub hand: f64,
    pub face_width: f64,
    pub bore_diameter: f64,
    pub relief_width: f64,
    pub relief_diameter: f64,
    pub profile_shift: f64,
    pub normal_tooth_thickness: f64,
    pub tip_diameter: f64,
    pub root_diameter: f64,
    pub root_fillet_radius: f64,
    pub tooth_phase: f64,
}

impl Inputs {
    pub fn parse(value: &Value) -> Result<Self, InputError> {
        let object = value.as_object().ok_or_else(|| {
            InputError::new("params", "Enter gear settings", "Expected an object")
        })?;
        let gear_type = GearType::parse(object.get("gear_type"))?;
        let create_as = match object.get("create_as").map(Value::as_str) {
            None | Some(Some("component")) => CreateAs::Component,
            Some(Some("body")) => CreateAs::Body,
            _ => {
                return Err(InputError::new(
                    "create_as",
                    "Choose where to place the gear",
                    "Choose New component or Body in current component",
                ));
            }
        };
        let number = |field, label, default, min, max| {
            let Some(value) = object.get(field) else {
                return Ok(default);
            };
            let status = match value.as_f64() {
                Some(value) if value < min => format!("{label} too small (>={min})"),
                Some(value) if value > max => format!("{label} too large (<={max})"),
                Some(value) => return Ok(value),
                None => format!("{label} needs a number"),
            };
            Err(InputError::new(
                field,
                status,
                format!("{label} must be a number from {min} to {max}"),
            ))
        };
        let name = match object.get("component_name") {
            None => None,
            Some(value) => {
                let name = value
                    .as_str()
                    .map(str::trim)
                    .filter(|name| {
                        name.chars().count() <= 64 && !name.chars().any(char::is_control)
                    })
                    .ok_or_else(|| {
                        InputError::new(
                            "component_name",
                            "Use a short, printable name",
                            "Use up to 64 printable characters for the gear name",
                        )
                    })?;
                (!name.is_empty()).then(|| name.to_owned())
            }
        };
        let teeth = match object.get("teeth") {
            None => 24,
            Some(teeth) => teeth
                .as_u64()
                .filter(|count| (8..=80).contains(count))
                .ok_or_else(|| {
                    let status = match teeth.as_f64() {
                        Some(count) if count < 8.0 => "Too few teeth (>=8)",
                        Some(count) if count > 80.0 => "Too many teeth (<=80)",
                        _ => "Tooth count needs a whole number",
                    };
                    InputError::new(
                        "teeth",
                        status,
                        "Tooth count must be a whole number from 8 to 80",
                    )
                })? as u32,
        };
        let hand = match object.get("hand") {
            None => 1.0,
            Some(hand) => match hand.as_str() {
                Some("right") => 1.0,
                Some("left") => -1.0,
                _ => {
                    return Err(InputError::new(
                        "hand",
                        "Choose a helix hand",
                        "Choose Left or Right",
                    ));
                }
            },
        };
        let face_width = number("face_width", "Face width", 12.0, 0.05, 1000.0)?;
        let (relief_width, relief_diameter) = if gear_type == GearType::DoubleHelical {
            let relief_width = number("relief_width", "Relief width", 2.0, 0.01, 1000.0)?;
            // Allow rounding at the limit when the inputs come from decimal values.
            let relief_fits = |width| width + 0.02 - face_width <= f64::EPSILON * face_width;
            if !relief_fits(relief_width) {
                let (comparison, limit) = maximum_mm(face_width - 0.02, relief_fits);
                return Err(InputError::new(
                    "relief_width",
                    format!("Relief too wide ({comparison}{limit}mm)"),
                    "Center relief width must be at least 0.02 mm smaller than total face width",
                ));
            }
            (
                relief_width,
                number("relief_diameter", "Relief diameter", 0.0, 0.0, 16_000.0)?,
            )
        } else {
            (0.0, 0.0)
        };
        Ok(Self {
            gear_type,
            create_as,
            name,
            teeth,
            normal_module: number("normal_module", "Tooth size", 2.0, 0.1, 100.0)?,
            normal_pressure_angle: number(
                "normal_pressure_angle",
                "Pressure angle",
                20.0,
                10.0,
                35.0,
            )?
            .to_radians(),
            helix_angle: number("helix_angle", "Helix angle", 30.0, 0.1, 60.0)?.to_radians(),
            hand,
            face_width,
            bore_diameter: number("bore_diameter", "Bore diameter", 8.0, 0.0, 16_000.0)?,
            relief_width,
            relief_diameter,
            profile_shift: number("profile_shift", "Profile shift", 0.0, -1.0, 1.0)?,
            normal_tooth_thickness: number(
                "normal_tooth_thickness",
                "Tooth thickness",
                0.0,
                0.0,
                1000.0,
            )?,
            tip_diameter: number("tip_diameter", "Tip diameter", 0.0, 0.0, 17_000.0)?,
            root_diameter: number("root_diameter", "Root diameter", 0.0, 0.0, 16_000.0)?,
            root_fillet_radius: number("root_fillet_radius", "Root fillet", 0.0, 0.0, 100.0)?,
            tooth_phase: number("tooth_phase", "Tooth phase", 0.0, -360.0, 360.0)?
                .to_radians()
                .rem_euclid(TAU),
        })
    }
}
