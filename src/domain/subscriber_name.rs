use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug)]
pub struct SubscriberName(String);

impl AsRef<str> for SubscriberName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl SubscriberName {
    pub fn parse_name(s: String) -> Result<SubscriberName, String> {
        let is_empty_or_whitespace = s.trim().is_empty();
        let is_too_long = s.graphemes(true).count() > 256;
        let forbidden_characters = ['/', '(', ')', '"', '<', '>', '\\', '{', '}'];
        let contains_forbidden_characters =
            s.chars().any(|char| forbidden_characters.contains(&char));
        if is_empty_or_whitespace || is_too_long || contains_forbidden_characters {
            Err(format!("{} is not a valid subscriber name.", s))
        } else {
            Ok(Self(s))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::SubscriberName;
    use claim::{assert_err, assert_ok};

    #[test]
    fn valid_name_has_256_graphemes_or_fewer() {
        let name = "a".repeat(256);
        assert_ok!(SubscriberName::parse_name(name));
    }

    #[test]
    fn name_longer_than_256_graphemes_is_rejected() {
        let name = "a".repeat(257);
        assert_err!(SubscriberName::parse_name(name));
    }

    #[test]
    fn only_whitespace_is_rejected() {
        let name = " ".to_string();
        assert_err!(SubscriberName::parse_name(name));
    }

    #[test]
    fn empty_string_is_rejected() {
        let name = "".to_string();
        assert_err!(SubscriberName::parse_name(name));
    }

    #[test]
    fn names_containing_an_invalid_character_are_rejected() {
        for name in &['/', '(', ')', '"', '<', '>', '\\', '{', '}'] {
            let name = name.to_string();
            assert_err!(SubscriberName::parse_name(name));
        }
    }

    #[test]
    fn valid_names_are_parsed_successfully() {
        let name = "carlos".to_string();
        assert_ok!(SubscriberName::parse_name(name));
    }
}
