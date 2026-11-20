//! Choosing a video variant and an audio rendition from a master playlist.

use super::playlist::{Rendition, Variant};

/// The height aimed for when the app has no quality preference.
pub const DEFAULT_HEIGHT: u32 = 720;

/// The tallest variant not above `target` (default [`DEFAULT_HEIGHT`]), else
/// the shortest one above it. Variants without a height are ranked by bandwidth.
pub fn pick_variant(variants: &[Variant], target: Option<u32>) -> Option<&Variant> {
    let target = target
        .filter(|height| *height > 0)
        .unwrap_or(DEFAULT_HEIGHT);
    let sized: Vec<&Variant> = variants.iter().filter(|v| v.height.is_some()).collect();
    if sized.is_empty() {
        // Nothing to compare with the target: the middle of the bandwidth ladder.
        let mut ranked: Vec<&Variant> = variants.iter().collect();
        ranked.sort_by_key(|variant| variant.bandwidth);
        return ranked.get(ranked.len().saturating_sub(1) / 2).copied();
    }
    let height = |variant: &&Variant| variant.height.unwrap_or(0);
    sized
        .iter()
        .filter(|v| height(v) <= target)
        .max_by_key(|v| (height(v), v.bandwidth))
        .or_else(|| sized.iter().min_by_key(|v| (height(v), v.bandwidth)))
        .copied()
}

/// The rendition of `group` in `language` (an ISO 639-1 or 639-2 code, or a
/// name), else the group's default, else its first.
pub fn pick_audio<'a>(
    renditions: &'a [Rendition],
    group: &str,
    language: Option<&str>,
) -> Option<&'a Rendition> {
    let group: Vec<&Rendition> = renditions
        .iter()
        .filter(|r| r.group_id == group && r.uri.is_some())
        .collect();
    let wanted = language.map(str::trim).filter(|l| !l.is_empty());
    wanted
        .and_then(|wanted| {
            group.iter().find(|r| {
                r.language
                    .as_deref()
                    .is_some_and(|l| same_language(l, wanted))
                    || same_language(&r.name, wanted)
            })
        })
        .or_else(|| group.iter().find(|r| r.default))
        .or_else(|| group.first())
        .copied()
}

const LANGUAGES: &[&[&str]] = &[
    &["en", "eng", "english"],
    &["hi", "hin", "hindi"],
    &["ja", "jpn", "japanese"],
    &["ta", "tam", "tamil"],
    &["te", "tel", "telugu"],
    &["ml", "mal", "malayalam"],
    &["kn", "kan", "kannada"],
    &["bn", "ben", "bengali", "bangla"],
    &["es", "spa", "spanish"],
];

fn same_language(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim().to_ascii_lowercase(), b.trim().to_ascii_lowercase());
    // "en-US" is English.
    let primary = |s: &str| s.split(['-', '_']).next().unwrap_or_default().to_string();
    let (a, b) = (primary(&a), primary(&b));
    a == b
        || LANGUAGES
            .iter()
            .any(|forms| forms.contains(&a.as_str()) && forms.contains(&b.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variant(height: Option<u32>, bandwidth: u64) -> Variant {
        Variant {
            uri: format!("v{bandwidth}"),
            bandwidth,
            height,
            name: None,
            audio_group: Some("audio".into()),
        }
    }

    fn rendition(language: &str, name: &str, default: bool) -> Rendition {
        Rendition {
            group_id: "audio".into(),
            language: Some(language.into()),
            name: name.into(),
            default,
            uri: Some(format!("a-{language}")),
        }
    }

    fn ladder() -> Vec<Variant> {
        [240, 360, 480, 720, 1080]
            .iter()
            .map(|h| variant(Some(*h), u64::from(*h) * 1000))
            .collect()
    }

    #[test]
    fn picks_the_tallest_variant_within_the_target() {
        let ladder = ladder();
        assert_eq!(pick_variant(&ladder, Some(720)).unwrap().height, Some(720));
        assert_eq!(pick_variant(&ladder, Some(600)).unwrap().height, Some(480));
        assert_eq!(
            pick_variant(&ladder, Some(2160)).unwrap().height,
            Some(1080)
        );
        assert_eq!(pick_variant(&ladder, None).unwrap().height, Some(720));
    }

    #[test]
    fn falls_back_to_the_shortest_variant_above_the_target() {
        assert_eq!(
            pick_variant(&ladder(), Some(144)).unwrap().height,
            Some(240)
        );
    }

    #[test]
    fn ranks_unsized_variants_by_bandwidth() {
        let list = vec![variant(None, 300), variant(None, 100), variant(None, 200)];
        assert_eq!(pick_variant(&list, Some(720)).unwrap().bandwidth, 200);
        assert!(pick_variant(&[], None).is_none());
    }

    #[test]
    fn picks_the_audio_language() {
        let list = vec![
            rendition("eng", "English", false),
            rendition("jpn", "Japanese", false),
            rendition("hin", "Hindi", false),
        ];
        assert_eq!(
            pick_audio(&list, "audio", Some("hi")).unwrap().name,
            "Hindi"
        );
        assert_eq!(
            pick_audio(&list, "audio", Some("ja")).unwrap().name,
            "Japanese"
        );
        assert_eq!(
            pick_audio(&list, "audio", Some("en-US")).unwrap().name,
            "English"
        );
        // No match and no default: the first one.
        assert_eq!(
            pick_audio(&list, "audio", Some("ta")).unwrap().name,
            "English"
        );
        assert_eq!(pick_audio(&list, "audio", None).unwrap().name, "English");
        assert!(pick_audio(&list, "other", None).is_none());
    }

    #[test]
    fn prefers_the_default_rendition_without_a_match() {
        let list = vec![
            rendition("eng", "English", false),
            rendition("jpn", "Japanese", true),
        ];
        assert_eq!(
            pick_audio(&list, "audio", Some("ta")).unwrap().name,
            "Japanese"
        );
    }
}
