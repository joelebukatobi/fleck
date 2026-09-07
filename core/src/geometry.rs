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
}
