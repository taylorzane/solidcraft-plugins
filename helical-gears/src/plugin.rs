use std::collections::HashSet;

use serde_json::{Map, Value, json};

use crate::gear::{Curve, Gear};
use crate::inputs::{CreateAs, GearType, InputError, Inputs};

wit_bindgen::generate!({ path: "../wit", world: "command-dialog-plugin" });

use craft::plugin::types::{CommandFailure, FieldFailure, PluginError};
use craft::plugin::{commands, control};
use exports::craft::plugin::{dialog_provider, entry};

struct Plugin;

impl entry::Guest for Plugin {
    fn execute(entry: String, params: String) -> Result<String, PluginError> {
        if entry != "helical-gear" {
            return Err(unknown_entry());
        }
        let (inputs, gear, tooth) = checked_gear(&parse_params(&params)?).map_err(invalid)?;
        let result = generate(&inputs, &gear, tooth)?;
        Ok(result.to_string())
    }
}

impl dialog_provider::Guest for Plugin {
    fn update_fields(entry: String, values: String) -> Result<String, PluginError> {
        if entry != "helical-gear" {
            return Err(unknown_entry());
        }
        let values = parse_params(&values)?;
        let mut fields = Map::new();
        if values["advanced"] != true {
            // Hook visibility hides the controls while keeping their values in Run and Preview.
            fields.extend(
                [
                    "component_name",
                    "normal_pressure_angle",
                    "profile_shift",
                    "normal_tooth_thickness",
                    "tip_diameter",
                    "root_diameter",
                    "root_fillet_radius",
                    "tooth_phase",
                    "relief_diameter",
                ]
                .into_iter()
                .map(|field| (field.to_owned(), json!({ "visible": false }))),
            );
        }
        let status = match checked_gear(&values) {
            Ok(_) => "Valid".to_owned(),
            Err(error) => {
                if values.get(error.field).is_some() {
                    let field = fields
                        .entry(error.field.to_owned())
                        .or_insert_with(|| json!({}));
                    field["hint"] = json!(error.reason);
                }
                error.status
            }
        };
        fields.insert("validation_message".to_owned(), json!({ "value": status }));
        Ok(Value::Object(fields).to_string())
    }
}

export!(Plugin);

fn parse_params(params: &str) -> Result<Value, PluginError> {
    serde_json::from_str(params).map_err(|err| {
        PluginError::InvalidInput(FieldFailure {
            field: "params".into(),
            reason: err.to_string(),
        })
    })
}

fn checked_gear(params: &Value) -> Result<(Inputs, Gear, Vec<Curve>), InputError> {
    let inputs = Inputs::parse(params)?;
    let gear = Gear::new(&inputs)?;
    let tooth = gear.tooth()?;
    Ok((inputs, gear, tooth))
}

fn gear_name(inputs: &Inputs, listing: &Value) -> Result<String, PluginError> {
    if let Some(name) = &inputs.name
        && inputs.create_as == CreateAs::Component
    {
        return Ok(name.clone());
    }

    let components = listing["components"]
        .as_array()
        .ok_or_else(|| failure("component.list", "The component list is missing"))?;
    let names: HashSet<_> = components
        .iter()
        .flat_map(|component| {
            let bodies = component["bodies"].as_array().into_iter().flatten();
            component["name"]
                .as_str()
                .into_iter()
                .chain(bodies.filter_map(Value::as_str))
        })
        .collect();
    if let Some(name) = &inputs.name
        && !names.contains(name.as_str())
    {
        return Ok(name.clone());
    }
    let base_name = inputs
        .name
        .as_deref()
        .unwrap_or(inputs.gear_type.default_name());
    for index in 1..=names.len() + 1 {
        let candidate = format!("{base_name} {index}");
        if !names.contains(candidate.as_str()) {
            return Ok(candidate);
        }
    }
    unreachable!("There are more candidate names than existing names");
}

fn generate(inputs: &Inputs, gear: &Gear, tooth: Vec<Curve>) -> Result<Value, PluginError> {
    control::progress(0.05, "Drawing the gear profile");
    let listing = run("component.list", json!({ "compact": true, "bodies": true }))?;
    let name = gear_name(inputs, &listing)?;
    // A starting sketch ID distinguishes repeated gears in an existing component.
    let (component_id, start_sketch, suffix, body_name) = match inputs.create_as {
        CreateAs::Component => {
            let component = run("component.create", json!({ "name": name, "parent": 0 }))?;
            let id = id(&component, "component", "component.create")?;
            let sketch = xy_sketch(0.0, Some(&format!("Starting tooth profile · {id}")))?;
            (id, sketch, id.to_string(), format!("Gear body {id}"))
        }
        CreateAs::Body => {
            let active = listing["active"]
                .as_u64()
                .ok_or_else(|| failure("component.list", "The active component is missing"))?;
            let sketch = xy_sketch(0.0, None)?;
            (active, sketch, format!("sketch {sketch}"), name.clone())
        }
    };
    let feature_name = |name| format!("{name} · {suffix}");
    draw_gear_section(&tooth, inputs, inputs.tooth_phase)?;
    if inputs.create_as == CreateAs::Body {
        run(
            "timeline.rename",
            json!({ "feature": start_sketch, "name": feature_name("Starting tooth profile") }),
        )?;
    }
    control::progress(0.30, "Sweeping the teeth");
    let start_path = axial_path(
        0.0,
        gear.section_width,
        &feature_name("Starting helix axis"),
    )?;
    let first_sweep = run(
        "solid.sweep",
        json!({
            "sketch": start_sketch,
            "path_sketch": start_path,
            "path": ["axis"],
            "twist": gear.twist.to_degrees(),
            "operation": "new",
            "name": feature_name("Helical teeth"),
            "body_name": body_name,
        }),
    )?;
    let body = match first_sweep["created_bodies"].as_array().map(Vec::as_slice) {
        Some([body]) => body.as_str(),
        _ => None,
    }
    .ok_or_else(|| failure("solid.sweep", "Expected one solid from the tooth profile"))?;

    match inputs.gear_type {
        GearType::Helical => {}
        GearType::Herringbone if inputs.create_as == CreateAs::Component => {
            control::progress(0.65, "Joining the mirrored teeth");
            let args = json!({
                "bodies": [body],
                "plane": { "origin": [0.0, 0.0, gear.section_width], "normal": [0, 0, 1] },
                "combine": true,
            });
            let mirror = join("solid.mirror", body, args)?;
            let feature = id(&mirror, "feature", "solid.mirror")?;
            run(
                "timeline.rename",
                json!({ "feature": feature, "name": feature_name("Mirrored teeth") }),
            )?;
        }
        GearType::DoubleHelical | GearType::Herringbone => {
            if inputs.gear_type == GearType::DoubleHelical {
                control::progress(0.50, "Adding the center relief");
                let relief = xy_sketch(
                    gear.section_width,
                    Some(&feature_name("Center relief profile")),
                )?;
                circle(gear.relief_radius)?;
                if inputs.bore_diameter > 0.0 {
                    circle(inputs.bore_diameter / 2.0)?;
                }
                run("sketch.finish", json!({}))?;
                join(
                    "solid.extrude",
                    body,
                    json!({
                        "sketch": relief,
                        "distance": format!("{} mm", inputs.relief_width),
                        "operation": "join",
                        "targets": [body],
                        "name": feature_name("Center relief"),
                    }),
                )?;
            }

            // A second sweep keeps the chevron in the component's frame even when placed
            // away from the design origin, where an explicit mirror plane is read.
            control::progress(0.70, "Sweeping the opposite helix");
            let z = gear.section_width + inputs.relief_width;
            let end_sketch = xy_sketch(z, Some(&feature_name("Opposite tooth profile")))?;
            draw_gear_section(&tooth, inputs, inputs.tooth_phase + gear.twist)?;
            let end_path = axial_path(z, gear.section_width, &feature_name("Opposite helix axis"))?;
            join(
                "solid.sweep",
                body,
                json!({
                    "sketch": end_sketch,
                    "path_sketch": end_path,
                    "path": ["axis"],
                    "twist": -gear.twist.to_degrees(),
                    "operation": "join",
                    "targets": [body],
                    "name": feature_name("Opposite helical teeth"),
                }),
            )?;
        }
    }
    control::progress(1.0, "Gear complete");
    Ok(json!({
        "component": component_id,
        "name": if inputs.create_as == CreateAs::Body { body } else { &name },
        "body": body,
        "teeth": inputs.teeth,
        "pitch_diameter_mm": gear.pitch_radius * 2.0,
        "base_diameter_mm": gear.base_radius * 2.0,
        "tip_diameter_mm": gear.tip_radius * 2.0,
        "root_diameter_mm": gear.root_radius * 2.0,
        "normal_tooth_thickness_mm": gear.normal_tooth_thickness,
        "bore_diameter_mm": inputs.bore_diameter,
        "face_width_mm": inputs.face_width,
        "section_width_mm": gear.section_width,
        "section_twist_deg": gear.twist.to_degrees(),
        "relief_width_mm": inputs.relief_width,
        "relief_diameter_mm": if inputs.relief_width > 0.0 { gear.relief_radius * 2.0 } else { 0.0 },
    }))
}

fn draw_gear_section(tooth: &[Curve], inputs: &Inputs, phase: f64) -> Result<(), PluginError> {
    let mut curves = Vec::with_capacity(tooth.len());
    for curve in tooth {
        let (command, args) = match curve.rotated(phase) {
            Curve::Line(start, end) => (
                "sketch.line",
                json!({ "points": [start, end], "infer": false }),
            ),
            Curve::Arc {
                start,
                through,
                end,
            } => (
                "sketch.arc.three_point",
                json!({ "start": start, "through": through, "end": end }),
            ),
            Curve::Spline(points) => ("sketch.spline.fit_point", json!({ "points": points })),
        };
        let reply = run(command, args)?;
        let curve = reply["created"]["curves"][0]
            .as_str()
            .ok_or_else(|| failure(command, "The tooth curve id is missing"))?;
        curves.push(curve.to_owned());
    }
    run(
        "sketch.pattern.circular",
        json!({
            "entities": curves, "center": [0, 0], "count": inputs.teeth,
        }),
    )?;
    if inputs.bore_diameter > 0.0 {
        circle(inputs.bore_diameter / 2.0)?;
    }
    run("sketch.finish", json!({}))?;
    Ok(())
}

fn xy_sketch(z: f64, name: Option<&str>) -> Result<u64, PluginError> {
    // A named origin plane belongs to the active component. Explicit planes in
    // a nested plugin command are read in world coordinates on SolidCraft 0.46–0.47.
    let result = run(
        "sketch.create",
        json!({
            "plane": "XY", "offset": format!("{z} mm"), "name": name,
        }),
    )?;
    id(&result, "sketch", "sketch.create")
}

fn axial_path(z: f64, length: f64, name: &str) -> Result<u64, PluginError> {
    let result = run("sketch.create", json!({ "plane": "YZ", "name": name }))?;
    run(
        "sketch.line",
        json!({
            "points": [[0.0, z], [0.0, z + length]], "ids": ["axis"], "infer": false,
        }),
    )?;
    run("sketch.finish", json!({}))?;
    id(&result, "sketch", "sketch.create")
}

fn circle(radius: f64) -> Result<(), PluginError> {
    run(
        "sketch.circle.center",
        json!({ "center": [0, 0], "radius": radius }),
    )?;
    Ok(())
}

fn join(command: &str, target: &str, args: Value) -> Result<Value, PluginError> {
    let reply = run(command, args)?;
    if reply["created_bodies"] != json!([]) || reply["changed_bodies"] != json!([target]) {
        return Err(failure(
            command,
            "The gear sections must join into one solid",
        ));
    }
    Ok(reply)
}

fn run(command: &str, args: Value) -> Result<Value, PluginError> {
    if control::cancelled() {
        return Err(PluginError::Cancelled);
    }
    let result = commands::run(command, &args.to_string())?;
    serde_json::from_str(&result).map_err(|err| failure(command, &err.to_string()))
}

fn id(reply: &Value, key: &str, command: &str) -> Result<u64, PluginError> {
    reply[key]
        .as_u64()
        .ok_or_else(|| failure(command, "The created object id is missing"))
}

fn failure(command: &str, reason: &str) -> PluginError {
    PluginError::CommandFailed(CommandFailure {
        command: command.into(),
        reason: reason.into(),
    })
}

fn invalid(error: InputError) -> PluginError {
    PluginError::InvalidInput(FieldFailure {
        field: error.field.into(),
        reason: error.reason,
    })
}

fn unknown_entry() -> PluginError {
    PluginError::InvalidInput(FieldFailure {
        field: "entry".into(),
        reason: "Unknown gear command".into(),
    })
}
