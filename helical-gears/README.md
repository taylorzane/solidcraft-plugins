# Helical Gears for SolidCraft

Generate an external involute helical, double-helical, or herringbone gear as a separate solid. See the repository's [versioning note](../README.md#versioning) for SolidCraft compatibility.

## Install and preview

1. Open **Preferences → Plugins → Install plugin…** and select the `helical-gears-<version>.zip` file from [GitHub Releases](https://github.com/taylorzane/solidcraft-plugins/releases) or your local `dist/` directory.
2. Open **Model → Plugins → Helical Gears** and select **Helical**, **Double-Helical**, or **Herringbone** in the **Gear type** dropdown. Double-Helical also shows the center relief width.
3. Under **Create as**, choose **New component** to add a part at the design origin or **Body in current component** to add a separate solid at the active component's origin.
4. Edit the dimensions with **Preview** checked. The viewport updates when the calculation finishes. **Status** shows the applicable limit or geometry conflict when a setting is invalid. Hover the affected setting for details. Drag the dialog by its title if it covers the gear.
5. Use **Run** to create the gear. **Cancel** closes the preview, and a single **Undo** removes a committed gear.

The starting face is at local Z = 0. The gear is built from native sketches and solid features. Rerun the plugin to generate a gear with a different set of inputs.

Turn on **Show advanced settings** to edit the gear name, pressure angle, tooth profile, diameter overrides, root fillet, and starting tooth phase. For Double-Helical it also shows the center relief diameter. Collapsing the fields keeps their values. Leave the name blank to use the first unused number for that gear type, such as **Helical Gear 1** or **Helical Gear 2**.

## Dimensions and replacement fit

Lengths are millimeters and angles are degrees. Module, pressure angle, and pitch tooth thickness are specified in the **normal plane**. Helix angle is measured at the pitch cylinder. A right-hand first helix rotates counterclockwise as it moves toward +Z when viewed from +Z. The face width includes any center relief.

| Input | Default | How to use it |
| --- | ---: | --- |
| Tooth count | 24 | Enter the number of teeth on either end face. |
| Tooth size | 2 mm | Normal module sets tooth size and spacing. Match the mating gear's normal module. If the source gives transverse module, multiply it by the cosine of the helix angle. |
| Normal pressure angle | 20° | If the source gives transverse pressure angle, take the arctangent of its tangent times the cosine of the helix angle. |
| Helix angle and starting hand | 30°, right | Use the hand on the lower-Z side. Both chevron modes reverse hand on the far side. |
| Total face width and bore | 12 mm, 8 mm | Use a bore of 0 for a solid center. |
| Center relief | 2 mm wide, root diameter | Available for double-helical gears. A zero diameter selects the root diameter. |
| Profile shift and normal tooth thickness | 0, automatic | Positive shift increases nominal tip and root diameters and pitch tooth thickness. Set a positive thickness to match a measured value directly. |
| Tip and root diameter overrides | Automatic | A positive value sets that diameter directly. A value of 0 uses the standard full-depth proportions for the given profile shift. |
| Transverse root fillet | 0 mm | Enter a measured circular blend radius. Zero uses a sharp transition, with a radial segment below the base circle when needed. |
| Starting tooth phase | 0° | Angle from +X to a tooth center on the starting face. Adjust it to align the generated teeth with the reference. |

For a replacement, measure the bore, face width, outside diameter, and tooth count on the physical gear and check those against the CAD model. Align the two models on their axes and first end faces, then compare transverse sections near the end faces, at intermediate heights, and on either side of the center. Use the angular change of the same tooth between sections to check the helix. Compare the flank, tip, root, and tooth space to the fit tolerance of the original. If you only know the outside diameter, confirm the module and pressure angle against the flank geometry before using the result as a replacement.

The flank uses a spline fit to the involute. The optional root blend is circular. Cutter-generated undercut, trochoidal roots, and tooth relief require comparison against the reference if present.

## Build and test

Run these commands from the repository root.

```sh
mise install
mise run build:helical-gears
mise run test:helical-gears
```

`mise run package:helical-gears` also writes the installable ZIP to the repository's `dist/` directory. Tests use the SolidCraft CLI pinned in the [root Cargo manifest](../Cargo.toml). Set `SOLIDCRAFT_CLI` to the executable's full path if it is outside your `PATH`.

Scripts and agents call the `helical-gears.helical-gear` command and pass `gear_type` as `helical`, `double-helical`, or `herringbone`. The optional `create_as` parameter accepts `component` (the default) or `body`.

The ZIP includes the repository's [BSD-3-Clause license](../LICENSE), generated [third-party notices](THIRD-PARTY-LICENSES.txt), and the Rust toolchain's standard library notices. Run `mise run licenses:helical-gears` to refresh the checked-in copy.

The transverse involute implementation adapts equations from [SolidCraft Gears](https://github.com/bherbruck/solidcraft-gears/tree/ebc316cc47b4b87590505b0f9cb61794e8a70a7f). The [shared WIT contract](../wit/plugin.wit) comes from the pinned SolidCraft release.
