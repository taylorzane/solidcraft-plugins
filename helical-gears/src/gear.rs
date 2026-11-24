//! Transverse involute section and signed axial twist. The set of polar
//! equations follows the SolidCraft Gears spur generator (see THIRD-PARTY-LICENSES.txt).

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use crate::inputs::{GearType, InputError, Inputs, maximum_mm, minimum_mm};

pub type Point = [f64; 2];

pub enum Curve {
    Line(Point, Point),
    Arc {
        start: Point,
        through: Point,
        end: Point,
    },
    Spline(Vec<Point>),
}

impl Curve {
    pub fn rotated(&self, angle: f64) -> Self {
        let (sin, cos) = angle.sin_cos();
        let rotate = |[x, y]: Point| [cos * x - sin * y, sin * x + cos * y];
        match self {
            Self::Line(start, end) => Self::Line(rotate(*start), rotate(*end)),
            Self::Arc {
                start,
                through,
                end,
            } => Self::Arc {
                start: rotate(*start),
                through: rotate(*through),
                end: rotate(*end),
            },
            Self::Spline(points) => Self::Spline(points.iter().copied().map(rotate).collect()),
        }
    }
}

pub struct Gear {
    pub pitch_radius: f64,
    pub base_radius: f64,
    pub tip_radius: f64,
    pub root_radius: f64,
    pub relief_radius: f64,
    pub normal_tooth_thickness: f64,
    pub section_width: f64,
    pub twist: f64,
    teeth: u32,
    transverse_pressure_angle: f64,
    half_pitch_thickness_angle: f64,
    root_fillet_radius: f64,
}

impl Gear {
    pub fn new(inputs: &Inputs) -> Result<Self, InputError> {
        let module = inputs.normal_module;
        let cos_helix = inputs.helix_angle.cos();
        let pitch_radius = inputs.teeth as f64 * module / (2.0 * cos_helix);
        let transverse_pressure_angle = (inputs.normal_pressure_angle.tan() / cos_helix).atan();
        let base_radius = pitch_radius * transverse_pressure_angle.cos();
        let normal_tooth_thickness = automatic(
            inputs.normal_tooth_thickness,
            module * (FRAC_PI_2 + 2.0 * inputs.profile_shift * inputs.normal_pressure_angle.tan()),
        );
        let tip_radius = automatic(
            inputs.tip_diameter / 2.0,
            pitch_radius + module * (1.0 + inputs.profile_shift),
        );
        let root_radius = automatic(
            inputs.root_diameter / 2.0,
            pitch_radius - module * (1.25 - inputs.profile_shift),
        );
        let relief_radius = automatic(inputs.relief_diameter / 2.0, root_radius);
        let pitch_diameter = 2.0 * pitch_radius;
        if tip_radius < pitch_radius {
            let limit = minimum_mm(pitch_diameter);
            return Err(InputError::new(
                "tip_diameter",
                format!("Tip too small (>={limit}mm)"),
                format!("Tip diameter must reach the pitch diameter (use {limit} mm or more)"),
            ));
        }
        if root_radius <= 0.0 {
            return Err(InputError::new(
                "root_diameter",
                "Root too small (>0mm)",
                "Root diameter must be positive",
            ));
        }
        if root_radius >= pitch_radius {
            let (comparison, limit) =
                maximum_mm(pitch_diameter, |diameter| diameter < pitch_diameter);
            return Err(InputError::new(
                "root_diameter",
                format!("Root too large ({comparison}{limit}mm)"),
                format!(
                    "Root diameter must be smaller than the pitch diameter ({comparison}{limit} mm)"
                ),
            ));
        }
        if relief_radius > root_radius {
            let root_diameter = 2.0 * root_radius;
            let (comparison, limit) =
                maximum_mm(root_diameter, |diameter| diameter <= root_diameter);
            return Err(InputError::new(
                "relief_diameter",
                format!("Relief Ø too large ({comparison}{limit}mm)"),
                format!(
                    "Center relief diameter must be at most the root diameter ({comparison}{limit} mm)"
                ),
            ));
        }
        let relief_diameter = 2.0 * relief_radius;
        if inputs.bore_diameter >= relief_diameter {
            let (comparison, limit) =
                maximum_mm(relief_diameter, |diameter| diameter < relief_diameter);
            let diameter = if inputs.gear_type == GearType::DoubleHelical {
                "center relief diameter"
            } else {
                "root diameter"
            };
            return Err(InputError::new(
                "bore_diameter",
                format!("Bore too large ({comparison}{limit}mm)"),
                format!("Bore must be smaller than the {diameter} ({comparison}{limit} mm)"),
            ));
        }
        if normal_tooth_thickness <= 0.0 {
            return Err(InputError::new(
                "normal_tooth_thickness",
                "Tooth too thin (>0mm)",
                "Normal tooth thickness must be positive",
            ));
        }
        let circular_pitch = PI * module;
        let thickness_scale = 2.0 * cos_helix * pitch_radius;
        let half_pitch_angle = PI / inputs.teeth as f64;
        let pitch_involute = involute(transverse_pressure_angle);
        let root_involute = involute((base_radius / root_radius).min(1.0).acos());
        let minimum_space = 0.001 * module;
        let root_space_at = |thickness| {
            let half_angle = thickness / thickness_scale + pitch_involute - root_involute;
            (half_pitch_angle - half_angle) * root_radius
        };
        let thickness_error = || {
            let root_limit = (half_pitch_angle - minimum_space / root_radius - pitch_involute
                + root_involute)
                * thickness_scale;
            let max_thickness = root_limit.min(circular_pitch);
            let status = if max_thickness > 0.0 {
                let (comparison, limit) = maximum_mm(max_thickness, |thickness| {
                    thickness < circular_pitch && root_space_at(thickness) >= minimum_space
                });
                format!("Tooth too thick ({comparison}{limit}mm)")
            } else {
                "Teeth too thick at root".to_owned()
            };
            InputError::new(
                "normal_tooth_thickness",
                status,
                "Reduce tooth thickness or increase root diameter to leave space between the teeth",
            )
        };
        if normal_tooth_thickness >= circular_pitch {
            return Err(thickness_error());
        }
        let section_width_at = |face_width| match inputs.gear_type {
            GearType::Helical => face_width,
            _ => (face_width - inputs.relief_width) / 2.0,
        };
        let helix_tangent = inputs.helix_angle.tan();
        let twist_at = |section_width| inputs.hand * section_width * helix_tangent / pitch_radius;
        let section_width = section_width_at(inputs.face_width);
        let twist = twist_at(section_width);
        let twist_limit = 4.0 * TAU;
        if twist.abs() > twist_limit {
            let section_limit = twist_limit * pitch_radius / helix_tangent;
            let face_limit = match inputs.gear_type {
                GearType::Helical => section_limit,
                _ => 2.0 * section_limit + inputs.relief_width,
            };
            let (comparison, limit) = maximum_mm(face_limit, |width| {
                twist_at(section_width_at(width)).abs() <= twist_limit
            });
            return Err(InputError::new(
                "face_width",
                format!("Face too wide ({comparison}{limit}mm)"),
                format!(
                    "Reduce the face width ({comparison}{limit} mm) or helix angle to stay within four turns"
                ),
            ));
        }
        let gear = Self {
            pitch_radius,
            base_radius,
            tip_radius,
            root_radius,
            relief_radius,
            normal_tooth_thickness,
            section_width,
            twist,
            teeth: inputs.teeth,
            transverse_pressure_angle,
            half_pitch_thickness_angle: normal_tooth_thickness / thickness_scale,
            root_fillet_radius: inputs.root_fillet_radius,
        };
        if gear.half_angle(tip_radius) * tip_radius < minimum_space {
            return Err(InputError::new(
                "tip_diameter",
                "Tooth tip too thin",
                "Reduce tip diameter or increase tooth thickness to keep the flanks apart at the tip",
            ));
        }
        if root_space_at(normal_tooth_thickness) < minimum_space {
            return Err(thickness_error());
        }
        Ok(gear)
    }

    fn half_angle(&self, radius: f64) -> f64 {
        let flank_angle = (self.base_radius / radius).min(1.0).acos();
        self.half_pitch_thickness_angle + involute(self.transverse_pressure_angle)
            - involute(flank_angle)
    }

    /// The positive-angle flank point and its tangent when moving toward the tip.
    fn flank(&self, radius: f64) -> (Point, f64) {
        let angle = self.half_angle(radius);
        let roll_angle = (self.base_radius / radius).min(1.0).acos();
        (polar(radius, angle), angle - roll_angle)
    }

    /// Find an exact tangent circle between the reference root circle and flank.
    /// Below the base circle, the nominal flank continues along a radial segment.
    fn fillet(&self) -> Result<(f64, Point, Point), InputError> {
        let fillet = self.root_fillet_radius;
        let root = self.root_radius;
        if fillet == 0.0 {
            let point = self.flank(root).0;
            return Ok((root, point, point));
        }
        let center_at = |radius| {
            let ([x, y], tangent) = self.flank(radius);
            let [dx, dy] = polar(fillet, tangent + FRAC_PI_2);
            [x + dx, y + dy]
        };
        let distance = |[x, y]: Point| x.hypot(y);
        let error = |status| {
            InputError::new(
                "root_fillet_radius",
                status,
                "Reduce the root fillet radius so it fits below the tip and leaves space between the teeth",
            )
        };
        if distance(center_at(self.tip_radius)) <= root + fillet {
            return Err(error("Fillet too large for tip"));
        }
        let mut low = root;
        let mut high = self.tip_radius;
        for _ in 0..56 {
            let middle = (low + high) / 2.0;
            if distance(center_at(middle)) < root + fillet {
                low = middle;
            } else {
                high = middle;
            }
        }
        let radius = (low + high) / 2.0;
        let center = center_at(radius);
        let center_angle = center[1].atan2(center[0]);
        if !(0.0 < center_angle && center_angle < PI / self.teeth as f64)
            || self.tip_radius - radius < 1e-5
        {
            return Err(error("Fillet too large for tooth gap"));
        }
        Ok((radius, polar(root, center_angle), center))
    }

    pub fn tooth(&self) -> Result<Vec<Curve>, InputError> {
        let pitch = TAU / self.teeth as f64;
        let (fillet_radius_at_flank, root, center) = self.fillet()?;
        let start_radius = fillet_radius_at_flank.max(self.base_radius);
        let flank_start = self.flank(fillet_radius_at_flank).0;
        let start = self.flank(start_radius).0;
        let tip = self.flank(self.tip_radius).0;
        let roll = |radius: f64| ((radius / self.base_radius).powi(2) - 1.0).max(0.0).sqrt();
        let start_roll = roll(start_radius);
        let end_roll = roll(self.tip_radius);
        let flank: Vec<Point> = (0..=10)
            .map(|index| match index {
                0 => start,
                10 => tip,
                _ => {
                    let t = start_roll + (end_roll - start_roll) * (index as f64 / 10.0);
                    self.flank(self.base_radius * t.hypot(1.0)).0
                }
            })
            .collect();

        let mut curves = Vec::new();
        if self.root_fillet_radius > 0.0 {
            let middle = arc_middle(center, flank_start, root, self.root_fillet_radius);
            curves.push(Curve::Arc {
                start: reflect(root),
                through: reflect(middle),
                end: reflect(flank_start),
            });
        }
        if start_radius - fillet_radius_at_flank > 1e-7 {
            curves.push(Curve::Line(reflect(flank_start), reflect(start)));
        }
        curves.push(Curve::Spline(flank.iter().copied().map(reflect).collect()));
        curves.push(Curve::Arc {
            start: reflect(tip),
            through: [self.tip_radius, 0.0],
            end: tip,
        });
        curves.push(Curve::Spline(flank.into_iter().rev().collect()));
        if start_radius - fillet_radius_at_flank > 1e-7 {
            curves.push(Curve::Line(start, flank_start));
        }
        if self.root_fillet_radius > 0.0 {
            let middle = arc_middle(center, flank_start, root, self.root_fillet_radius);
            curves.push(Curve::Arc {
                start: flank_start,
                through: middle,
                end: root,
            });
        }
        let root_angle = root[1].atan2(root[0]);
        curves.push(Curve::Arc {
            start: root,
            through: polar(self.root_radius, pitch / 2.0),
            end: polar(self.root_radius, pitch - root_angle),
        });
        Ok(curves)
    }
}

fn automatic(value: f64, nominal: f64) -> f64 {
    if value == 0.0 { nominal } else { value }
}

fn involute(angle: f64) -> f64 {
    angle.tan() - angle
}

fn polar(radius: f64, angle: f64) -> Point {
    [radius * angle.cos(), radius * angle.sin()]
}

fn reflect([x, y]: Point) -> Point {
    [x, -y]
}

fn arc_middle([cx, cy]: Point, from: Point, to: Point, radius: f64) -> Point {
    let angle = |[x, y]: Point| (y - cy).atan2(x - cx);
    let from = angle(from);
    let delta = (angle(to) - from + PI).rem_euclid(TAU) - PI;
    let [x, y] = polar(radius, from + delta / 2.0);
    [x + cx, y + cy]
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::inputs::CreateAs;

    fn near(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
    }

    fn distance([x, y]: Point, [a, b]: Point) -> f64 {
        (x - a).hypot(y - b)
    }

    fn endpoints(curve: &Curve) -> (Point, Point) {
        match curve {
            Curve::Line(start, end) | Curve::Arc { start, end, .. } => (*start, *end),
            Curve::Spline(points) => (points[0], *points.last().unwrap()),
        }
    }

    #[test]
    fn defaults_use_normal_dimensions_and_a_signed_pitch_helix() {
        let inputs = Inputs::parse(&json!({ "gear_type": "helical" })).unwrap();
        let gear = Gear::new(&inputs).unwrap();
        assert_eq!(inputs.create_as, CreateAs::Component);
        assert_eq!(inputs.name, None);
        assert_eq!(inputs.gear_type.default_name(), "Helical Gear");
        near(gear.pitch_radius, 27.712812921102);
        near(
            gear.base_radius,
            gear.pitch_radius * gear.transverse_pressure_angle.cos(),
        );
        near(gear.tip_radius - gear.pitch_radius, 2.0);
        near(gear.pitch_radius - gear.root_radius, 2.5);
        near(gear.relief_radius, gear.root_radius);
        near(gear.normal_tooth_thickness, PI);
        near(gear.half_angle(gear.pitch_radius), PI / 48.0);
        near(gear.twist, 0.25);
        near(gear.section_width, 12.0);
    }

    #[test]
    fn double_helix_uses_the_measured_diameters_and_thickness() {
        let inputs = Inputs::parse(&json!({
            "gear_type": "double-helical",
            "hand": "left",
            "relief_diameter": 45.0,
            "tip_diameter": 59.0,
            "root_diameter": 48.0,
            "normal_tooth_thickness": 2.8,
            "tooth_phase": -30.0
        }))
        .unwrap();
        let gear = Gear::new(&inputs).unwrap();
        near(inputs.tooth_phase, 11.0 * PI / 6.0);
        near(gear.section_width, 5.0);
        near(gear.twist, -5.0 / 48.0);
        near(gear.tip_radius, 29.5);
        near(gear.root_radius, 24.0);
        near(gear.relief_radius, 22.5);
        near(gear.normal_tooth_thickness, 2.8);
        near(gear.half_angle(gear.pitch_radius), 2.8 / 48.0);
    }

    #[test]
    fn tooth_and_root_space_close_under_circular_pattern() {
        for fillet in [0.0, 0.3] {
            let inputs = Inputs::parse(&json!({
                "gear_type": "herringbone",
                "root_fillet_radius": fillet
            }))
            .unwrap();
            let gear = Gear::new(&inputs).unwrap();
            let curves = gear.tooth().unwrap();
            for pair in curves.windows(2) {
                near(distance(endpoints(&pair[0]).1, endpoints(&pair[1]).0), 0.0);
            }
            let arc = curves.last().unwrap().rotated(inputs.tooth_phase);
            let Curve::Arc { through, .. } = arc else {
                panic!("Expected circular root space")
            };
            near(through[0].hypot(through[1]), gear.root_radius);
            let next = curves[0].rotated(TAU / 24.0);
            near(distance(endpoints(&arc).1, endpoints(&next).0), 0.0);
        }
    }

    #[test]
    fn fillet_is_tangent_to_the_root_and_flank() {
        let inputs = Inputs::parse(&json!({
            "gear_type": "helical",
            "root_fillet_radius": 0.3
        }))
        .unwrap();
        let gear = Gear::new(&inputs).unwrap();
        let (radius, root, center) = gear.fillet().unwrap();
        let (flank, tangent) = gear.flank(radius);
        near(distance(root, center), 0.3);
        near(distance(flank, center), 0.3);
        near(
            (flank[0] - center[0]) * tangent.cos() + (flank[1] - center[1]) * tangent.sin(),
            0.0,
        );
    }

    #[test]
    fn center_relief_leaves_room_for_teeth_on_both_sides() {
        let params = |face_width, relief_width| {
            json!({
                "gear_type": "double-helical",
                "face_width": face_width,
                "relief_width": relief_width
            })
        };
        for (face_width, relief_width) in [(12.0, 11.9), (12.0, 11.98), (0.06, 0.04)] {
            assert!(Inputs::parse(&params(face_width, relief_width)).is_ok());
        }
        for relief_width in [11.99, 12.0, 13.0] {
            let error = Inputs::parse(&params(12.0, relief_width)).unwrap_err();
            assert_eq!(error.field, "relief_width");
            assert_eq!(error.status, "Relief too wide (<=11.9mm)");
            assert!(error.reason.contains("0.02 mm smaller"));
        }
        let error = Inputs::parse(&params(0.06, 0.05)).unwrap_err();
        assert_eq!(error.status, "Relief too wide (<=0.04mm)");
    }

    #[test]
    fn bore_status_uses_the_limiting_diameter() {
        let cases = [
            (
                json!({"gear_type": "helical", "bore_diameter": 100.0}),
                "<=50.4mm",
                50.4,
                "root diameter",
            ),
            (
                json!({"gear_type": "double-helical", "relief_diameter": 40.0, "bore_diameter": 40.0}),
                "<40mm",
                39.9,
                "center relief diameter",
            ),
            (
                json!({"gear_type": "double-helical", "root_diameter": 45.0, "bore_diameter": 46.0}),
                "<45mm",
                44.9,
                "center relief diameter",
            ),
        ];
        for (mut params, limit, corrected, surface) in cases {
            let inputs = Inputs::parse(&params).unwrap();
            let error = Gear::new(&inputs).err().unwrap();
            assert_eq!(error.field, "bore_diameter");
            assert_eq!(error.status, format!("Bore too large ({limit})"));
            assert!(error.reason.contains(surface));
            params["bore_diameter"] = json!(corrected);
            assert!(Gear::new(&Inputs::parse(&params).unwrap()).is_ok());
        }
    }

    #[test]
    fn tip_limit_rounds_up_to_an_accepted_diameter() {
        let mut params = json!({"gear_type": "helical", "tip_diameter": 55.425});
        let error = Gear::new(&Inputs::parse(&params).unwrap()).err().unwrap();
        assert_eq!(error.status, "Tip too small (>=55.5mm)");
        params["tip_diameter"] = json!(55.5);
        assert!(Gear::new(&Inputs::parse(&params).unwrap()).is_ok());
    }

    #[test]
    fn tooth_thickness_limit_leaves_space_at_the_root() {
        let mut params = json!({"gear_type": "helical", "normal_tooth_thickness": 7.0});
        let error = Gear::new(&Inputs::parse(&params).unwrap()).err().unwrap();
        let limit = error.status.strip_prefix("Tooth too thick (<=").unwrap();
        let limit = limit.strip_suffix("mm)").unwrap().parse::<f64>().unwrap();
        params["normal_tooth_thickness"] = json!(limit);
        assert!(Gear::new(&Inputs::parse(&params).unwrap()).is_ok());
        params["normal_tooth_thickness"] = json!(limit + 0.1);
        let next_step = Gear::new(&Inputs::parse(&params).unwrap()).err().unwrap();
        assert_eq!(error.status, next_step.status);
    }

    #[test]
    fn incompatible_inputs_report_the_field() {
        let unknown_type = Inputs::parse(&json!({ "gear_type": "spur" })).unwrap_err();
        assert_eq!(unknown_type.field, "gear_type");
        let unknown_destination =
            Inputs::parse(&json!({ "gear_type": "helical", "create_as": "sketch" })).unwrap_err();
        assert_eq!(unknown_destination.field, "create_as");
        let wrong_teeth =
            Inputs::parse(&json!({ "gear_type": "helical", "teeth": 24.5 })).unwrap_err();
        assert_eq!(wrong_teeth.field, "teeth");
        for (mut value, field) in [
            (json!({ "relief_diameter": 53.0 }), "relief_diameter"),
            (
                json!({ "bore_diameter": 44.0, "relief_diameter": 40.0 }),
                "bore_diameter",
            ),
            (
                json!({ "normal_tooth_thickness": 5.5 }),
                "normal_tooth_thickness",
            ),
            (json!({ "tip_diameter": 100.0 }), "tip_diameter"),
        ] {
            value["gear_type"] = json!("double-helical");
            let inputs = Inputs::parse(&value).unwrap();
            assert_eq!(Gear::new(&inputs).err().unwrap().field, field);
        }
    }
}
