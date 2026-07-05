/// Direction for sorting operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// Field to sort segments by
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortField {
    /// Sort by original segment index (no reordering)
    Index,
    /// Sort by the original text of tokens (alphabetical)
    Original,
    /// Sort by number of tokens in each segment
    Length,
    /// Sort by token count
    Count,
    /// Sort by translation completion ratio (0.0 to 1.0)
    TranslatedRatio,
    /// Sort by number of translated tokens
    TranslatedCount,
}

/// Complete sort specification combining field and direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortMode {
    pub field: SortField,
    pub direction: SortDirection,
}

impl SortMode {
    /// Get all possible sort mode combinations (12 total: 6 fields × 2 directions)
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

    /// Get a human-readable display text for UI menus
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

    /// Default sort mode: by index in ascending order
    pub const DEFAULT: Self = Self {
        field: SortField::Index,
        direction: SortDirection::Ascending,
    };
}
