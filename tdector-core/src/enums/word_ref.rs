/// Specifies what word a comment is attached to
#[derive(Debug, Clone)]
pub enum CommentTarget {
    /// Comment for a base/original vocabulary word
    BaseWord(String),
    /// Comment for a derived word (produced by applying formation rules)
    FormattedWord(String),
}
