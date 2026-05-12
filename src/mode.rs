use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Mode {
    Normal,
    Insert,
    Command,
}

impl Mode {
    pub(crate) const ALL: &'static [Self] = &[Self::Normal, Self::Insert, Self::Command];

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Insert => "insert",
            Self::Command => "command",
        }
    }

    pub(crate) fn from_str(s: &str) -> Option<Self> {
        match s {
            "normal" => Some(Self::Normal),
            "insert" => Some(Self::Insert),
            "command" => Some(Self::Command),
            _ => None,
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
