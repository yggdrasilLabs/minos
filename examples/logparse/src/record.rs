//! The four fields one input line yields, and their tab-separated rendering.

/// What one log line yields. A field the line does not carry is absent and renders as `-`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Record {
    timestamp: Option<String>,
    level: Option<String>,
    component: Option<String>,
    latency_ms: Option<u64>,
}

/// The rendering of an absent field.
const ABSENT: &str = "-";

impl Record {
    /// A record from its four fields, in output order.
    #[must_use]
    pub fn new(
        timestamp: Option<String>,
        level: Option<String>,
        component: Option<String>,
        latency_ms: Option<u64>,
    ) -> Self {
        Self {
            timestamp,
            level,
            component,
            latency_ms,
        }
    }

    /// The four fields joined by tabs, in the order timestamp, level, component, latency, with `-` for an absent
    /// field. The row number and the line ending are the caller's.
    #[must_use]
    pub fn to_tsv(&self) -> String {
        let latency = self.latency_ms.map(|value| value.to_string());
        [
            self.timestamp.as_deref(),
            self.level.as_deref(),
            self.component.as_deref(),
            latency.as_deref(),
        ]
        .map(|field| field.unwrap_or(ABSENT))
        .join("\t")
    }
}

#[cfg(test)]
mod tests {
    use super::Record;

    #[test]
    fn an_empty_record_renders_four_dashes() {
        // Arrange
        let record = Record::default();

        // Act
        let row = record.to_tsv();

        // Assert
        assert_eq!(row, "-\t-\t-\t-");
    }

    #[test]
    fn a_full_record_renders_its_fields_in_order() {
        // Arrange
        let record = Record::new(
            Some("2026-10-28T14:03:11Z".to_owned()),
            Some("INFO".to_owned()),
            Some("auth".to_owned()),
            Some(12),
        );

        // Act
        let row = record.to_tsv();

        // Assert
        assert_eq!(row, "2026-10-28T14:03:11Z\tINFO\tauth\t12");
    }

    #[test]
    fn a_missing_latency_renders_as_a_dash() {
        // Arrange
        let record = Record::new(
            Some("2026-10-28T14:03:36Z".to_owned()),
            Some("WARN".to_owned()),
            Some("db".to_owned()),
            None,
        );

        // Act
        let row = record.to_tsv();

        // Assert
        assert_eq!(row, "2026-10-28T14:03:36Z\tWARN\tdb\t-");
    }
}
