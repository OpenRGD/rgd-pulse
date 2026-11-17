//! Fixed, flash-resident Chronoscope skin manifests for 128x128 pixels.
//! Theme identity is data, not a duplicate of the Somatic/Chronon renderers.
//! No dynamic executable code, remote downloads or motor commands.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SkinId {
    Classic = 0,
    Phosphor = 1,
    Chrome99 = 2,
    Italia = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChromeStyle {
    Clean,
    Oscilloscope,
    Beveled,
    Tricolore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SkinManifest {
    /// Future settings/host sync keys must be stable across display names.
    pub key: &'static str,
    pub label: &'static str,
    pub style: ChromeStyle,
}

pub const SKIN_COUNT: u8 = 4;
pub const SKIN_SCHEMA: &str = "openrgd.pulse-skin.v0.1-candidate";

impl SkinId {
    pub const fn from_index(index: u8) -> Self {
        match index {
            1 => Self::Phosphor,
            2 => Self::Chrome99,
            3 => Self::Italia,
            _ => Self::Classic,
        }
    }

    pub const fn index(self) -> u8 {
        self as u8
    }

    pub const fn manifest(self) -> SkinManifest {
        match self {
            Self::Classic => SkinManifest {
                key: "rgd-classic",
                label: "RGD CLASSIC",
                style: ChromeStyle::Clean,
            },
            Self::Phosphor => SkinManifest {
                key: "phosphor",
                label: "PHOSPHOR",
                style: ChromeStyle::Oscilloscope,
            },
            Self::Chrome99 => SkinManifest {
                key: "chrome-99",
                label: "CHROME 99",
                style: ChromeStyle::Beveled,
            },
            Self::Italia => SkinManifest {
                key: "italia",
                label: "ITALIA",
                style: ChromeStyle::Tricolore,
            },
        }
    }

    pub const fn next(self) -> Self {
        Self::from_index((self.index() + 1) % SKIN_COUNT)
    }

    pub const fn previous(self) -> Self {
        Self::from_index((self.index() + SKIN_COUNT - 1) % SKIN_COUNT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_stable_manifest_keys_and_styles() {
        let keys = [
            SkinId::Classic.manifest().key,
            SkinId::Phosphor.manifest().key,
            SkinId::Chrome99.manifest().key,
            SkinId::Italia.manifest().key,
        ];
        for i in 0..4 {
            for j in i + 1..4 {
                assert_ne!(keys[i], keys[j]);
            }
            assert!(!keys[i].is_empty());
            assert!(!SkinId::from_index(i as u8).manifest().label.is_empty());
        }
        assert_eq!(SkinId::Italia.manifest().style, ChromeStyle::Tricolore);
    }

    #[test]
    fn skin_cycle_wraps_in_both_directions() {
        assert_eq!(SkinId::Classic.previous(), SkinId::Italia);
        assert_eq!(SkinId::Italia.next(), SkinId::Classic);
        let mut id = SkinId::Classic;
        for _ in 0..4 {
            id = id.next();
        }
        assert_eq!(id, SkinId::Classic);
        assert_eq!(SkinId::from_index(255), SkinId::Classic);
    }
}
