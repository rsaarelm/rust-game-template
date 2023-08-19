use serde::{Deserialize, Serialize};

#[derive(
    Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub enum Layout {
    #[default]
    Qwerty,
    Colemak,
    Dvorak,
}

impl Layout {
    /// Try to detect keyboard layout using platform-specific magic.
    ///
    /// Defaults to qwerty if it can't determine any other known layout being
    /// active.
    pub fn system_layout() -> Self {
        if is_active("colemak") {
            Layout::Colemak
        } else if is_active("dvorak") {
            Layout::Dvorak
        } else {
            Layout::Qwerty
        }
    }

    pub fn remap_from_qwerty(&self, c: char) -> char {
        if let Some(pos) = Layout::Qwerty
            .board()
            .iter()
            .position(|&b| b as u32 == c as u32)
        {
            return self.board()[pos] as char;
        }
        c
    }

    pub fn remap_to_qwerty(&self, c: char) -> char {
        // Not optimized, but this probably isn't a code hot spot.
        if let Some(pos) =
            self.board().iter().position(|&b| b as u32 == c as u32)
        {
            return Layout::Qwerty.board()[pos] as char;
        }
        c
    }

    fn board(&self) -> &'static [u8] {
        match self {
            Layout::Qwerty => {
                b"\
~!@#$%^&*()_+
`1234567890-=
QWERTYUIOP{}
qwertyuiop[]
ASDFGHJKL:\"|
asdfghjkl;'\\
ZXCVBNM<>?
zxcvbnm,./"
            }
            Layout::Colemak => {
                b"\
~!@#$%^&*()_+
`1234567890-=
QWFPGJLUY:{}
qwfpgjluy;[]
ARSTDHNEIO\"|
arstdhneio'\\
ZXCVBKM<>?
zxcvbkm,./"
            }
            Layout::Dvorak => {
                b"\
~!@#$%^&*(){}
`1234567890[]
\"<>PYFGCRL?+
',.pyfgcrl/=
AOEUIDHTNS_|
aoeuidhtns-\\
:QJKXBMWVZ
;qjkxbmwvz"
            }
        }
    }
}

/// Try to detect if user has a specific keyboard layout active.
#[cfg(target_os = "linux")]
fn is_active(layout_name: &str) -> bool {
    use std::process::Command;

    fn localectl(
        layout_name: &str,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let output = Command::new("localectl").arg("status").output()?;
        if !output.status.success() {
            Err("".into())
        } else {
            Ok(String::from_utf8(output.stdout)?
                .lines()
                .any(|line| line.contains(layout_name)))
        }
    }

    localectl(layout_name).unwrap_or(false)
}
#[cfg(not(target_os = "linux"))]
pub fn is_active(_layout_name: &str) -> bool {
    // XXX: Not implemented for Windows or OS X.
    false
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn kb_mapping() {
        let layout = Layout::Colemak;

        let qwertified: String =
            "arst".chars().map(|c| layout.remap_to_qwerty(c)).collect();
        assert_eq!(qwertified, "asdf".to_string());
    }
}
