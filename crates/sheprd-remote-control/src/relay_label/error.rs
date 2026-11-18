/// Why a relay's label cannot be used.
#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum Error {
    // keep-sorted start
    #[error("its {field} is blank")]
    Blank { field: &'static str },
    #[error("its {field} has a control character, such as a line break")]
    Control { field: &'static str },
    #[error("its {field} is longer than {max} characters")]
    Length { field: &'static str, max: usize },
    #[error("it has more than {} tags", super::TAGS_MAX)]
    Tags,
    // keep-sorted end
}
