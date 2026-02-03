/// Sort direction for segment list ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// Sort field for segment list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortField {
    Index,
    Original,
    Length,
    Count,
    TranslatedRatio,
    TranslatedCount,
}

/// Available sorting modes for the segment list.
///
/// Segments can be sorted by index (original order), original text content,
/// token count, or translation completion ratio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortMode {
    pub field: SortField,
    pub direction: SortDirection,
}

impl SortMode {
    /// Returns all available sort modes in their display order.
    ///
    /// This method provides the canonical ordering for UI selection widgets.
    #[must_use]
    pub const fn all() -> [Self; 12] {
        [
            Self {
                field: SortField::Index,
                direction: SortDirection::Ascending,
            },
            Self {
                field: SortField::Index,
                direction: SortDirection::Descending,
            },
            Self {
                field: SortField::Original,
                direction: SortDirection::Ascending,
            },
            Self {
                field: SortField::Original,
                direction: SortDirection::Descending,
            },
            Self {
                field: SortField::Length,
                direction: SortDirection::Ascending,
            },
            Self {
                field: SortField::Length,
                direction: SortDirection::Descending,
            },
            Self {
                field: SortField::Count,
                direction: SortDirection::Ascending,
            },
            Self {
                field: SortField::Count,
                direction: SortDirection::Descending,
            },
            Self {
                field: SortField::TranslatedRatio,
                direction: SortDirection::Ascending,
            },
            Self {
                field: SortField::TranslatedRatio,
                direction: SortDirection::Descending,
            },
            Self {
                field: SortField::TranslatedCount,
                direction: SortDirection::Ascending,
            },
            Self {
                field: SortField::TranslatedCount,
                direction: SortDirection::Descending,
            },
        ]
    }

    /// Returns the display text for this sort mode.
    #[must_use]
    pub fn display_text(self) -> &'static str {
        match (self.field, self.direction) {
            (SortField::Index, SortDirection::Ascending) => "Index (Asc)",
            (SortField::Index, SortDirection::Descending) => "Index (Desc)",
            (SortField::Original, SortDirection::Ascending) => "Original (Asc)",
            (SortField::Original, SortDirection::Descending) => "Original (Desc)",
            (SortField::Length, SortDirection::Ascending) => "Length (Shortest First)",
            (SortField::Length, SortDirection::Descending) => "Length (Longest First)",
            (SortField::Count, SortDirection::Ascending) => "Token Count (Asc)",
            (SortField::Count, SortDirection::Descending) => "Token Count (Desc)",
            (SortField::TranslatedRatio, SortDirection::Ascending) => "Translated Ratio (Asc)",
            (SortField::TranslatedRatio, SortDirection::Descending) => "Translated Ratio (Desc)",
            (SortField::TranslatedCount, SortDirection::Ascending) => {
                "Translated Token Count (Asc)"
            }
            (SortField::TranslatedCount, SortDirection::Descending) => {
                "Translated Token Count (Desc)"
            }
        }
    }

    pub const DEFAULT: Self = Self {
        field: SortField::Index,
        direction: SortDirection::Ascending,
    };
}
