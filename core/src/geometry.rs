use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputId {
    pub name: String,
    pub make: String,
    pub model: String,
    pub serial: Option<String>,
}

impl OutputId {
    /// Serial first; fall back to make, model and name when either side
    /// reports no serial.
    pub fn matches(&self, other: &OutputId) -> bool {
        match (&self.serial, &other.serial) {
            (Some(a), Some(b)) => a == b,
            _ => self.make == other.make && self.model == other.model && self.name == other.name,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub output: OutputId,
    /// Fractions of the output's width and height, so a resolution change
    /// needs no migration.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputInfo {
    pub id: OutputId,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolved {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

/// Place a note on the display it names, or on the primary display if that one
/// is absent. Never mutates `placement`: a displaced note keeps naming its home
/// display and returns there when it is reconnected.
pub fn resolve(placement: &Placement, outputs: &[OutputInfo]) -> Option<Resolved> {
    let target = outputs
        .iter()
        .find(|o| o.id.matches(&placement.output))
        .or_else(|| outputs.iter().find(|o| o.primary))
        .or_else(|| outputs.first())?;

    let (ow, oh) = (target.width as f32, target.height as f32);
    let w = (placement.w * ow).round().clamp(1.0, ow) as u32;
    let h = (placement.h * oh).round().clamp(1.0, oh) as u32;
    let x = (placement.x * ow).round().clamp(0.0, ow - w as f32) as i32;
    let y = (placement.y * oh).round().clamp(0.0, oh - h as f32) as i32;

    Some(Resolved { x, y, w, h })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(name: &str, serial: Option<&str>) -> OutputId {
        OutputId {
            name: name.into(),
            make: "Dell".into(),
            model: "U2720Q".into(),
            serial: serial.map(str::to_string),
        }
    }

    #[test]
    fn matches_on_serial_even_when_the_name_changed() {
        assert!(output("DP-1", Some("ABC123")).matches(&output("DP-3", Some("ABC123"))));
    }

    #[test]
    fn different_serials_never_match() {
        assert!(!output("DP-1", Some("ABC123")).matches(&output("DP-1", Some("XYZ789"))));
    }

    #[test]
    fn falls_back_to_make_model_name_without_a_serial() {
        assert!(output("DP-1", None).matches(&output("DP-1", None)));
        assert!(!output("DP-1", None).matches(&output("DP-2", None)));
    }

    #[test]
    fn a_serial_on_one_side_only_falls_back_to_name() {
        assert!(output("DP-1", Some("ABC123")).matches(&output("DP-1", None)));
    }

    fn info(name: &str, serial: Option<&str>, w: u32, h: u32, primary: bool) -> OutputInfo {
        OutputInfo { id: output(name, serial), width: w, height: h, primary }
    }

    fn placement(on: OutputId) -> Placement {
        Placement { output: on, x: 0.5, y: 0.25, w: 0.2, h: 0.1 }
    }

    #[test]
    fn places_on_its_own_display() {
        let outputs = vec![
            info("DP-1", Some("ABC123"), 3840, 2160, false),
            info("eDP-1", Some("LAP001"), 1920, 1080, true),
        ];
        let r = resolve(&placement(output("DP-1", Some("ABC123"))), &outputs).unwrap();
        assert_eq!((r.x, r.y, r.w, r.h), (1920, 540, 768, 216));
    }

    #[test]
    fn falls_back_to_primary_when_its_display_is_gone() {
        let outputs = vec![info("eDP-1", Some("LAP001"), 1920, 1080, true)];
        let r = resolve(&placement(output("DP-1", Some("ABC123"))), &outputs).unwrap();
        assert_eq!((r.x, r.y), (960, 270));
    }

    #[test]
    fn fallback_does_not_alter_the_stored_placement() {
        let outputs = vec![info("eDP-1", Some("LAP001"), 1920, 1080, true)];
        let p = placement(output("DP-1", Some("ABC123")));
        resolve(&p, &outputs).unwrap();
        assert_eq!(p.output, output("DP-1", Some("ABC123")));
    }

    #[test]
    fn clamps_a_note_that_would_fall_off_a_smaller_display() {
        let outputs = vec![info("eDP-1", Some("LAP001"), 1000, 1000, true)];
        let p = Placement { output: output("eDP-1", Some("LAP001")), x: 0.95, y: 0.95, w: 0.2, h: 0.2 };
        let r = resolve(&p, &outputs).unwrap();
        assert_eq!((r.x + r.w as i32, r.y + r.h as i32), (1000, 1000));
    }

    #[test]
    fn no_outputs_means_no_placement() {
        assert!(resolve(&placement(output("DP-1", Some("ABC123"))), &[]).is_none());
    }

    #[test]
    fn without_a_primary_the_first_output_is_used() {
        let outputs = vec![info("eDP-1", Some("LAP001"), 1920, 1080, false)];
        assert!(resolve(&placement(output("DP-9", Some("GONE"))), &outputs).is_some());
    }
}
