use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use uuid::Uuid;

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
    if !placement.x.is_finite()
        || !placement.y.is_finite()
        || !placement.w.is_finite()
        || !placement.h.is_finite()
    {
        return None;
    }

    let target = outputs
        .iter()
        .find(|o| o.id.matches(&placement.output))
        .or_else(|| outputs.iter().find(|o| o.primary))
        .or_else(|| outputs.first())?;

    if target.width == 0 || target.height == 0 {
        return None;
    }

    let (ow, oh) = (target.width as f32, target.height as f32);
    let w = (placement.w * ow).round().clamp(1.0, ow) as u32;
    let h = (placement.h * oh).round().clamp(1.0, oh) as u32;
    let x = (placement.x * ow).round().clamp(0.0, ow - w as f32) as i32;
    let y = (placement.y * oh).round().clamp(0.0, oh - h as f32) as i32;

    Some(Resolved { x, y, w, h })
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WindowState {
    #[serde(default)]
    pub placements: BTreeMap<Uuid, Placement>,
    #[serde(default)]
    pub minimized: BTreeSet<Uuid>,
    /// Last known (width, height) of each note's window, in logical pixels,
    /// so it can reopen at the size it was last resized to. Deliberately
    /// separate from `Placement`: position is never persisted, only size,
    /// and `#[serde(default)]` keeps an older state file (saved before this
    /// field existed) loading as an empty map instead of failing.
    #[serde(default)]
    pub sizes: BTreeMap<Uuid, (u32, u32)>,
}

impl WindowState {
    /// A missing or unreadable state file yields empty state. Window positions
    /// are worth losing; startup is not.
    pub fn load(path: &Path) -> std::io::Result<Self> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(_) => return Ok(Self::default()),
        };
        Ok(toml::from_str(&text).unwrap_or_default())
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string(self).expect("window state is serializable");
        let temp = path.with_extension("toml.tmp");
        std::fs::write(&temp, text)?;
        std::fs::rename(&temp, path)
    }
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

    #[test]
    fn round_trips_window_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("windows.toml");
        let id = uuid::Uuid::from_u128(3);

        let mut state = WindowState::default();
        state.placements.insert(id, placement(output("DP-1", Some("ABC123"))));
        state.minimized.insert(id);
        state.save(&path).unwrap();

        assert_eq!(WindowState::load(&path).unwrap(), state);
    }

    #[test]
    fn missing_state_file_loads_empty() {
        let dir = tempfile::tempdir().unwrap();
        let state = WindowState::load(&dir.path().join("nope.toml")).unwrap();
        assert!(state.placements.is_empty());
    }

    #[test]
    fn corrupt_state_file_loads_empty_rather_than_failing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("windows.toml");
        std::fs::write(&path, "this is not toml =").unwrap();
        assert!(WindowState::load(&path).unwrap().placements.is_empty());
    }

    #[test]
    fn zero_width_output_returns_none_instead_of_panicking() {
        let outputs = vec![info("eDP-1", Some("LAP001"), 0, 1080, true)];
        assert!(resolve(&placement(output("eDP-1", Some("LAP001"))), &outputs).is_none());
    }

    #[test]
    fn zero_height_output_returns_none_instead_of_panicking() {
        let outputs = vec![info("eDP-1", Some("LAP001"), 1920, 0, true)];
        assert!(resolve(&placement(output("eDP-1", Some("LAP001"))), &outputs).is_none());
    }

    #[test]
    fn nan_x_returns_none() {
        let outputs = vec![info("eDP-1", Some("LAP001"), 1920, 1080, true)];
        let mut p = placement(output("eDP-1", Some("LAP001")));
        p.x = f32::NAN;
        assert!(resolve(&p, &outputs).is_none());
    }

    #[test]
    fn infinite_w_returns_none() {
        let outputs = vec![info("eDP-1", Some("LAP001"), 1920, 1080, true)];
        let mut p = placement(output("eDP-1", Some("LAP001")));
        p.w = f32::INFINITY;
        assert!(resolve(&p, &outputs).is_none());
    }

    #[test]
    fn unreadable_state_file_loads_empty_rather_than_failing() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("windows.toml");
        std::fs::write(&path, "").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();

        // Check whether permissions are actually enforced in this environment
        // (they are not for root). If the raw read still succeeds, skip the
        // assertion visibly rather than fail on an environment quirk unrelated
        // to the fix under test.
        let permissions_enforced = std::fs::read_to_string(&path).is_err();

        let result = WindowState::load(&path);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        if !permissions_enforced {
            eprintln!("skipping unreadable_state_file_loads_empty_rather_than_failing: running as root, permissions not enforced");
            return;
        }

        assert!(result.unwrap().placements.is_empty());
    }

    #[test]
    fn round_trips_window_state_with_sizes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("windows.toml");
        let id = uuid::Uuid::from_u128(4);

        let mut state = WindowState::default();
        state.sizes.insert(id, (600, 900));
        state.save(&path).unwrap();

        assert_eq!(WindowState::load(&path).unwrap(), state);
    }

    #[test]
    fn a_state_file_without_the_sizes_field_loads_it_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("windows.toml");
        // Written by a version of `WindowState` that predates `sizes`.
        std::fs::write(&path, "minimized = []\n\n[placements]\n").unwrap();

        let state = WindowState::load(&path).unwrap();
        assert!(state.sizes.is_empty());
    }

    #[test]
    fn removing_a_size_entry_drops_it_from_the_saved_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("windows.toml");
        let id = uuid::Uuid::from_u128(5);

        let mut state = WindowState::default();
        state.sizes.insert(id, (512, 768));
        state.save(&path).unwrap();

        state.sizes.remove(&id);
        state.save(&path).unwrap();

        assert!(WindowState::load(&path).unwrap().sizes.is_empty());
    }

    #[test]
    fn three_outputs_primary_in_the_middle_and_target_absent_lands_on_primary() {
        let outputs = vec![
            info("DP-1", Some("AAA"), 1920, 1080, false),
            info("DP-2", Some("BBB"), 2000, 2000, true),
            info("DP-3", Some("CCC"), 1280, 720, false),
        ];
        let r = resolve(&placement(output("DP-9", Some("GONE"))), &outputs).unwrap();
        // placement is x:0.5, y:0.25 of the primary (DP-2, 2000x2000): (1000, 500)
        assert_eq!((r.x, r.y), (1000, 500));
    }
}
