#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Note {
    pub author: String,
    pub text: String,
    pub visible: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Reply {
    pub author: String,
    pub text: String,
    pub time: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Comment {
    pub thread: Vec<Reply>,
    pub resolved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Link {
    pub target: String,
    pub tooltip: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    Web,
    Mail,
    File,
    Place,
}

impl Link {
    pub fn kind(&self) -> LinkKind {
        let target = self.target.trim();
        let lower = target.to_ascii_lowercase();
        if target.starts_with('#') {
            LinkKind::Place
        } else if lower.starts_with("mailto:") {
            LinkKind::Mail
        } else if lower.starts_with("http://")
            || lower.starts_with("https://")
            || lower.starts_with("www.")
        {
            LinkKind::Web
        } else {
            LinkKind::File
        }
    }

    pub fn place(&self) -> Option<&str> {
        self.target.trim().strip_prefix('#')
    }
}

pub fn normalize_target(input: &str) -> String {
    let input = input.trim();
    let lower = input.to_ascii_lowercase();
    if lower.starts_with("www.") {
        format!("https://{input}")
    } else if !lower.contains(':') && input.contains('@') && !input.contains(['/', '\\']) {
        format!("mailto:{input}")
    } else {
        input.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_link_targets() {
        let link = |target: &str| Link {
            target: normalize_target(target),
            tooltip: String::new(),
        };
        assert_eq!(link("www.ornek.com.tr").target, "https://www.ornek.com.tr");
        assert_eq!(link("www.ornek.com.tr").kind(), LinkKind::Web);
        assert_eq!(link("ali@firma.com").target, "mailto:ali@firma.com");
        assert_eq!(link("ali@firma.com").kind(), LinkKind::Mail);
        assert_eq!(link("C:\\Raporlar\\ocak.xlsx").kind(), LinkKind::File);
        assert_eq!(link("#'Satış Verisi'!B4").kind(), LinkKind::Place);
        assert_eq!(
            link("#'Satış Verisi'!B4").place(),
            Some("'Satış Verisi'!B4")
        );
    }
}
