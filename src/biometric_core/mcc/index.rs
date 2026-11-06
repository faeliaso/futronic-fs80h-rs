//! Large-scale 1:N identification gallery with Rayon parallel work-stealing.

use super::cylinder::MccCylinder;
use super::encoder::MccTemplate;
use super::matcher::MccMatcher;
use super::params::MccConfig;
use rayon::prelude::*;

/// A gallery record associating a unique identity with an [`MccTemplate`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GalleryRecord {
    /// Unique subject or enrollment identifier.
    pub id: u64,
    /// Pre-computed MCC template.
    pub template: MccTemplate,
}

impl GalleryRecord {
    /// Creates a new gallery record.
    pub fn new(id: u64, template: MccTemplate) -> Self {
        Self { id, template }
    }

    /// Creates a record from raw cylinders.
    pub fn from_cylinders(id: u64, cylinders: Vec<MccCylinder>) -> Self {
        Self {
            id,
            template: MccTemplate { cylinders },
        }
    }
}

/// In-memory biometric gallery index providing parallel 1:N identification via Rayon.
#[derive(Debug, Clone, Default)]
pub struct GalleryIndex {
    records: Vec<GalleryRecord>,
    matcher: MccMatcher,
}

impl GalleryIndex {
    /// Creates an empty gallery index.
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            matcher: MccMatcher::new(),
        }
    }

    /// Creates a gallery index initialized with records and default matcher config.
    pub fn with_records(records: Vec<GalleryRecord>) -> Self {
        Self {
            records,
            matcher: MccMatcher::new(),
        }
    }

    /// Creates a gallery index with custom matcher configuration.
    pub fn with_config(records: Vec<GalleryRecord>, config: MccConfig) -> Self {
        Self {
            records,
            matcher: MccMatcher::with_config(config),
        }
    }

    /// Inserts a new record into the gallery index.
    pub fn insert(&mut self, id: u64, template: MccTemplate) {
        self.records.push(GalleryRecord::new(id, template));
    }

    /// Returns the number of enrolled records.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns `true` if the gallery contains no records.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Returns a slice of all stored records.
    pub fn records(&self) -> &[GalleryRecord] {
        &self.records
    }

    /// Searches the entire gallery in parallel using Rayon work-stealing,
    /// returning all candidates meeting or exceeding `threshold`.
    pub fn search(&self, probe: &MccTemplate, threshold: f32) -> Vec<(u64, f32)> {
        let matcher = &self.matcher;
        self.records
            .par_iter()
            .filter_map(|record| {
                let score = matcher.match_pair(probe, &record.template).score;
                if score >= threshold {
                    Some((record.id, score))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Searches the gallery and returns the single candidate with the highest matching score,
    /// tie-breaking by the number of matched minutiae pairs.
    pub fn search_best(&self, probe: &MccTemplate) -> Option<(u64, f32)> {
        let matcher = &self.matcher;
        self.records
            .par_iter()
            .map(|record| {
                let res = matcher.match_pair(probe, &record.template);
                (record.id, res.score, res.matched_pairs)
            })
            .max_by(|a, b| {
                a.1.partial_cmp(&b.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.2.cmp(&b.2))
            })
            .filter(|&(_, score, _)| score > 0.0)
            .map(|(id, score, _)| (id, score))
    }

    /// Searches the gallery and returns up to `n` top-ranking candidates meeting `threshold`,
    /// sorted in descending order of score.
    pub fn search_top_n(&self, probe: &MccTemplate, n: usize, threshold: f32) -> Vec<(u64, f32)> {
        let mut results = self.search(probe, threshold);
        results.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(n);
        results
    }
}
